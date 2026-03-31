// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use crate::devices::device_base::{fetch_device_node, open_uinput, Device, DeviceState, BUS_USB};
use libc::{c_int, close, open};
use std::io;
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

pub struct MouseDevice {
    state: DeviceState,
}

impl Device for MouseDevice {
    fn name() -> &'static str {
        "Mouse"
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

        unsafe { setup_mouse(fd)? };

        let temp_device = MouseDevice {
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

        Ok(MouseDevice {
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
