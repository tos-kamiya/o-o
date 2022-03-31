use std::env;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::rc::Rc;

use anyhow::{Context, Result};
use subprocess::{Exec, ExitStatus, NullFile, Pipeline, Redirection};
use tempfile::{tempdir, TempDir};
use thiserror::Error;

use ng_clp::{is_argument, next_index, parse, unwrap_argument};

fn split_append_flag(file_name: &str) -> (&str, bool) {
    if let Some(stripped) = file_name.strip_prefix('+') {
        (stripped, true)
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
    let mut buf = [0; 64 * 1024];
    loop {
        match src.read(&mut buf)? {
            0 => {
                break;
            }
            n => {
                let buf = &buf[..n];
                dst.write_all(buf)?;
            }
        }
    }
    Ok(())
}

const VERSION: &str = env!("CARGO_PKG_VERSION");
const NAME: &str = env!("CARGO_PKG_NAME");

const STDOUT_TEMP: &str = "STDOUT_TEMP";

const USAGE: &str = "Start a sub-process and redirect its standard I/O's.

Usage:
  o-o [options] <stdin> <stdout> <stderr> [--] <commandline>...
  o-o --help
  o-o --version

Options:
  <stdin>       File served as the standard input. `-` for no redirection.
  <stdout>      File served as the standard output. `-` for no redirection. `=` for the same file as the standard input. `.` for /dev/null.
  <stderr>      File served as the standard error. `-` for no redirection. `=` for the same file as the standard output. `.` for /dev/null.
                Prefixing the file name with `+` will append to the file (same as `>>`).
  -e VAR=VALUE                      Environment variables.
  --pipe=STR, -p STR                Use the string for connecting sub-processes by pipe (that is, `|`).
  --force-overwrite, -F             Overwrite the file even when exit status != 0. Valid only when <stdout> is `=`.
  --working-directory=DIR, -d DIR   Working directory.
  --version, -V                     Version info.
  --help, -h                        Help message.
";

#[derive(Error, Debug)]
pub enum OOError {
    #[error("o-o: invalid argument/option {}", .name)]
    InvaidArgument { name: String },

    #[error("o-o: no command line specified")]
    NoCommandLineSpecified,

    #[error("o-o: {}", .message)]
    CLIError { message: String },
}

#[derive(Debug, PartialEq)]
struct Args<'s> {
    fds: Vec<&'s str>,
    command_line: Vec<&'s str>,
    force_overwrite: bool,
    envs: Vec<(&'s str, &'s str)>,
    working_directory: Option<&'s str>,
    debug_info: bool,
    pipe_str: Option<&'s str>,
}

impl Args<'_> {
    fn parse<'s>(argv: &Vec<&'s str>) -> anyhow::Result<Args<'s>> {
        let mut args = Args {
            fds: vec![],
            command_line: vec![],
            force_overwrite: false,
            envs: vec![],
            working_directory: None,
            debug_info: false,
            pipe_str: None,
        };

        let mut argv_index = 1;
        while args.fds.len() < 3 {
            let pr = parse(&argv, argv_index)?;
            let eat = match pr.0 {
                "-h" | "--help" => { // help
                    print!("{}", USAGE);
                    std::process::exit(0);
                }
                "-V" | "--version" => {
                    println!("{} {}", NAME, VERSION);
                    std::process::exit(0);
                }
                "-F" | "--force-overwrite" => {
                    args.force_overwrite = true;
                    1
                }
                "--debug-info" => {
                    args.debug_info = true;
                    1
                }
                "-e" => {
                    let value = unwrap_argument(pr)?;
                    let p = value
                        .find('=')
                        .expect("o-o: option -e's argument should be `VAR=VALUE`");
                    args.envs.push((&value[..p], &value[p + 1..]));
                    2
                }
                "-d" | "--working-directory" => {
                    args.working_directory = Some(unwrap_argument(pr)?);
                    2
                }
                "-p" | "--pipe"  => {
                    args.pipe_str = Some(unwrap_argument(pr)?);
                    2
                }
                "--" => { // separator
                    while args.fds.len() < 3 {
                        args.fds.push("-");
                    }
                    break;
                }
                a if is_argument(a) => { // argument
                    args.fds.push(a);
                    1
                }
                _ => 0 // unknown flag/option 
            };
    
            argv_index = next_index(&argv, argv_index, eat)?;
            if argv_index >= argv.len() {
                break;
            }
        }
        if argv_index < argv.len() {
            if argv[argv_index] == "--" { // in case a redundant "--" is given as the 4th argument
                argv_index += 1;
            }
            args.command_line.extend_from_slice(&argv[argv_index..]);
        }

        if args.fds.len() < 3 {
            return Err(OOError::NoCommandLineSpecified.into());
        }

        Ok(args)
    }
}

