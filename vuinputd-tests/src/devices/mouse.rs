// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use super::Device;
use libc::c_int;
use std::io;
use std::{ffi::CStr, fs::File};
use uinput_ioctls::*;

// Mouse codes
pub const BTN_LEFT: u16 = 272;
pub const BTN_RIGHT: u16 = 273;
pub const BTN_MIDDLE: u16 = 274;
pub const REL_X: u16 = 0;
pub const REL_Y: u16 = 1;

/// Setup mouse device
unsafe fn setup_mouse(fd: c_int) -> io::Result<()> {
    // EV_SYN
    ui_set_evbit(fd, super::EV_SYN.try_into().unwrap())?;
    // EV_KEY
    ui_set_evbit(fd, super::EV_KEY.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_LEFT.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_RIGHT.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_MIDDLE.try_into().unwrap())?;
    // EV_REL
    ui_set_evbit(fd, super::EV_REL.try_into().unwrap())?;
    ui_set_relbit(fd, REL_X.try_into().unwrap())?;
    ui_set_relbit(fd, REL_Y.try_into().unwrap())?;

    Ok(())
}

pub struct MouseDevice;

impl Device for MouseDevice {
    fn name() -> &'static str {
        "Mouse"
    }

    fn get_event_device(sysname: &str) -> Result<File, io::Error> {
        super::utils::fetch_device_node(sysname).and_then(|devnode| File::open(&devnode))
    }

    fn setup(device: Option<&str>, name: &str) -> Result<i32, io::Error> {
        let fd = super::utils::open_uinput(device)?;
        unsafe { setup_mouse(fd) }?;

        unsafe {
            let mut usetup: libc::uinput_setup = std::mem::zeroed();
            usetup.id.bustype = BUS_USB;
            usetup.id.vendor = 0xbeef;
            usetup.id.product = 0xdead;

            let name_cstr = CString::new(name).unwrap();
            let name_ptr = usetup.name.as_mut_ptr() as *mut c_char;
            std::ptr::copy_nonoverlapping(
                name_cstr.as_ptr(),
                name_ptr,
                name_cstr.to_bytes_with_nul().len(),
            );

            let usetup_ptr = &mut usetup as *mut libc::uinput_setup;
            ui_dev_setup(fd, usetup_ptr).map_err(|e| {
                eprintln!("ui_dev_setup failed: {:?}", e);
                e
            })?;
        }

        Ok(fd)
    }

    fn create(fd: i32) -> Result<String, io::Error> {
        unsafe {
            ui_dev_create(fd).map_err(|e| {
                eprintln!("ui_dev_create failed: {:?}", e);
                e
            })?;

            let mut resultbuf: [c_char; 64] = [0; 64];
            ui_get_sysname(fd, resultbuf.as_mut_slice()).map_err(|e| {
                eprintln!("ui_get_sysname failed: {:?}", e);
                e
            })?;

            let sysname = format!(
                "{}{}",
                SYS_INPUT_DIR,
                CStr::from_ptr(resultbuf.as_ptr()).to_string_lossy()
            );

            Ok(sysname)
        }
    }

    fn destroy(fd: i32) {
        unsafe {
            ui_dev_destroy(fd).unwrap_or_else(|e| {
                eprintln!("ui_dev_destroy failed: {:?}", e);
                std::process::exit(1);
            });
            close(fd);
        }
    }
}

use libc::{c_char, close};
use std::ffi::CString;

const SYS_INPUT_DIR: &str = "/sys/devices/virtual/input/";
const BUS_USB: u16 = 0x03;
