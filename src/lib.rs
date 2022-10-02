use std::fs::File;
use std::io::{Read, Write};
use subprocess::{Exec, Redirection};

// #[cfg(windows)]
// pub fn command_exists(cmd: &str) -> bool {
//     let output = Exec::cmd("cmd").arg("/c").arg("where").arg(cmd)
//         .stdout(Redirection::Pipe)
//         .capture()
//         .unwrap()
//         .stdout_str();

//     !output.is_empty()
// }

#[cfg(not(windows))]
pub fn command_exists(cmd: &str) -> bool {
    let output = Exec::cmd("which").arg(cmd)
        .stdout(Redirection::Pipe)
        .capture()
        .unwrap()
        .stdout_str();

    !output.is_empty()
}

pub fn copy_to(mut src: File, mut dst: File) -> std::io::Result<()> {
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
