use std::env;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};

use anyhow::Result;
use subprocess::{Exec, ExitStatus, Redirection};
use tempfile::{tempdir, TempDir};

use o_o::zgclp::{arg_parse, Arg};

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
  -e VAR=VALUE      Environment variables.
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

fn main() -> Result<()> {
    // Parse command-line arguments
    let mut fds = Vec::<&str>::new();
    let mut command_line = Vec::<&str>::new();
    let mut force_overwrite = false;
    let mut envs = Vec::<(&str, &str)>::new();
    let mut working_directory: Option<&str> = None;
    let mut debug_info = false;

    let argv0: Vec<String> = env::args().collect();
    let argv: Vec<&str> = argv0.iter().map(AsRef::as_ref).collect();
    let mut ai = 1;
    while ai < argv.len() && fds.len() < 3 {
        ai += match arg_parse(&argv, ai) {
            (Arg::Option("-h"), Some(_eat), _) => {
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
                    eprintln!("{}", "Error: option -e's argument should be `VAR=VALUE`.");
                    std::process::exit(1);
                });
                envs.push((&value[.. p], &value[p + 1 ..]));
                eat
            }
            (Arg::Option("-d" | "--working-directory"), _, Some((eat, value))) => {
                working_directory = Some(value);
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
                eprintln!("Error: invalid option/argument: {}", argv[ai]);
                std::process::exit(1);
            }
        };
    }
    command_line.extend_from_slice(&argv[ai ..]);

    if debug_info {
        println!("fds = {:?}", fds);
        println!("command_line = {:?}", command_line);
        println!("force_overwrite = {:?}", force_overwrite);
        println!("envs = {:?}", envs);
        println!("working_directory = {:?}", working_directory);
        return Ok(());
    }

    // Validate command-line arguments
    if command_line.is_empty() {
        eprintln!("{}", "Error: no command line specified.");
        std::process::exit(1);
    }

    if let Err(message) = do_validate_fds(&fds, force_overwrite) {
        eprintln!("Error: {}", message);
        std::process::exit(1);
    }

    if fds[0] == "-" && fds[1] == "=" {
        fds[1] = "-";
    }

    // Invoke a sub-process
    let mut sp = Exec::cmd(&command_line[0]).args(&command_line[1..]);

    if ! envs.is_empty() {
        sp = sp.env_extend(&envs);
    }

    if let Some(dir) = working_directory {
        sp = sp.cwd(dir);
    }

    if fds[0] != "-" {
        let f = File::open(split_append_flag(&fds[0]).0)?;
        sp = sp.stdin(f);
    }

    let mut temp_dir: Option<TempDir> = None;
    if fds[1] == "=" {
        let dir = tempdir()?;
        let f = File::create(dir.path().join(STDOUT_TEMP))?;
        temp_dir = Some(dir);
        sp = sp.stdout(f);
    } else if fds[1] != "-" {
        let f = open_for_writing(&fds[1])?;
        sp = sp.stdout(f);
    }

    if fds[2] == "=" {
        sp = sp.stderr(Redirection::Merge);
    } else if fds[2] != "-" {
        let f = open_for_writing(&fds[2])?;
        sp = sp.stderr(f);
    }

    let exit_status = sp.join()?;
    let success = matches!(exit_status, ExitStatus::Exited(0));

    if let Some(dir) = temp_dir {
        let temp_file = dir.path().join(STDOUT_TEMP);
        let (f, a) = split_append_flag(&fds[0]);
        if success || force_overwrite {
            if a {
                let dst = open_for_writing(&fds[0])?;
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

    if ! success {
        eprintln!("Error: {:?}", exit_status);
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