fn do_validate_fds<'a>(fds: &'a [&'a str], force_overwrite: bool) -> anyhow::Result<()> {
    let err = |message: &str| {
        Err(OOError::CLIError { message: message.to_string() }.into())
    };

    if fds.len() < 3 {
        return err("requires three arguments: stdin, stdout and stderr");
    }

    for i in 0..fds.len() {
        if fds[i] == "+-" || fds[i] == "+=" {
            return err("not possible to use `-` or `=` in combination with `+`");
        }
        if fds[i] != "-" && fds[i] != "=" {
            for j in i + 1..fds.len() {
                if split_append_flag(fds[j]).0 == split_append_flag(fds[i]).0 {
                    return err("explicitly use `=` when dealing with the same file");
                }
            }
        }
    }

    if force_overwrite {
        if fds[0] == "-" {
            return err("option --force-overwrite requires a real file name");
        }
        if fds[1] != "=" {
            return err("option --force-overwrite is only valid when <stdout> is `=`");
        }
    }

    if fds[0] == "=" || fds[0] == "." {
        return err("can not specify either `=` or `.` as stdin");
    }

    Ok(())
}

macro_rules! exec_it {
    ( $sp:ident, $fds:expr, $force_overwrite:expr ) => {
        (|| -> Result<ExitStatus> {
            if $fds[0] != "-" {
                let fname = split_append_flag(&$fds[0]).0;
                let f = File::open(fname).with_context(|| format!("o-o: fail to open: {}", fname))?;
                $sp = $sp.stdin(f);
            }

            let mut temp_dir: Option<TempDir> = None;
            if $fds[1] == "=" {
                let dir = tempdir()?;
                let f = File::create(dir.path().join(STDOUT_TEMP))?;
                temp_dir = Some(dir);
                $sp = $sp.stdout(f);
            } else if $fds[1] == "." {
                $sp = $sp.stdout(NullFile);
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
                } else if !success {
                    eprintln!("Warning: exit code != 0. Not overwrite the file: {}", f);
                }
                dir.close()?;
            }

            Ok(exit_status)
        })()
    };
}

