// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

// this file is ai genrated and contains many mistake that need to be fixed manually

use crate::devices::device_base::{fetch_device_node, open_uinput, Device, DeviceState, BUS_USB};
use libc::{c_int, close, open};
use std::io;
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

pub struct Ps4GamepadDevice {
    state: DeviceState,
}

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

        unsafe { setup_ps4_gamepad(fd)? };

        let temp_device = Ps4GamepadDevice {
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

        Ok(Ps4GamepadDevice {
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
