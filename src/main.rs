use std::env;
use std::fs;
use std::rc::{Rc};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};

use anyhow::Result;
use subprocess::{Exec, ExitStatus, Redirection, Pipeline};
use tempfile::{tempdir, TempDir};

use zgclp::{arg_parse, Arg};

fn split_append_flag(file_name: &str) -> (&str, bool) {
    if file_name.starts_with("+") {
        (&file_name[1..], true)
    } else {
        (file_name, false)
    }
}

fn open_for_writing(file_name: &str) -> std::io::Result<File> {
    let (f, a) = split_append_flag(file_name);
    if a {
        OpenOptions::new().create(true).append(true).open(f)
    } else {
        File::create(f)
    }
}

fn copy_to(mut src: File, mut dst: File) -> Result<()> {
    let mut buf = [0; 64*1024];
    loop {
        match src.read(&mut buf)? {
            0 => { break; }
            n => {
                let buf = &buf[..n];
                dst.write(buf)?;
            }
        }
    }
    Ok(())
}

const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const NAME: &'static str = env!("CARGO_PKG_NAME");

const STDOUT_TEMP: &str = "STDOUT_TEMP";

const USAGE: &str = "Redirect subprocess's standard i/o's.

Usage:
  o-o [options] <stdin> <stdout> <stderr> [--] <commandline>...
  o-o --help
  o-o --version

Options:
  <stdin>       File served as the standard input. `-` for no redirection.
  <stdout>      File served as the standard output. `-` for no redirection. `=` for the same file as the standard input.
  <stderr>      File served as the standard error. `-` for no redirection. `=` for the same file as the standard output.
                Adding `+` before a file name means appending to the file (like `>>` in redirects).
  -e VAR=VALUE                      Environment variables.
  --pipe=STR, -p STR                Use the string for connecting sub-processes by pipe (that is, `|`).
  --force-overwrite, -F             Overwrite the file even when exit status != 0. Valid only when <stdout> is `=`.
  --working-directory=DIR, -d DIR   Working directory.
";

fn do_validate_fds<'a>(fds: &'a Vec::<&'a str>, force_overwrite: bool) -> std::result::Result<(), &str> {
    if fds.len() < 3 {
        return Err("required three arguments: stdin, stdout and stderr.");
    }

    for i in 0..fds.len() {
        if fds[i] == "+-" || fds[i] == "+=" {
            return Err("not possible to use `-` or `=` in combination with `+`.");
        }
        if fds[i] != "-" && fds[i] != "=" {
            for j in i + 1 .. fds.len() {
                if split_append_flag(fds[j]).0 == split_append_flag(fds[i]).0 {
                    return Err("explicitly use `=` when dealing with the same file.");
                }
            }
        }
    }

    if force_overwrite {
        if fds[0] == "-" {
            return Err("option --force-overwrite requires a real file name.");
        }
        if fds[1] != "=" {
            return Err("option --force-overwrite is only valid when <stdout> is `=`.");
        }
    }

    if fds[0] == "=" {
        return Err("can not specify `=` as stdin.");
    }

    Ok(())
}


macro_rules! exec_it {
    ( $sp:ident, $fds:expr, $force_overwrite:expr ) => (
        {
            if $fds[0] != "-" {
                let fname = split_append_flag(&$fds[0]).0;
                let f = File::open(fname).unwrap_or_else(|_| {
                    eprintln!("Error: o-o: fail to open: {}", fname);
                    std::process::exit(1);
                });
                $sp = $sp.stdin(f);
            }

            let mut temp_dir: Option<TempDir> = None;
            if $fds[1] == "=" {
                let dir = tempdir()?;
                let f = File::create(dir.path().join(STDOUT_TEMP))?;
                temp_dir = Some(dir);
                $sp = $sp.stdout(f);
            } else if $fds[1] != "-" {
                let f = open_for_writing(&$fds[1])?;
                $sp = $sp.stdout(f);
            }

            let exit_status = $sp.join()?;
            let success = matches!(exit_status, ExitStatus::Exited(0));

            if let Some(dir) = temp_dir {
                let temp_file = dir.path().join(STDOUT_TEMP);
                let (f, a) = split_append_flag(&$fds[0]);
                if success || $force_overwrite {
                    if a {
                        let dst = open_for_writing(&$fds[0])?;
                        let src = File::open(temp_file)?;
                        copy_to(src, dst)?;
                    } else {
                        fs::rename(temp_file, f)?;
                    }
                } else if ! success {
                    eprintln!("Warning: exit code != 0. Not overwrite the file: {}", f); 
                }
                dir.close()?;
            }

            exit_status
        }
    )
}

