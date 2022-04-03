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
