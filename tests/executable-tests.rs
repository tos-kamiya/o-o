#[cfg(test)]
#[allow(non_snake_case)]

mod executable_tests {
    use std::fs;
    use std::io::{self, Write};
    use std::path::Path;
    use std::process::Command;

    use tempfile::tempdir;

    use o_o::do_sync;

    fn SU<'a>(p: &'a Path) -> &'a str {
        p.to_str().unwrap()
    }

    /// Helper function: writes contents to a file and flushes/syncs to disk.
    pub fn file_write<P: AsRef<Path> + Copy, C: AsRef<[u8]>>(
        path: P,
        contents: C,
    ) -> io::Result<()> {
        let mut f = fs::File::create(path)?;
        f.write(contents.as_ref())?;
        f.sync_all()?;
        do_sync();

        Ok(())
    }

    #[test]
    fn run_simple() {
        let status = Command::new("cargo")
            .args(["run"])
            .status()
            .expect("failed to run o-o");

        assert_eq!(status.code().unwrap(), 0);
    }

    #[test]
    fn run_ls() -> Result<(), io::Error> {
        const FILE_A: &str = "a.txt";

        let temp_dir = tempdir()?;

        let file_a = temp_dir.path().join(FILE_A);
        let _ = file_write(SU(&file_a), "file a.\n")?;

        let temp_file = temp_dir.path().join("ls-output.txt");
        let output = Command::new("./target/debug/o-o")
            .args(["-d", SU(&temp_dir.path()), "-", SU(&temp_file), "-", "ls"])
            .output()?;

        assert_eq!(output.status.code().unwrap(), 0);

        let temp_file_contents = fs::read_to_string(SU(&temp_file))?;
        assert!(temp_file_contents.find(FILE_A).is_some());

        assert!(Path::new(SU(&temp_file)).exists());

        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn run_ls_with_wrong_option() -> Result<(), io::Error> {
        let temp_dir = tempdir()?;

        let status = Command::new("cargo")
            .args([
                "run",
                "--",
                "-d",
                SU(&temp_dir.path()),
                "-",
                "-",
                "-",
                "ls",
                "--a-option-ls-must-not-have",
            ])
            .status()?;

        assert!(status.code().unwrap() != 0);

        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn sink_to_stdin() -> Result<(), io::Error> {
        const FILE_A: &str = "a.txt";

        let temp_dir = tempdir()?;

        let file_a = temp_dir.path().join(FILE_A);
        let _ = file_write(SU(&file_a), "1st line\n2nd line\n3rd line\n")?;

        let output = Command::new("cargo")
            .args([
                "run",
                "--",
                "-d",
                SU(&temp_dir.path()),
                SU(&file_a),
                "-",
                "-",
                "cat",
                SU(&file_a),
            ])
            .output()?;

        assert!(output.status.code().unwrap() == 0);

        let output_contents = String::from_utf8(output.stdout).unwrap();
        assert!(output_contents.find("2nd line").is_some());

        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn capture_stdout_and_stderr() -> Result<(), io::Error> {
        const SCRIPT: &str = "a_script.sh";

        let temp_dir = tempdir()?;

        let script = temp_dir.path().join(SCRIPT);
        let _ = file_write(SU(&script), "echo \"stdout\" >&1\necho \"stderr\" >&2\n")?;

        let out_file = temp_dir.path().join("out.txt");
        let err_file = temp_dir.path().join("err.txt");
        let output = Command::new("cargo")
            .args([
                "run",
                "--",
                "-d",
                SU(&temp_dir.path()),
                "-",
                SU(&out_file),
                SU(&err_file),
                "bash",
                SU(&script),
            ])
            .output()?; // use output()

        assert_eq!(
            output.status.code(),
            Some(0),
            "o-o command failed. Exit code: {:?}\nStdout: {}\nStderr: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr) // stderr from o-o itself
        );

        do_sync();

        let out_file_contents = fs::read_to_string(SU(&out_file))?;
        assert!(out_file_contents.find("stdout").is_some());
        assert!(out_file_contents.find("stderr").is_none());

        let err_file_contents = fs::read_to_string(SU(&err_file))?;
        assert!(err_file_contents.find("stderr").is_some());
        assert!(err_file_contents.find("stdout").is_none());

        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn redirect_stderr_to_stdout() -> Result<(), io::Error> {
        const SCRIPT: &str = "a_script.sh";

        let temp_dir = tempdir()?;

        let script = temp_dir.path().join(SCRIPT);
        let _ = file_write(SU(&script), "echo \"stdout\" >&1\necho \"stderr\" >&2\n")?;

        let output = Command::new("cargo")
            .args([
                "run",
                "--",
                "-d",
                SU(&temp_dir.path()),
                "-",
                "-",
                "=",
                "bash",
                SU(&script),
            ])
            .output()?;

        assert!(output.status.code().unwrap() == 0);

        do_sync();

        let output_contents = String::from_utf8(output.stdout).unwrap();
        assert!(output_contents.find("stdout").is_some());
        assert!(output_contents.find("stderr").is_some());

        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn append_to_output_file() -> Result<(), io::Error> {
        let temp_dir = tempdir()?;

        let out_file = temp_dir.path().join("out.txt");
        let append_out_file = format!("+{}", SU(&out_file));

        let status1 = Command::new("cargo")
            .args([
                "run",
                "--",
                "-d",
                SU(&temp_dir.path()),
                "-",
                &append_out_file,
                "-",
                "echo",
                "1st line",
            ])
            .status()?;
        assert!(status1.code().unwrap() == 0);

        do_sync();

        let status2 = Command::new("cargo")
            .args([
                "run",
                "--",
                "-d",
                SU(&temp_dir.path()),
                "-",
                &append_out_file,
                "-",
                "echo",
                "2ne line",
            ])
            .status()?;
        assert!(status2.code().unwrap() == 0);

        do_sync();

        let out_file_contents = fs::read_to_string(SU(&out_file))?;
        assert!(out_file_contents.find("1st line").is_some());
        assert!(out_file_contents.find("2nd line").is_none());

        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn overwrite_input_file() -> Result<(), io::Error> {
        const FILE_A: &str = "a.txt";

        let temp_dir = tempdir()?;

        let file_a = temp_dir.path().join(FILE_A);
        let _ = file_write(SU(&file_a), "file a.\n")?;

        let status = Command::new("cargo")
            .args([
                "run",
                "--",
                "-d",
                SU(&temp_dir.path()),
                SU(&file_a),
                "=",
                "-",
                "wc",
            ])
            .status()?;
        assert!(status.code().unwrap() == 0);

        do_sync();

        let file_a_contents = fs::read_to_string(SU(&file_a))?;
        assert!(file_a_contents.find("1").is_some());

        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn pipe_commands() -> Result<(), io::Error> {
        const FILE_A: &str = "a.txt";

        let temp_dir = tempdir()?;

        let file_a = temp_dir.path().join(FILE_A);
        let _ = file_write(SU(&file_a), "1st line\n2nd line\n3rd line\n")?;

        let output = Command::new("cargo")
            .args([
                "run",
                "--",
                "-d",
                SU(&temp_dir.path()),
                "-p",
                "P",
                "-",
                "-",
                "-",
                "cat",
                FILE_A,
                "P",
                "wc",
                "-l",
            ])
            .output()?;

        assert!(output.status.code().unwrap() == 0);

        do_sync();

        let output_contents = String::from_utf8(output.stdout).unwrap();
        assert!(output_contents.find("3\n").is_some());

        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn sequential_run_commands() -> Result<(), io::Error> {
        const FILE_A: &str = "a.txt";
        const FILE_B: &str = "b.txt";

        let temp_dir = tempdir()?;

        let file_a = temp_dir.path().join(FILE_A);
        let _ = file_write(SU(&file_a), "1st line\n2nd line\n3rd line\n")?;

        let output = Command::new("cargo")
            .args([
                "run",
                "--",
                "-d",
                SU(&temp_dir.path()),
                "-s",
                "S",
                "-",
                "-",
                "-",
                "cp",
                FILE_A,
                FILE_B,
                "S",
                "wc",
                "-l",
                FILE_B,
            ])
            .output()?;

        assert!(output.status.code().unwrap() == 0);

        do_sync();

        let output_contents = String::from_utf8(output.stdout).unwrap();
        assert!(output_contents.trim().starts_with("3"));

        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn sequential_run_commands_sub_oo_invalid_option() -> Result<(), io::Error> {
        const FILE_A: &str = "a.txt";

        let temp_dir = tempdir()?;

        let file_a = temp_dir.path().join(FILE_A);
        let _ = file_write(SU(&file_a), "1st line\n2nd line\n3rd line\n")?;

        let output = Command::new("cargo")
            .args([
                "run",
                "--",
                "-d",
                SU(&temp_dir.path()),
                "-s",
                "S",
                "-",
                "-",
                "-",
                "cat",
                FILE_A,
                "S",
                "o-o",
                "-s",
                "%%",
                "-",
                "-",
                "-",
            ])
            .output()?;

        assert!(output.status.code().unwrap() != 0);
        Ok(())
    }

    #[test]
    fn sub_oo_redirection() -> Result<(), io::Error> {
        const FILE_A: &str = "a.txt";
        const FILE_B: &str = "b.txt";

        let temp_dir = tempdir()?;
        let work_dir = SU(&temp_dir.path());

        let file_a_path = temp_dir.path().join(FILE_A);
        let _ = file_write(&file_a_path, "1st line\n2nd line\n3rd line\n")?;
        let file_b_path = temp_dir.path().join(FILE_B); // Keep as PathBuf

        let output = Command::new("./target/debug/o-o")
            .args([
                "-d",
                work_dir,
                "-s",
                "S",
                "-", // stdin for inner o-o for cp
                "-", // stdout for inner o-o for cp
                "-", // stderr for inner o-o for cp
                "cp",
                FILE_A, // cp's argument (relative to working dir)
                FILE_B, // cp's argument (relative to working dir)
                "S",
                // Second o-o command: run wc
                "o-o", // command
                "-d",
                work_dir,         // specify working directory for nested o-o
                SU(&file_b_path), // stdin for inner o-o for wc (reads b.txt)
                "-",              // stdout for inner o-o for wc (to overall stdout)
                "-",              // stderr for inner o-o for wc
                "wc",
                "-l", // argument for wc
            ])
            .output()?;

        do_sync();

        assert_eq!(
            output.status.code(),
            Some(0),
            "o-o command failed. Exit code: {:?}\nStdout: {}\nStderr: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let output_contents = String::from_utf8(output.stdout).unwrap();
        // Since we are reading from stdin (not wc -l <file_path>), the output should only be the line count.
        assert_eq!(
            output_contents.trim(),
            "3",
            "Expected output to be '3', but got: '{}'",
            output_contents.trim()
        );

        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn process_which_fails() -> Result<(), io::Error> {
        const SCRIPT_ECHO_AND_FAIL: &str = "echo_and_fail.sh";
        const FILE_A: &str = "a.txt";

        let temp_dir = tempdir()?;

        let script_echo_and_fail = temp_dir.path().join(SCRIPT_ECHO_AND_FAIL);
        let _ = file_write(
            SU(&script_echo_and_fail),
            "#!/bin/bash\n\necho \"echo and fail!\"\nexit 12\n",
        )?;

        let file_a = temp_dir.path().join(FILE_A);
        let _ = file_write(SU(&file_a), "file a original contents\n")?;

        let status = Command::new("cargo")
            .args([
                "run",
                "--",
                "-d",
                SU(&temp_dir.path()),
                SU(&file_a),
                "=",
                "-",
                "bash",
                SU(&script_echo_and_fail),
            ])
            .status()?;
        assert!(status.code().unwrap() != 0);

        do_sync();

        let file_a_contents = fs::read_to_string(SU(&file_a))?;
        assert!(file_a_contents.find("original contents").is_some());
        assert!(!file_a_contents.find("echo and fail!").is_some());

        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn overwrite_with_process_which_fails() -> Result<(), io::Error> {
        const SCRIPT_ECHO_AND_FAIL: &str = "echo_and_fail.sh";
        const FILE_A: &str = "a.txt";

        let temp_dir = tempdir()?;

        let script_echo_and_fail = temp_dir.path().join(SCRIPT_ECHO_AND_FAIL);
        let _ = file_write(
            SU(&script_echo_and_fail),
            "#!/bin/bash\n\necho \"echo and fail!\"\nexit 12\n",
        )?;

        let file_a = temp_dir.path().join(FILE_A);
        let _ = file_write(SU(&file_a), "file a original contents\n")?;

        let status = Command::new("cargo")
            .args([
                "run",
                "--",
                "-F",
                "-d",
                SU(&temp_dir.path()),
                SU(&file_a),
                "=",
                "-",
                "bash",
                SU(&script_echo_and_fail),
            ])
            .status()?;
        assert!(status.code().unwrap() != 0);

        do_sync();

        let file_a_contents = fs::read_to_string(SU(&file_a))?;
        assert!(!file_a_contents.find("original contents").is_some());
        assert!(file_a_contents.find("echo and fail!").is_some());

        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn environment_variable() -> Result<(), io::Error> {
        const SCRIPT: &str = "a_script.sh";

        let temp_dir = tempdir()?;

        let script = temp_dir.path().join(SCRIPT);
        let _ = file_write(SU(&script), "echo $V\n")?;

        let output = Command::new("cargo")
            .args([
                "run",
                "--",
                "-d",
                SU(&temp_dir.path()),
                "-e",
                "V=some",
                "-",
                "-",
                "-",
                "bash",
                SU(&script),
            ])
            .output()?;

        assert!(output.status.code().unwrap() == 0);

        do_sync();

        let output_contents = String::from_utf8(output.stdout).unwrap();
        assert!(output_contents.find("some").is_some());

        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn stdout_devnull() -> Result<(), io::Error> {
        let temp_dir = tempdir()?;

        let output = Command::new("cargo")
            .args([
                "run",
                "--",
                "-d",
                SU(&temp_dir.path()),
                "-",
                ".",
                "-",
                "echo",
                "hello",
            ])
            .output()?;

        assert!(output.status.code().unwrap() == 0);

        do_sync();

        let output_contents = String::from_utf8(output.stdout).unwrap();
        assert!(!output_contents.find("hello").is_some());

        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn stderr_devnull() -> Result<(), io::Error> {
        const SCRIPT: &str = "a_script.sh";

        let temp_dir = tempdir()?;

        let script = temp_dir.path().join(SCRIPT);
        let _ = file_write(
            SU(&script),
            "echo !!If you see this message, the test \"stderr_devnull\" failed.!! >&2\n",
        )?;

        let output = Command::new("cargo")
            .args([
                "run",
                "--",
                "-d",
                SU(&temp_dir.path()),
                "-",
                "-",
                ".",
                "bash",
                SU(&script),
            ])
            .output()?;

        assert!(output.status.code().unwrap() == 0);

        temp_dir.close()?;
        Ok(())
    }
}
