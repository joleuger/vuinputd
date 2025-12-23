// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::fs::OpenOptions;
use std::io;
use std::os::unix::io::AsRawFd;
use log::{error, info, warn};

use libc::ioctl;

// see include/uapi/linux/kd.h
const KDSKBMODE: u64 = 0x4B45; // sets current keyboard mode
const KDGKBMODE: u64 = 0x4B44; // gets current keyboard mode

const K_OFF: u64 = 0x04;

pub fn check_vt_status() {
    match OpenOptions::new().read(true).open("/dev/tty1") {
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            info!("/dev/tty1 not present — no VT-related input problem");
        }
        Err(err) => {
            error!("failed to open /dev/tty1: {}", err);
        }
        Ok(tty) => {
            let fd = tty.as_raw_fd();
            let mut mode: u64 = 0;

            let rc = unsafe { ioctl(fd, KDGKBMODE, &mut mode) };
            if rc < 0 {
                error!(
                    "KDGKBMODE ioctl failed: {}",
                    io::Error::last_os_error()
                );
                return;
            }

            if mode == K_OFF {
                info!("tty1 keyboard mode is K_OFF — VT input is disabled");
            } else {
                warn!(
                    "tty1 keyboard mode is active (mode={}) — VT may consume input",
                    mode
                );
            }
        }
    }
}

pub fn mute_keyboard() -> std::io::Result<()> {
    // 1. Open the TTY (usually TTY1 for a DM)
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty1")?;
    let fd = file.as_raw_fd();

    // 2. Mute the keyboard
    unsafe {
        if libc::ioctl(fd, KDSKBMODE, K_OFF) < 0 {
            panic!("Failed to mute keyboard. Are you root?");
        }
    }
    println!("Keyboard muted.");
    Ok(())
}
