// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use super::Device;
use libc::c_int;
use std::io;
use std::{ffi::CStr, fs::File};
use uinput_ioctls::*;

// Xbox Gamepad codes
// https://github.com/torvalds/linux/blob/master/Documentation/input/gamepad.rst
// https://github.com/torvalds/linux/blob/master/include/uapi/linux/input-event-codes.h// Keys and Buttons
// https://github.com/torvalds/linux/blob/master/drivers/input/joystick/xpad.c
pub const BTN_SOUTH: u16 = 0x130;
pub const BTN_EAST: u16 = 0x131;
pub const BTN_NORTH: u16 = 0x133;
pub const BTN_WEST: u16 = 0x134;
pub const BTN_TL: u16 = 0x136;
pub const BTN_TR: u16 = 0x137;
pub const BTN_SELECT: u16 = 0x13a;
pub const BTN_START: u16 = 0x13b;
pub const BTN_MODE: u16 = 0x13c;
pub const BTN_THUMBL: u16 = 0x13d;
pub const BTN_THUMBR: u16 = 0x13e;

// Absolute Axes
pub const ABS_X: u16 = 0x00;
pub const ABS_Y: u16 = 0x01;
pub const ABS_Z: u16 = 0x02;
pub const ABS_RX: u16 = 0x03;
pub const ABS_RY: u16 = 0x04;
pub const ABS_RZ: u16 = 0x05;
pub const ABS_HAT0X: u16 = 0x10;
pub const ABS_HAT0Y: u16 = 0x11;

// Force Feedback
// https://github.com/torvalds/linux/blob/master/include/uapi/linux/input.h
pub const FF_RUMBLE: u16 = 0x50;
pub const FF_PERIODIC: u16 = 0x51;
pub const FF_CONSTANT: u16 = 0x52;
pub const FF_RAMP: u16 = 0x57;
pub const FF_SINE: u16 = 0x5a;
pub const FF_GAIN: u16 = 0x60;

/// Setup Xbox gamepad device
/// https://github.com/LizardByte/Sunshine/blob/master/src/platform/linux/input/inputtino_gamepad.cpp
/// https://github.com/games-on-whales/inputtino/blob/stable/src/uinput/joypad_xbox.cpp
unsafe fn setup_xbox_gamepad(fd: c_int) -> io::Result<()> {
    // EV_SYN
    ui_set_evbit(fd, super::EV_SYN.try_into().unwrap())?;

    // EV_KEY
    ui_set_evbit(fd, super::EV_KEY.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_WEST.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_EAST.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_NORTH.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_SOUTH.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_THUMBL.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_THUMBR.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_TR.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_TL.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_SELECT.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_MODE.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_START.try_into().unwrap())?;

    // EV_ABS
    ui_set_evbit(fd, super::EV_ABS.try_into().unwrap())?;
    // EV_ABS dpad
    let abs_info_dpad = libc::input_absinfo { value: 0, minimum: -1, maximum: 1, fuzz: 0, flat: 0, resolution: 0 };
    ui_abs_setup(fd, &libc::uinput_abs_setup { code: ABS_HAT0Y, absinfo: abs_info_dpad.clone() })?;
    ui_abs_setup(fd, &libc::uinput_abs_setup { code: ABS_HAT0X, absinfo: abs_info_dpad.clone() })?;
    // EV_ABS stick
    let abs_info_stick = libc::input_absinfo { value: 0, minimum: -32768, maximum: 32767, fuzz: 16, flat: 128, resolution: 0 };
    ui_abs_setup(fd, &libc::uinput_abs_setup { code: ABS_X, absinfo: abs_info_stick.clone() })?;
    ui_abs_setup(fd, &libc::uinput_abs_setup { code: ABS_RX, absinfo: abs_info_stick.clone() })?;
    ui_abs_setup(fd, &libc::uinput_abs_setup { code: ABS_Y, absinfo: abs_info_stick.clone() })?;
    ui_abs_setup(fd, &libc::uinput_abs_setup { code: ABS_RY, absinfo: abs_info_stick.clone() })?;
    // EV_ABS trigger
    let abs_info_trigger = libc::input_absinfo { value: 0, minimum: 0, maximum: 255, fuzz: 0, flat: 0, resolution: 0 };
    ui_abs_setup(fd, &libc::uinput_abs_setup { code: ABS_Z, absinfo: abs_info_trigger.clone() })?;
    ui_abs_setup(fd, &libc::uinput_abs_setup { code: ABS_RZ, absinfo: abs_info_trigger.clone() })?;

    // EV_FF
    ui_set_evbit(fd, super::EV_FF.try_into().unwrap())?;
    ui_set_ffbit(fd, FF_RUMBLE.try_into().unwrap())?;
    ui_set_ffbit(fd, FF_CONSTANT.try_into().unwrap())?;
    ui_set_ffbit(fd, FF_PERIODIC.try_into().unwrap())?;
    ui_set_ffbit(fd, FF_SINE.try_into().unwrap())?;
    ui_set_ffbit(fd, FF_RAMP.try_into().unwrap())?;
    ui_set_ffbit(fd, FF_GAIN.try_into().unwrap())?;

    Ok(())
}

pub struct XboxGamepadDevice;

impl Device for XboxGamepadDevice {
    fn name() -> &'static str {
        "Xbox Gamepad"
    }

    fn get_event_device(sysname: &str) -> Result<File, io::Error> {
        super::utils::fetch_device_node(sysname).and_then(|devnode| File::open(&devnode))
    }

    fn setup(device: Option<&str>, name: &str) -> Result<i32, io::Error> {
        let fd = super::utils::open_uinput(device)?;
        unsafe { setup_xbox_gamepad(fd) }?;

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

pub const SYS_INPUT_DIR: &str = "/sys/devices/virtual/input/";
pub const BUS_USB: u16 = 0x03;
