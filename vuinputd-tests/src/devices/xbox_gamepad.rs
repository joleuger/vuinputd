// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use crate::devices::device_base::{fetch_device_node, open_uinput, Device, DeviceState, BUS_USB};
use libc::{c_int, close, open};
use std::io;
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
    let abs_info_dpad = libc::input_absinfo {
        value: 0,
        minimum: -1,
        maximum: 1,
        fuzz: 0,
        flat: 0,
        resolution: 0,
    };
    ui_abs_setup(
        fd,
        &libc::uinput_abs_setup {
            code: ABS_HAT0Y,
            absinfo: abs_info_dpad.clone(),
        },
    )?;
    ui_abs_setup(
        fd,
        &libc::uinput_abs_setup {
            code: ABS_HAT0X,
            absinfo: abs_info_dpad.clone(),
        },
    )?;
    // EV_ABS stick
    let abs_info_stick = libc::input_absinfo {
        value: 0,
        minimum: -32768,
        maximum: 32767,
        fuzz: 16,
        flat: 128,
        resolution: 0,
    };
    ui_abs_setup(
        fd,
        &libc::uinput_abs_setup {
            code: ABS_X,
            absinfo: abs_info_stick.clone(),
        },
    )?;
    ui_abs_setup(
        fd,
        &libc::uinput_abs_setup {
            code: ABS_RX,
            absinfo: abs_info_stick.clone(),
        },
    )?;
    ui_abs_setup(
        fd,
        &libc::uinput_abs_setup {
            code: ABS_Y,
            absinfo: abs_info_stick.clone(),
        },
    )?;
    ui_abs_setup(
        fd,
        &libc::uinput_abs_setup {
            code: ABS_RY,
            absinfo: abs_info_stick.clone(),
        },
    )?;
    // EV_ABS trigger
    let abs_info_trigger = libc::input_absinfo {
        value: 0,
        minimum: 0,
        maximum: 255,
        fuzz: 0,
        flat: 0,
        resolution: 0,
    };
    ui_abs_setup(
        fd,
        &libc::uinput_abs_setup {
            code: ABS_Z,
            absinfo: abs_info_trigger.clone(),
        },
    )?;
    ui_abs_setup(
        fd,
        &libc::uinput_abs_setup {
            code: ABS_RZ,
            absinfo: abs_info_trigger.clone(),
        },
    )?;

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

pub struct XboxGamepadDevice {
    state: DeviceState,
}

impl Device for XboxGamepadDevice {
    fn name() -> &'static str {
        "Xbox Gamepad"
    }

    fn state(&self) -> &DeviceState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut DeviceState {
        &mut self.state
    }

    fn get_event_device(&self) -> Result<c_int, io::Error> {
        Ok(self.state.event_device_fd)
    }

    fn create(device: Option<&str>, name: &str) -> Result<Self, io::Error> {
        let fd = open_uinput(device)?;

        unsafe { setup_xbox_gamepad(fd)? };

        let temp_device = XboxGamepadDevice {
            state: DeviceState {
                uinput_fd: fd,
                sysname: String::new(),
                device_name: name.to_string(),
                event_device_node: String::new(),
                event_device_fd: -1,
                events: Vec::new(),
            },
        };
        temp_device.setup_device(name, 0xbeef, 0xdead, BUS_USB)?;

        unsafe {
            ui_dev_create(fd).map_err(|e| {
                eprintln!("ui_dev_create failed: {:?}", e);
                e
            })?;
        }

        let sysname = temp_device.get_sysname()?;

        let event_device_node = fetch_device_node(&sysname)?;
        let event_device_fd = unsafe {
            open(
                event_device_node.as_ptr() as *const i8,
                libc::O_RDONLY | libc::O_NONBLOCK,
            )
        };
        if event_device_fd < 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(XboxGamepadDevice {
            state: DeviceState {
                uinput_fd: fd,
                sysname,
                device_name: name.to_string(),
                event_device_node,
                event_device_fd,
                events: Vec::new(),
            },
        })
    }

    fn destroy(self) {
        unsafe {
            ui_dev_destroy(self.state.uinput_fd).unwrap_or_else(|e| {
                eprintln!("ui_dev_destroy failed: {:?}", e);
                std::process::exit(1);
            });
            close(self.state.uinput_fd);
            close(self.state.event_device_fd);
        }
    }
}