fn main() -> anyhow::Result<()> {
    // Parse command-line arguments
    let argv0: Vec<String> = env::args().collect();
    let argv: Vec<&str> = argv0.iter().map(AsRef::as_ref).collect();
    if argv.len() == 1 {
        print!("{}", USAGE);
        return Ok(());
    }

    let mut a = Args::parse(&argv)?;

    let mut sub_command_lines = Vec::<&[&str]>::new();
    if let Some(p) = a.pipe_str {
        let mut i = 0;
        for j in 0..a.command_line.len() {
            if a.command_line[j] == p && j > i {
                sub_command_lines.push(&a.command_line[i..j]);
                i = j + 1;
            }
        }
        if i < a.command_line.len() {
            sub_command_lines.push(&a.command_line[i..]);
        }
    } else {
        sub_command_lines.push(&a.command_line);
    }

    if a.debug_info {
        println!("fds = {:?}", a.fds);
        println!("command_line = {:?}", a.command_line);
        println!("sub_command_line = {:?}", sub_command_lines);
        println!("force_overwrite = {:?}", a.force_overwrite);
        println!("envs = {:?}", a.envs);
        println!("working_directory = {:?}", a.working_directory);
        println!("pipe = {:?}", a.pipe_str);
        return Ok(());
    }

    // Validate command-line arguments
    if a.command_line.is_empty() {
        return Err(OOError::NoCommandLineSpecified.into());
    }

    do_validate_fds(&a.fds, a.force_overwrite)?;

    if a.fds[0] == "-" && a.fds[1] == "=" {
        a.fds[1] = "-";
    }

    let mut stderr_sink: Option<Rc<File>> = None;
    if a.fds[2] != "-" && a.fds[2] != "=" && a.fds[2] != "." {
        let f = open_for_writing(a.fds[2])?;
        stderr_sink = Some(Rc::new(f));
    }

    // Invoke a sub-process
    let mut execs: Vec<Exec> = sub_command_lines
        .iter()
        .map(|&scl| {
            let mut exec = Exec::cmd(&scl[0]).args(&scl[1..]);
            if !a.envs.is_empty() {
                exec = exec.env_extend(&a.envs);
            }
            if let Some(dir) = a.working_directory {
                exec = exec.cwd(dir);
            }
            if let Some(ss) = &stderr_sink {
                exec = exec.stderr(Redirection::RcFile(ss.clone()));
            } else if a.fds[2] == "=" {
                exec = exec.stderr(Redirection::Merge);
            } else if a.fds[2] == "." {
                exec = exec.stderr(NullFile);
            }
            exec
        }).collect();

    let exit_status = if execs.len() >= 2 {
        let mut sp = Pipeline::from_exec_iter(execs);
        exec_it!(sp, a.fds, a.force_overwrite)
    } else {
        let mut sp = execs.pop().unwrap();
        exec_it!(sp, a.fds, a.force_overwrite)
    }?;

    let success = matches!(exit_status, ExitStatus::Exited(0));
    if !success {
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
mod argv_parse_test {
    use super::*;

    #[test]
    fn parse_empty() {
        let argv: Vec<&str> = vec!["exec", "cmd"];
        let _err: anyhow::Error = Args::parse(&argv).unwrap_err();
    }

    #[test]
    fn parse_fds() {
        let argv: Vec<&str> = vec!["exec", "a", "b", "c", "cmd"];
        let a = Args::parse(&argv).unwrap();

        assert_eq!(a, Args { 
            fds: vec!["a", "b", "c"],
            command_line: vec!["cmd"],
            force_overwrite: false,
            envs: vec![],
            working_directory: None,
            debug_info: false,
            pipe_str: None,
        });
    }

    #[test]
    fn parse_omitted_fds() {
        let argv: Vec<&str> = vec!["exec", "a", "b", "--", "cmd"];
        let a = Args::parse(&argv).unwrap();

        assert_eq!(a, Args { 
            fds: vec!["a", "b", "-"],
            command_line: vec!["cmd"],
            force_overwrite: false,
            envs: vec![],
            working_directory: None,
            debug_info: false,
            pipe_str: None,
        });
    }

    #[test]
    fn parse_omitted_fds2() {
        let argv: Vec<&str> = vec!["exec", "a", "--", "cmd"];
        let a = Args::parse(&argv).unwrap();

        assert_eq!(a, Args { 
            fds: vec!["a", "-", "-"],
            command_line: vec!["cmd"],
            force_overwrite: false,
            envs: vec![],
            working_directory: None,
            debug_info: false,
            pipe_str: None,
        });
    }

    #[test]
    fn parse_omitted_fds3() {
        let argv: Vec<&str> = vec!["exec", "--", "cmd"];
        let a = Args::parse(&argv).unwrap();

        assert_eq!(a, Args { 
            fds: vec!["-", "-", "-"],
            command_line: vec!["cmd"],
            force_overwrite: false,
            envs: vec![],
            working_directory: None,
            debug_info: false,
            pipe_str: None,
        });
    }
}

#[cfg(test)]
mod fds_validate_test {
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
