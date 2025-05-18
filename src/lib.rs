use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use thiserror::Error;

use duct::cmd;
use tempfile::{Builder, NamedTempFile};

#[cfg(not(windows))]
pub fn command_exists(cmd: &str) -> bool {
    let output = cmd!("which", cmd).read().unwrap_or_else(|_| String::new());

    !output.trim().is_empty()
}

pub fn do_sync() {
    if let Err(e) = Command::new("sync").status() {
        eprintln!("o-o: warning: failed to execute sync command: {}", e);
    }
}

pub fn open_file_with_mode(path: &str) -> Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create(true);

    let (mode, clean_path) = if let Some(s) = path.strip_prefix('+') {
        (true, s)
    } else {
        (false, path)
    };

    if mode {
        options.append(true);
    } else {
        options.truncate(true);
    }

    let file = options
        .open(clean_path)
        .with_context(|| format!("Failed to open file: {}", clean_path))?;

    Ok(file)
}

pub fn create_temp_file(tempdir_placeholder: &Option<&str>) -> Result<PathBuf> {
    let temp_file = if let Some(dir) = tempdir_placeholder {
        Builder::new().prefix("tempfile").tempfile_in(dir)?
    } else {
        NamedTempFile::new()?
    };

    Ok(temp_file.path().to_path_buf())
}

#[derive(Error, Debug)]
pub enum FileIOError {
    #[error("Write failed for file '{0}' after {1} miliseconds")]
    WriteFailedError(String, u64),
}

pub fn wait_for_file_existence_with_mode(file_path: &str, timeout: u64) -> Result<(), FileIOError> {
    let (_mode, clean_path) = if let Some(s) = file_path.strip_prefix('+') {
        (true, s)
    } else {
        (false, file_path)
    };

    let start = std::time::Instant::now();
    let path = Path::new(clean_path);

    while !path.exists() {
        let elapsed = start.elapsed().as_millis() as u64;
        if elapsed >= timeout {
            return Err(FileIOError::WriteFailedError(
                path.to_string_lossy().to_string(),
                timeout,
            ));
        }
        thread::sleep(Duration::from_millis(100));
    }

    Ok(())
}
