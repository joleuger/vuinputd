// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use crate::devices::{
    device_base::{fetch_device_node, open_uinput, Device, DeviceState, BUS_USB},
    utils::{ABS_X, ABS_Y},
};
use libc::{c_int, close, input_absinfo, open, uinput_abs_setup, INPUT_PROP_DIRECT};
use std::io;
use uinput_ioctls::*;

// Mouse codes
pub const BTN_LEFT: u16 = 272;
pub const BTN_RIGHT: u16 = 273;
pub const BTN_MIDDLE: u16 = 274;
pub const REL_X: u16 = 0;
pub const REL_Y: u16 = 1;

// non linux constants used this way in inputtino
pub const ABS_MAX_WIDTH: i32 = 19200;
pub const ABS_MAX_HEIGHT: i32 = 12000;

/// Setup absolute mouse device
unsafe fn setup_mouse_absolute(fd: c_int) -> io::Result<()> {
    // EV_SYN (implicitly handled by libevdev, but required manually for uinput)
    ui_set_evbit(fd, super::EV_SYN.try_into().unwrap())?;

    // INPUT_PROP_DIRECT
    ui_set_propbit(fd, INPUT_PROP_DIRECT.try_into().unwrap())?;

    // EV_KEY
    ui_set_evbit(fd, super::EV_KEY.try_into().unwrap())?;
    ui_set_keybit(fd, BTN_LEFT.try_into().unwrap())?;

    // EV_ABS
    ui_set_evbit(fd, super::EV_ABS.try_into().unwrap())?;
    ui_set_absbit(fd, ABS_X.try_into().unwrap())?;
    ui_set_absbit(fd, ABS_Y.try_into().unwrap())?;

    // Setup absolute axis parameters (min, max, fuzz, flat, resolution)
    let abs_x_setup = uinput_abs_setup {
        code: ABS_X,
        absinfo: input_absinfo {
            value: 0,
            minimum: 0,
            maximum: ABS_MAX_WIDTH,
            fuzz: 1,
            flat: 0,
            resolution: 28,
        },
    };
    ui_abs_setup(fd, &abs_x_setup).map_err(|e| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("ui_abs_setup X failed: {:?}", e),
        )
    })?;

    let abs_y_setup = uinput_abs_setup {
        code: ABS_Y,
        absinfo: input_absinfo {
            value: 0,
            minimum: 0,
            maximum: ABS_MAX_HEIGHT,
            fuzz: 1,
            flat: 0,
            resolution: 28,
        },
    };
    ui_abs_setup(fd, &abs_y_setup).map_err(|e| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("ui_abs_setup Y failed: {:?}", e),
        )
    })?;

    Ok(())
}

pub struct MouseAbsoluteDevice {
    state: DeviceState,
}

impl Device for MouseAbsoluteDevice {
    fn name() -> &'static str {
        "Mouse Absolute"
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

        unsafe { setup_mouse_absolute(fd)? };

        let temp_device = MouseAbsoluteDevice {
            state: DeviceState {
                uinput_fd: fd,
                sysname: String::new(),
                device_name: name.to_string(),
                event_device_node: String::new(),
                event_device_fd: -1,
                events: Vec::new(),
            },
        };
        temp_device.setup_device(name, 0xbeef, 0xdead, BUS_USB, 0)?;

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

        Ok(MouseAbsoluteDevice {
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
