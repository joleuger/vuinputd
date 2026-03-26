// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use super::Device;
use libc::c_int;
use std::io;
use std::{ffi::CStr, fs::File};
use uinput_ioctls::*;

// PS4 Gamepad codes
const BTN_SOUTH: u16 = 304; // Cross
const BTN_EAST: u16 = 305; // Circle
const BTN_NORTH: u16 = 306; // Square
const BTN_WEST: u16 = 307; // Triangle
const BTN_TOP: u16 = 310; // L1
const BTN_TOP2: u16 = 311; // R1
const BTN_BASE: u16 = 312; // Share
const BTN_BASE2: u16 = 313; // Options
const BTN_BASE3: u16 = 314; // L3
const BTN_BASE23: u16 = 315; // R3
const BTN_TL: u16 = 316; // L2
const BTN_TR: u16 = 317; // R2
const BTN_SELECT: u16 = 318;
const BTN_START: u16 = 319;
const BTN_THUMBL: u16 = 320;
const BTN_THUMBR: u16 = 321;
const BTN_TOUCH: u16 = 322;
const BTN_TR2: u16 = 323;
const BTN_DPAD_UP: u16 = 325;
const BTN_DPAD_DOWN: u16 = 326;
const BTN_DPAD_LEFT: u16 = 327;
const BTN_DPAD_RIGHT: u16 = 328;

const ABS_X: u16 = 0;
const ABS_Y: u16 = 1;
const ABS_Z: u16 = 2;
const ABS_RX: u16 = 3;
const ABS_RY: u16 = 4;
const ABS_RZ: u16 = 5;
const ABS_THROTTLE: u16 = 6;
const ABS_RUDDER: u16 = 7;
const ABS_PRESSURE: u16 = 24;
const ABS_DISTANCE: u16 = 32;
const ABS_MT_POSITION_X: u16 = 47;
const ABS_MT_POSITION_Y: u16 = 48;
const ABS_MT_TRACKING_ID: u16 = 57;
const ABS_MT_PRESSURE: u16 = 47;
const ABS_MT_TOOL_TYPE: u16 = 55;
const ABS_MT_WIDTH: u16 = 56;

// Xbox Gamepad codes
const BTN_A: u16 = 304; // A
const BTN_B: u16 = 305; // B
const BTN_X: u16 = 306; // X
const BTN_Y: u16 = 307; // Y
const BTN_TL2: u16 = 319; // LT

pub struct Ps4GamepadDevice;

/// Setup PS4 gamepad device
unsafe fn setup_ps4_gamepad(fd: c_int) -> io::Result<()> {
    // EV_SYN
    ui_set_evbit(fd, super::EV_SYN.try_into().unwrap())?;
    // EV_KEY
    ui_set_evbit(fd, super::EV_KEY.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_SOUTH.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_EAST.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_NORTH.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_WEST.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_TOP.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_TOP2.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_BASE.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_BASE2.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_BASE3.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_BASE23.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_TL.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_TR.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_SELECT.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_START.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_THUMBL.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_THUMBR.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_TOUCH.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_TR2.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_DPAD_UP.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_DPAD_DOWN.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_DPAD_LEFT.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_DPAD_RIGHT.try_into().unwrap())?;
    // EV_ABS
    ui_set_evbit(fd, super::EV_ABS.try_into().unwrap())?;
    ui_set_absbit(fd, ABS_X.try_into().unwrap())?;
    ui_set_absbit(fd, ABS_Y.try_into().unwrap())?;
    ui_set_absbit(fd, ABS_RX.try_into().unwrap())?;
    ui_set_absbit(fd, ABS_RY.try_into().unwrap())?;
    ui_set_absbit(fd, ABS_PRESSURE.try_into().unwrap())?;
    ui_set_absbit(fd, ABS_DISTANCE.try_into().unwrap())?;

    Ok(())
}

impl Device for Ps4GamepadDevice {
    fn name() -> &'static str {
        "PS4 Gamepad"
    }

    fn get_event_device(sysname: &str) -> Result<File, io::Error> {
        super::utils::fetch_device_node(sysname).and_then(|devnode| File::open(&devnode))
    }

    fn setup(device: Option<&str>, name: &str) -> Result<i32, io::Error> {
        let fd = super::utils::open_uinput(device)?;
        unsafe { setup_ps4_gamepad(fd) }?;

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
