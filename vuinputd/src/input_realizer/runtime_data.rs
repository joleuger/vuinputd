// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

use log::{info, warn};

/// Ensure required udev directories and files exist
pub fn ensure_udev_structure() -> io::Result<()> {
    // Note that this structure _must_ exist, before a service using libinput is run. The time of device creation might be too late.

    let data_dir = format!("/run/udev/data");
    let data_dir = Path::new(&data_dir);
    let control_file = format!("/run/udev/control");
    let control_file = Path::new(&control_file);

    // Create directory like `mkdir -p`
    if !data_dir.exists() {
        fs::create_dir_all(data_dir)?;
    }

    // Ensure /run/udev/control exists, create empty if not
    if !control_file.exists() {
        warn!(
            "VUI-UDEV-001 — /run/udev/control/ not available. Keyboard or mouse might be unusable."
        );
        warn!("Visit https://github.com/joleuger/vuinputd/blob/main/docs/TROUBLESHOOTING.md for details");
        info!("Creating file /run/udev/control anyway for subsequent runs.");
        File::create(control_file)?;
    }

    Ok(())
}

/// Write udev data entry for a given major/minor number
/// - `content` = original udev data text
/// - `major`, `minor` = device numbers
///
/// Performs these transforms:
///  - remove all lines containing `ID_SEAT=`
///  - remove all lines containing `seat_` references (G:, Q: lines)
///  - replace ID_VUINPUT_* with ID_INPUT_*
///  - write updated content to `/run/udev/data/c<major>:<minor>`
pub fn write_udev_data(path_prefix: &str, content: &str, major: u64, minor: u64) -> io::Result<()> {
    let mut cleaned = String::new();

    for line in content.lines() {
        // skip seat-related lines
        if line.contains("ID_SEAT=") || line.contains("seat_") {
            continue;
        }

        // perform replacements
        let line = line
            .replace("ID_VUINPUT_KEYBOARD=1", "ID_INPUT_KEYBOARD=1")
            .replace("ID_VUINPUT_MOUSE=1", "ID_INPUT_MOUSE=1");

        cleaned.push_str(&line);
        cleaned.push('\n');
    }

    let path = format!("{}/udev/data/c{}:{}", path_prefix, major, minor);
    let mut file = File::create(&path)?;
    file.write_all(cleaned.as_bytes())?;

    Ok(())
}

/// Delete udev data for a given major/minor number
/// - `major`, `minor` = device numbers
pub fn delete_udev_data(path_prefix: &str, major: u64, minor: u64) -> io::Result<()> {
    let path = format!("{}/udev/data/c{}:{}", path_prefix, major, minor);
    fs::remove_file(&path)?;
    Ok(())
}

pub fn read_udev_data(major: u64, minor: u64) -> io::Result<String> {
    let path = format!("/run/udev/data/c{}:{}", major, minor);
    fs::read_to_string(path)
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_replacement_and_filter() {
        let input = r#"I:16429403327735
E:ID_VUINPUT_KEYBOARD=1
E:ID_INPUT=1
E:ID_INPUT_KEY=1
E:ID_SERIAL=noserial
E:ID_SEAT=seat_vuinput
G:seat_vuinput
G:power-switch
Q:seat_vuinput
Q:power-switch
V:1"#;

        let expected = r#"I:16429403327735
E:ID_INPUT_KEYBOARD=1
E:ID_INPUT=1
E:ID_INPUT_KEY=1
E:ID_SERIAL=noserial
G:power-switch
Q:power-switch
V:1
"#;

        let mut cleaned = String::new();
        for line in input.lines() {
            if line.contains("ID_SEAT=") || line.contains("seat_") {
                continue;
            }
            let line = line
                .replace("ID_VUINPUT_KEYBOARD=1", "ID_INPUT_KEYBOARD=1")
                .replace("ID_VUINPUT_MOUSE=1", "ID_INPUT_MOUSE=1");
            cleaned.push_str(&line);
            cleaned.push('\n');
        }

        assert_eq!(cleaned, expected);
    }
}
