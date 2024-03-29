#[macro_use]
extern crate anyhow;

use std::env;
use std::fs;
use std::fs::{File, OpenOptions};
use std::rc::Rc;
use std::thread::yield_now;

use anyhow::Context;
use subprocess::{Exec, ExitStatus, NullFile, Pipeline, Redirection};
use tempfile::{tempdir, TempDir};
use thiserror::Error;

use ng_clp::{is_argument, next_index, parse, unwrap_argument};

use o_o::*;

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

fn unpack_shorthand_args(a: &str) -> Option<Vec<&'static str>> {
    if a.len() != 3 {
        return None;
    }

    let mut v: Vec<&'static str> = vec![];
    for c in a.chars() {
        if c == '-' {
            v.push("-");
        } else if c == '.' {
            v.push(".");
        } else if c == '=' {
            v.push("=");
        } else {
            return None;
        }
    }

    return Some(v);
}

fn is_filename_like_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-' || c == '.'
}

fn replace_tempdir_name(arg: &str, tempdir_placeholder: &str, temp_dir_str: &str) -> Option<String> {
    if tempdir_placeholder.is_empty() {
        return None
    }

    let parts: Vec<&str> = arg.split(tempdir_placeholder).collect();
    let mut replaced_parts: Vec<String> = vec![];
    let mut replacement_occurs = false;
    for i in 0..parts.len() {
        let prev = if i > 0 { parts[i - 1] } else { "" };
        let next = if i + 1 < parts.len() { parts[i + 1] } else { "" };
        let prev_last_char = if prev.is_empty() { ' ' } else { prev.chars().last().unwrap() };
        let next_first_char = if next.is_empty() { ' ' } else { next.chars().nth(0).unwrap() };
        if !is_filename_like_char(prev_last_char) && next_first_char == '/' {
            replaced_parts.push(temp_dir_str.to_owned());
            replacement_occurs = true;
        } else {
            replaced_parts.push(parts[i].to_owned());
        }
    }

    if replacement_occurs {
        Some(replaced_parts.join(""))
    } else {
        None
    }
}

const VERSION: &str = env!("CARGO_PKG_VERSION");
const NAME: &str = env!("CARGO_PKG_NAME");

const STDOUT_TEMP: &str = "STDOUT_TEMP";

#[derive(Error, Debug)]
pub enum OOError {
    #[error("o-o: {}", .message)]
    CLIError { message: String },
}

const USAGE: &str = "Run a sub-process and customize how it handles standard I/O.

Usage:
  o-o [options] <stdin> <stdout> <stderr> [--] <commandline>...
  o-o --help
  o-o --version

Options:
  <stdin>       File served as the standard input. Use `-` for no redirection.
  <stdout>      File served as the standard output. Use `-` for no redirection, `=` for the same file as the standard input, and `.` for /dev/null.
  <stderr>      File served as the standard error. Use `-` for no redirection, `=` for the same file as the standard output, and `.` for /dev/null.
                Prefix with `+` to append to the file (akin to the `>>` redirection in shell).
  -e VAR=VALUE                      Set environment variables.
  --pipe=STR, -p STR                String for pipe to connect subprocesses (`|` in shell) [default: `I`].
  --separator=STR, -s STR           String for separator of command lines (`;` in shell) [default: `J`].
  --tempdir-placeholder=STR, -t STR     Placeholder string for temporary directory [default: `T`].
  --force-overwrite, -F             Overwrite the file even if subprocess fails (exit status != 0). Valid only when <stdout> is `=`.
  --keep-going, -k                  Only effective when multiple command lines are chained with the separator. Even if one command line fails, subsequent command lines continue to be executed.
  --working-directory=DIR, -d DIR   Working directory.
  --version, -V                     Version information.
  --help, -h                        Shows this help message.
";

#[derive(Debug, PartialEq)]
struct Args<'s> {
    fds: Vec<&'s str>,
    command_line: Vec<&'s str>,
    force_overwrite: bool,
    envs: Vec<(&'s str, &'s str)>,
    working_directory: Option<&'s str>,
    keep_going: bool,
    debug_info: bool,
    pipe_str: Option<&'s str>,
    separator_str: Option<&'s str>,
    tempdir_placeholder: Option<&'s str>,
}