fn main() -> Result<()> {
    // Parse command-line arguments
    let mut fds = Vec::<&str>::new();
    let mut command_line = Vec::<&str>::new();
    let mut force_overwrite = false;
    let mut envs = Vec::<(&str, &str)>::new();
    let mut working_directory: Option<&str> = None;
    let mut debug_info = false;
    let mut pipe_str: Option<&str> = None;

    let argv0: Vec<String> = env::args().collect();
    let argv: Vec<&str> = argv0.iter().map(AsRef::as_ref).collect();
    let mut ai = 1;
    while ai < argv.len() && fds.len() < 3 {
        let eat = match arg_parse(&argv, ai) {
            (Arg::Option("-h" | "--help"), Some(_eat), _) => {
                print!("{}", USAGE);
                std::process::exit(0);
            }
            (Arg::Option("-v" | "--version"), Some(_eat), _) => {
                println!("{} {}", NAME, VERSION);
                std::process::exit(0);
            }
            (Arg::Option("-F" | "--force-overwrite"), Some(eat), _) => {
                force_overwrite = true;
                eat
            }
            (Arg::Option("--debug-info"), Some(eat), _) => {
                debug_info = true;
                eat
            }
            (Arg::Option("-e"), _, Some((eat, value))) => {
                let p = value.find("=").unwrap_or_else(|| {
                    eprintln!("{}", "Error: o-o: option -e's argument should be `VAR=VALUE`.");
                    std::process::exit(1);
                });
                envs.push((&value[.. p], &value[p + 1 ..]));
                eat
            }
            (Arg::Option("-d" | "--working-directory"), _, Some((eat, value))) => {
                working_directory = Some(value);
                eat
            }
            (Arg::Option("-p" | "--pipe"), _, Some((eat, value))) => {
                pipe_str = Some(value);
                eat
            }
            (Arg::Separator(_), Some(eat), _) => {
                while fds.len() < 3 {
                    fds.push("-");
                }
                eat
            }
            (Arg::Value, _, Some((eat, value))) => {
                fds.push(value);
                eat
            }
            _ => {
                eprintln!("Error: o-o: invalid option/argument: {}", argv[ai]);
                std::process::exit(1);
            }
        };
        ai += eat;
    }
    command_line.extend_from_slice(&argv[ai ..]);

    let mut sub_command_lines = Vec::<&[&str]>::new();
    if let Some(p) = pipe_str {
        let mut i = 0;
        for j in i..command_line.len() {
            if command_line[j] == p && j > i {
                sub_command_lines.push(&command_line[i..j]);
                i = j + 1;
            }
        }
    } else {
        sub_command_lines.push(&command_line);
    }

    if debug_info {
        println!("fds = {:?}", fds);
        println!("command_line = {:?}", command_line);
        println!("sub_command_line = {:?}", sub_command_lines);
        println!("force_overwrite = {:?}", force_overwrite);
        println!("envs = {:?}", envs);
        println!("working_directory = {:?}", working_directory);
        println!("pipe = {:?}", pipe_str);
        return Ok(());
    }

    // Validate command-line arguments
    if command_line.is_empty() {
        eprintln!("{}", "Error: o-o: no command line specified.");
        std::process::exit(1);
    }

    if let Err(message) = do_validate_fds(&fds, force_overwrite) {
        eprintln!("Error: o-o: {}", message);
        std::process::exit(1);
    }

    if fds[0] == "-" && fds[1] == "=" {
        fds[1] = "-";
    }

    let mut stderr_sink: Option<Rc<File>> = None;
    if fds[2] != "-" && fds[2] != "=" {
        let f = open_for_writing(&fds[2])?;
        stderr_sink = Some(Rc::new(f));
    }

    // Invoke a sub-process
    let mut execs = sub_command_lines.iter().map(|&scl| { 
        let mut exec = Exec::cmd(&scl[0]).args(&scl[1..]);
        if ! envs.is_empty() {
            exec = exec.env_extend(&envs);
        }
        if let Some(dir) = working_directory {
            exec = exec.cwd(dir);
        }
        if let Some(ss) = &stderr_sink {
            exec = exec.stderr(Redirection::RcFile(ss.clone()));
        } else if fds[2] == "=" {
            exec = exec.stderr(Redirection::Merge);
        }
        exec
    }).collect::<Vec<Exec>>();

    let exit_status = if execs.len() >= 2 {
        let mut sp = Pipeline::from_exec_iter(execs);
        exec_it!(sp, fds, force_overwrite)
    } else {
        let mut sp = execs.pop().unwrap();
        exec_it!(sp, fds, force_overwrite)
    };

    let success = matches!(exit_status, ExitStatus::Exited(0));
    if ! success {
        eprintln!("Error: o-o: {:?}", exit_status);
        if let ExitStatus::Exited(code) = exit_status {
            std::process::exit(code.try_into()?);
        } else {
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(test)]

mod test {
    use super::*;

    #[test]
    fn missing_fds() {
        let fds: Vec<&str> = vec!["a", "b"];
        assert!(do_validate_fds(&fds, false).is_err());
    }

    #[test]
    fn invalid_usage_of_plus() {
        let fds: Vec<&str> = vec!["a", "b", "+="];
        assert!(do_validate_fds(&fds, false).is_err());

        let fds: Vec<&str> = vec!["a", "b", "+-"];
        assert!(do_validate_fds(&fds, false).is_err());
    }

    #[test]
    fn invalid_usage_of_equal() {
        let fds: Vec<&str> = vec!["=", "b", "c"];
        assert!(do_validate_fds(&fds, false).is_err());
    }

    #[test]
    fn same_file_names() {
        let fds: Vec<&str> = vec!["a", "a", "b"];
        assert!(do_validate_fds(&fds, false).is_err());

        let fds: Vec<&str> = vec!["a", "b", "a"];
        assert!(do_validate_fds(&fds, false).is_err());

        let fds: Vec<&str> = vec!["a", "b", "b"];
        assert!(do_validate_fds(&fds, false).is_err());
    }

    #[test]
    fn force_overwrite() {
        let fds: Vec<&str> = vec!["a", "b", "c"];
        assert!(do_validate_fds(&fds, true).is_err());

        let fds: Vec<&str> = vec!["a", "=", "c"];
        assert!(do_validate_fds(&fds, true).is_ok());

        let fds: Vec<&str> = vec!["-", "=", "c"];
        assert!(do_validate_fds(&fds, true).is_err());
    }
}
