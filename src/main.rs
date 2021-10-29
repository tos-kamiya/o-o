use std::env;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use anyhow::Result;

use subprocess::{Exec, ExitStatus, Redirection};
use tempfile::{tempdir, TempDir};

mod unsp;


const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const NAME: &'static str = env!("CARGO_PKG_NAME");

const STDOUT_TEMP: &str = "STDOUT_TEMP";

const USAGE: &str = "Redirect subprocess's standard i/o's.

Usage:
  o-o [options] <stdin> (-a <stdout>|<stdout>) (-a <stderr>|<stderr>) [--] <commandline>...
  o-o --help
  o-o --version

Options:
  <stdin>       File served as the standard input. `-` for no redirection.
  <stdout>      File served as the standard output. `-` for no redirection. `=` for the same file as the standard input.
  <stderr>      File served as the standard error. `-` for no redirection. `=` for the same file as the standard output.
  -e VAR=VALUE      Environment variables.
  --force-overwrite, -F             Overwrite the file even when exit status != 0. Valid only when <stdout> is `=`.
  --working-directory=DIR, -d DIR   Working directory.
";


#[derive(Debug, PartialEq)]
struct FileMode {
    name: String,
    append: bool,
}

impl FileMode {
    pub fn w(name: String) -> FileMode {
        FileMode { name, append: false }
    }

    pub fn a(name: String) -> FileMode {
        FileMode { name, append: true }
    }
}


fn main() -> Result<()> {
    // Parse command-line arguments
    let mut fds = Vec::<FileMode>::new();
    let mut command_line = Vec::<String>::new();
    let mut force_overwrite = false;
    let mut envs = Vec::<(String, String)>::new();
    let mut working_directory: Option<String> = None;
    let mut debug_info = false;

    let argv: Vec<String> = env::args().collect();
    let mut ai = 1;
    'ai: while ai < argv.len() && fds.len() < 3 {
        for alt in unsp::parse(&argv, ai) {
            match alt.0 {
                unsp::Arg::FlagOption(name) if name == "-h" || name == "--help" => {
                    print!("{}", USAGE);
                    std::process::exit(0);
                }
                unsp::Arg::FlagOption(name) if name == "-v" || name == "--version" => {
                    println!("{} {}", NAME, VERSION);
                    std::process::exit(0);
                }
                unsp::Arg::FlagOption(name) if name == "-F" || name == "--force-overwrite" => {
                    println!("{} {}", NAME, VERSION);
                    force_overwrite = true;
                    ai += alt.1;
                    continue 'ai;
                }
                unsp::Arg::FlagOption(name) if name == "--debug-info" => {
                    debug_info = true;
                    ai += alt.1;
                    continue 'ai;
                }
                unsp::Arg::ValueOption(name, value) if name == "-e" => {
                    let p = value.find("=").unwrap_or_else(|| {
                        eprintln!("{}", "Error: option -e's argument should be `VAR=VALUE`.");
                        std::process::exit(1);
                    });
                    envs.push((value[.. p].to_string(), value[p + 1 ..].to_string()));
                    ai += alt.1;
                    continue 'ai;
                }
                unsp::Arg::ValueOption(name, value) if name == "-d" || name == "--working-directory" => {
                    working_directory = Some(value.to_string());
                    ai += alt.1;
                    continue 'ai;
                }
                unsp::Arg::Separator(_) => {
                    ai += alt.1;
                    break 'ai;
                }
                unsp::Arg::Value(value) => {
                    if value.starts_with("+") {
                        fds.push(FileMode::a(value[1..].to_string()));
                    } else {
                        fds.push(FileMode::w(value.to_string()));
                    }
                    ai += alt.1;
                    continue 'ai;
                }
                _ => {
                }
            }
        }
        eprintln!("Error: invalid option/argument: {}", argv[ai]);
        std::process::exit(1);
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

    // Validate arguments
    if fds.len() < 3 {
        eprintln!("{}", "Error: required three arguments: stdin, stdout and stderr.");
        std::process::exit(1);
    }
    if command_line.is_empty() {
        eprintln!("{}", "Error: no command line specified.");
        std::process::exit(1);
    }
    if force_overwrite {
        if fds[0].name == "-" {
            eprintln!("{}", "Error: option --force-overwrite requires a real file name.");
            std::process::exit(1);
        }
        if fds[1].name != "=" {
            eprintln!("{}", "Error: option --force-overwrite is only valid when <stdout> is `=`.");
            std::process::exit(1);
        }
    }
    if fds[0].name == "=" {
        eprintln!("{}", "Error: can not specify `=` as stdin.");
        std::process::exit(1);
    }
    if fds[0].name == "-" && fds[1].name == "=" {
        fds[1].name = "-".to_string();
    }

    // Invoke a sub-process
    let sp = Exec::cmd(&command_line[0]).args(&command_line[1..]);

    let sp = if ! envs.is_empty() {
        sp.env_extend(&envs)
    } else {
        sp
    };

    let sp = if let Some(dir) = working_directory {
        sp.cwd(dir)
    } else {
        sp
    };

    let sp = if fds[0].name != "-" {
        let f = File::open(&fds[0].name)?;
        sp.stdin(f)
    } else { 
        sp 
    };

    let mut temp_dir: Option<TempDir> = None;
    let sp = if fds[1].name == "=" {
        let dir = tempdir()?;
        let f = File::create(dir.path().join(STDOUT_TEMP))?;
        temp_dir = Some(dir);
        sp.stdout(f)
    } else if fds[1].name != "-" {
        let f = if fds[1].append {
            OpenOptions::new().append(true).open(&fds[1].name)?
        } else {
            OpenOptions::new().write(true).truncate(true).open(&fds[1].name)?
        };
        sp.stdout(f)
    } else {
        sp
    };

    let sp = if fds[2].name == "=" {
        sp.stderr(Redirection::Merge)
    } else if fds[2].name != "-" {
        let f = if fds[1].append {
            OpenOptions::new().append(true).open(&fds[2].name)?
        } else {
            OpenOptions::new().write(true).truncate(true).open(&fds[2].name)?
        };
        sp.stderr(f)
    } else {
        sp
    };

    let exit_status = sp.join()?;
    let success = matches!(exit_status, ExitStatus::Exited(0));

    if let Some(dir) = temp_dir {
        let temp_file = dir.path().join(STDOUT_TEMP);
        if success || force_overwrite {
            if fds[0].append {
                let mut dst = OpenOptions::new().append(true).open(&fds[0].name)?;
                let mut src = File::open(temp_file)?;
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
            } else {
                fs::rename(temp_file, &fds[0].name)?;
            }
        }
        dir.close()?;
    }

    if ! success {
        eprintln!("Error: {:?}", exit_status);
        if let ExitStatus::Exited(code) = exit_status {
            std::process::exit(code.try_into()?);
        }
        std::process::exit(1);
    }

    Ok(())
}
