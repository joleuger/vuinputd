// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::io::{self, BufRead};
use std::{
    fs::{self, File},
    path::Path,
};

/// Ensure required dev-input, udev directories and files exist
pub fn ensure_host_fs_structure(path_prefix: &str) -> io::Result<()> {
    let _ = check_if_path_allows_char_devs(&path_prefix);
    let dev_input_dir = format!("{}/dev-input", path_prefix);
    let dev_input_dir = Path::new(&dev_input_dir);
    // Create directory like `mkdir -p`
    if !dev_input_dir.exists() {
        fs::create_dir_all(dev_input_dir)?;
    }

    // Note that this structure _must_ exist, before a service using libinput is run.
    let data_dir = format!("{}/udev/data", path_prefix);
    let data_dir = Path::new(&data_dir);
    // Create directory like `mkdir -p`
    if !data_dir.exists() {
        fs::create_dir_all(data_dir)?;
    }

    let control_file = format!("{}/udev/control", path_prefix);
    let control_file = Path::new(&control_file);
    // Ensure /run/udev/control exists, create empty if not
    if !control_file.exists() {
        File::create(control_file)?;
    }

    Ok(())
}

/// simple heuristic that checks whether path_prefix allows the hosting of character devices
/// This heuristic is not 100%, but a simple indicator
pub fn check_if_path_allows_char_devs(path: &str) -> io::Result<()> {
    let file = File::open("/proc/self/mountinfo")?;
    let reader = io::BufReader::new(file);

    for line in reader.lines() {
        let line = line?;

        let (left, _) = match line.split_once(" - ") {
            Some(v) => v,
            None => continue,
        };

        let fields: Vec<&str> = left.split_whitespace().collect();

        // mount point is field 5
        let mount_point = fields.get(4).copied().unwrap_or("");
        // mount options are field 6
        let options = fields.get(5).copied().unwrap_or("");

        if mount_point.contains(path) {
            if options.split(',').any(|o| o == "nodev") {
                log::warn!(
                    "mount {} is present but mounted with nodev; device nodes will not work",
                    path
                );
            } else {
                log::info!("mount {} is present and allows device nodes", path);
            }

            return Ok(());
        }
    }

    log::warn!(
        "expected mount {} not found; user likely forgot to mount tmpfs with dev-option on it",
        path
    );

    Ok(())
}