impl Args<'_> {
    fn parse<'s>(argv: &[&'s str]) -> anyhow::Result<Args<'s>> {
        let mut args = Args {
            fds: vec![],
            command_line: vec![],
            force_overwrite: false,
            envs: vec![],
            working_directory: None,
            keep_going: false,
            debug_info: false,
            pipe_str: None,
            separator_str: None,
            tempdir_placeholder: None,
        };

        let argv = &argv[1..];
        let mut argv_index = 0;
        while args.fds.len() < 3 {
            if args.fds.is_empty() {
                if let Some(u) = unpack_shorthand_args(argv[argv_index]) {
                    args.fds = u;
                    argv_index += 1;
                    break; // while
                }
            }
            let pr = parse(argv, argv_index)?;
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
                "-k" | "--keep-going" => {
                    args.keep_going = true;
                    1
                }
                "--debug-info" => {
                    args.debug_info = true;
                    1
                }
                "-e" => {
                    let value = unwrap_argument(pr)?;
                    let p = value.find('=');
                    if p.is_none() {
                        return Err(OOError::CLIError { message: format!("option -e's argument should be `VAR=VALUE`: {}", pr.0) }.into());
                    }
                    let p = p.unwrap();
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
                "-s" | "--separator"  => {
                    args.separator_str = Some(unwrap_argument(pr)?);
                    2
                }
                "-t" | "--tempdir-placeholder" => {
                    args.tempdir_placeholder = Some(unwrap_argument(pr)?);
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

            argv_index = next_index(argv, argv_index, eat)?;
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

        if args.command_line.is_empty() {
            return Err(OOError::CLIError { message: "no command line specified".to_string() }.into())
        }

        Ok(args)
    }
}

fn do_validate_fds(fds: &[&str], force_overwrite: bool) -> std::result::Result<(), OOError> {
    let err = |message: &str| {
        Err(OOError::CLIError { message: message.to_string() })
    };

    if fds.len() < 3 {
        return err("requires three arguments: stdin, stdout and stderr");
    }

    for fd in &fds[1..] {
        if command_exists(fd) {
            return Err(OOError::CLIError { message: format!("out/err looks a command: {}\n> (Use `--` to explicitly separate command from out/err)", fd)})
        }
    }

    for i in 0..fds.len() {
        if fds[i] == "+-" || fds[i] == "+=" {
            return err("not possible to use `-` or `=` in combination with `+`");
        }
        if !(fds[i] == "-" || fds[i] == "=" || fds[i] == ".") {
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
        (|| -> anyhow::Result<ExitStatus> {
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

fn exec_pipeline(pl: &[Vec<String>], fds: &[&str], envs: &[(&str, &str)], working_directory: &Option<&str>, force_overwrite: bool) -> anyhow::Result<u32> {
    let mut stderr_sink: Option<Rc<File>> = None;
    if !(fds[2] == "-" || fds[2] == "=" || fds[2] == ".") {
        let f = open_for_writing(fds[2])?;
        stderr_sink = Some(Rc::new(f));
    }

    let mut execs: Vec<Exec> = pl.iter()
    .map(|cml| {
        let mut exec = Exec::cmd(&cml[0]).args(&cml[1..]);
        if !envs.is_empty() {
            exec = exec.env_extend(envs);
        }
        if let Some(dir) = working_directory {
            exec = exec.cwd(dir);
        }
        if let Some(ss) = &stderr_sink {
            exec = exec.stderr(Redirection::RcFile(ss.clone()));
        } else if fds[2] == "=" {
            exec = exec.stderr(Redirection::Merge);
        } else if fds[2] == "." {
            exec = exec.stderr(NullFile);
        }
        exec
    }).collect();

    let exit_status = if execs.len() >= 2 {
        let mut sp = Pipeline::from_exec_iter(execs);
        exec_it!(sp, fds, force_overwrite)
    } else {
        let mut sp = execs.pop().unwrap();
        exec_it!(sp, fds, force_overwrite)
    }?;

    yield_now(); // force occurs a context switch, hoping completion of file IOs

    return if matches!(exit_status, ExitStatus::Exited(0)) {
        Ok(0)
    } else {
        if let ExitStatus::Exited(code) = exit_status {
            Ok(code)
        } else {
            Ok(1)
        }
    }
}

fn print_debug_info<S: AsRef<str>, T: AsRef<str>, U: AsRef<str>>(raw_args: &Args, pipelines : &[Vec<Vec<S>>], tempdir_replaced_arguments: &[(T, U)]) {
    println!("fds = {:?}", raw_args.fds);
    println!("command_line = {:?}", raw_args.command_line);
    println!("force_overwrite = {:?}", raw_args.force_overwrite);
    println!("keep_going = {:?}", raw_args.keep_going);
    println!("envs = {:?}", raw_args.envs);
    println!("working_directory = {:?}", raw_args.working_directory);
    println!("pipe = {:?}", raw_args.pipe_str);
    println!("tempdir_placeholder = {:?}", raw_args.tempdir_placeholder);

    println!();
    println!("target command lines:");
    for pl in pipelines.iter() {
        let mut buf = String::new();
        for (i, cml) in pl.iter().enumerate() {
            if i > 0 {
                buf.push_str(" | ");
            }
            for (j, a) in cml.iter().enumerate() {
                if j > 0 {
                    buf.push_str(" ");
                }
                buf.push_str(a.as_ref());
            }
        }
        println!("{:} ;", buf);
    }

    if !tempdir_replaced_arguments.is_empty() {
        println!();
        println!("tempdir-including arguments:");
        for tra in tempdir_replaced_arguments {
            println!("{:?}", tra.0.as_ref());
        }
    }
}

fn reform_pipeline_for_2nd_or_later_oo_command_line<'s>(pl: &'s Vec<Vec<String>>, a: &'s Args) -> anyhow::Result<(Vec<Vec<String>>, Args<'s>)> {
    let err = |message: &str| {
        Err(OOError::CLIError { message: message.to_string() }.into())
    };

    let pl0: Vec<&str> = pl.get(0).unwrap().iter().map(|s| s.as_ref()).collect();
    let mut sub_a = Args::parse(&pl0)?;
    if sub_a.debug_info {
        return err("invalid option used in sub-command: --debug-info");
    }
    if sub_a.pipe_str.is_some() {
        return err("invalid option used in sub-command: --pipe");
    }
    if sub_a.separator_str.is_some() {
        return err("invalid option used in sub-command: --separator");
    }
    if sub_a.tempdir_placeholder.is_some() {
        return err("invalid option used in sub-command: --tempdir-placeholder=");
    }

    do_validate_fds(&sub_a.fds, sub_a.force_overwrite)?;
    if sub_a.fds[0] == "-" && sub_a.fds[1] == "=" {
        sub_a.fds[1] = "-";
    }

    let mut sub_pl0: Vec<String> = vec![];
    for a in sub_a.command_line.iter() {
        sub_pl0.push(a.to_string());
    }
    let mut sub_pl: Vec<Vec<String>> = vec![sub_pl0];
    sub_pl.extend_from_slice(&pl[1..]);

    let mut envs: Vec<(&str, &str)> = vec![];
    envs.extend_from_slice(&a.envs);
    envs.extend_from_slice(&sub_a.envs);
    sub_a.envs = envs;

    if sub_a.working_directory.is_none() {
        sub_a.working_directory = a.working_directory;
    }
    sub_a.force_overwrite = sub_a.force_overwrite || a.force_overwrite;

    Ok((sub_pl, sub_a))
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

    let td_placeholder = a.tempdir_placeholder.unwrap_or("T");
    let pipe_str = a.pipe_str.unwrap_or("I");
    let separator_str = a.separator_str.unwrap_or("J");

    // Split sub-commands and replace temporary-directory path
    let mut pipelines: Vec<Vec<Vec<String>>> = vec![vec![vec![]]];
    let mut temp_dir: Option<TempDir> = None;
    let mut tdrep_args: Vec<(&str, String)> = vec![];
    for arg in a.command_line.iter() {
        if !separator_str.is_empty() && *arg == separator_str {
            if pipelines.last().unwrap().is_empty() {
                return Err(anyhow!("o-o: empty command line (unexpected separator)"));
            }
            pipelines.push(vec![vec![]]);
        } else if !pipe_str.is_empty() && *arg == pipe_str {
            let pl = pipelines.last_mut().unwrap();
            if pl.last().unwrap().is_empty() {
                return Err(anyhow!("o-o: empty command line (unexpected pipe)"));
            }
            pl.push(vec![]);
        } else {
            // Replace temp-directory holder string to a real temp-directory path
            let r = replace_tempdir_name(arg, td_placeholder, "dummy");
            pipelines.last_mut().unwrap().last_mut().unwrap().push(
                if r.is_some() {
                    let td = temp_dir.get_or_insert_with(|| tempdir().unwrap());
                    let td_path_str = td.path().to_str().unwrap();
                    let r = replace_tempdir_name(arg, td_placeholder, td_path_str).unwrap();
                    tdrep_args.push((arg, r.clone()));
                    r
                } else {
                    arg.to_string()
                }
            );
        }
    }

    if a.debug_info {
        print_debug_info(&a, &pipelines, &tdrep_args);
        return Ok(());
    }

    // Validate command-line arguments
    do_validate_fds(&a.fds, a.force_overwrite)?;
    if a.fds[0] == "-" && a.fds[1] == "=" {
        a.fds[1] = "-";
    }

    // Exec 1st pipeline
    let pl = pipelines.remove(0);
    let mut exit_code = exec_pipeline(&pl, &a.fds, &a.envs, &a.working_directory, a.force_overwrite)?;
    if ! a.keep_going && exit_code != 0 {
        std::process::exit(exit_code.try_into()?);
    }

    // Exec 2nd or later pipeline
    let non_redirected_fds = vec!["-", "-", "-"];
    a.fds = non_redirected_fds; // The second and subsequent pipelines do not redirect unless you explicitly write the o-o command
    for pl in pipelines.into_iter() {
        let pl0: Vec<&str> = pl.get(0).unwrap().iter().map(|s| s.as_ref()).collect();
        let cmd_is_oo = !pl0.is_empty() && pl0[0] == "o-o";
        exit_code = if cmd_is_oo {
            let (sub_pl, sub_a) = reform_pipeline_for_2nd_or_later_oo_command_line(&pl, &a)?;
            exec_pipeline(&sub_pl, &sub_a.fds, &sub_a.envs, &sub_a.working_directory, sub_a.force_overwrite)?
        } else {
            exec_pipeline(&pl, &a.fds, &a.envs, &a.working_directory, a.force_overwrite)?
        };
        if ! a.keep_going && exit_code != 0 {
            std::process::exit(exit_code.try_into()?);
        }
    }
    if exit_code != 0 {
        std::process::exit(exit_code.try_into()?);
    }

    Ok(())
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

#[cfg(test)]
mod main_tests {
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
            keep_going: false,
            envs: vec![],
            working_directory: None,
            debug_info: false,
            pipe_str: None,
            separator_str: None,
            tempdir_placeholder: None,
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
            keep_going: false,
            envs: vec![],
            working_directory: None,
            debug_info: false,
            pipe_str: None,
            separator_str: None,
            tempdir_placeholder: None,
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
            keep_going: false,
            envs: vec![],
            working_directory: None,
            debug_info: false,
            pipe_str: None,
            separator_str: None,
            tempdir_placeholder: None,
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
            keep_going: false,
            envs: vec![],
            working_directory: None,
            debug_info: false,
            pipe_str: None,
            separator_str: None,
            tempdir_placeholder: None,
        });
    }

    #[test]
    fn parse_shorthand_fds() {
        let argv: Vec<&str> = vec!["exec", "---", "cmd"];
        let a = Args::parse(&argv).unwrap();

        assert_eq!(a, Args { 
            fds: vec!["-", "-", "-"],
            command_line: vec!["cmd"],
            force_overwrite: false,
            keep_going: false,
            envs: vec![],
            working_directory: None,
            debug_info: false,
            pipe_str: None,
            separator_str: None,
            tempdir_placeholder: None,
        });
    }

    #[test]
    fn parse_including_tempdir() {
        let argv: Vec<&str> = vec!["exec", "---", "cat", "T/hoge.txt"];
        let a = Args::parse(&argv).unwrap();

        assert_eq!(a, Args { 
            fds: vec!["-", "-", "-"],
            command_line: vec!["cat", "T/hoge.txt"],
            force_overwrite: false,
            keep_going: false,
            envs: vec![],
            working_directory: None,
            debug_info: false,
            pipe_str: None,
            separator_str: None,
            tempdir_placeholder: None,
        });
    }

    #[test]
    fn parse_tempdir_option() {
        let argv: Vec<&str> = vec!["exec", "-t", "HOGE", "---", "cat", "HOGE/hoge.txt"];
        let a = Args::parse(&argv).unwrap();

        assert_eq!(a, Args { 
            fds: vec!["-", "-", "-"],
            command_line: vec!["cat", "HOGE/hoge.txt"],
            force_overwrite: false,
            keep_going: false,
            envs: vec![],
            working_directory: None,
            debug_info: false,
            pipe_str: None,
            separator_str: None,
            tempdir_placeholder: Some("HOGE"),
        });
    }

    #[test]
    fn parse_pipe_str_option() {
        let argv: Vec<&str> = vec!["exec", "--pipe", "%%", "---", "cat", "hoge.txt", "%%", "wc"];
        let a = Args::parse(&argv).unwrap();

        assert_eq!(a, Args { 
            fds: vec!["-", "-", "-"],
            command_line: vec!["cat", "hoge.txt", "%%", "wc"],
            force_overwrite: false,
            keep_going: false,
            envs: vec![],
            working_directory: None,
            debug_info: false,
            pipe_str: Some("%%"),
            separator_str: None,
            tempdir_placeholder: None,
        });
    }

    #[test]
    fn parse_separator_str_option() {
        let argv: Vec<&str> = vec!["exec", "--separator", "%%", "---", "cat", "hoge.txt", "%%", "cat", "fuga.txt"];
        let a = Args::parse(&argv).unwrap();

        assert_eq!(a, Args { 
            fds: vec!["-", "-", "-"],
            command_line: vec!["cat", "hoge.txt", "%%", "cat", "fuga.txt"],
            force_overwrite: false,
            keep_going: false,
            envs: vec![],
            working_directory: None,
            debug_info: false,
            pipe_str: None,
            separator_str: Some("%%"),
            tempdir_placeholder: None,
        });
    }
}
