use std::fs::{File, OpenOptions};
use std::path::PathBuf;

use anyhow::{Context, Result};

use duct::cmd;
use tempfile::{NamedTempFile, Builder};

#[cfg(not(windows))]
pub fn command_exists(cmd: &str) -> bool {
    let output = cmd!("which", cmd)
        .read()
        .unwrap_or_else(|_| String::new());

    !output.trim().is_empty()
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

    let file = options.open(clean_path)
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
