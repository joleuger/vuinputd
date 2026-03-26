// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::fs::File;
use std::io;

pub mod keyboard;
pub mod mouse;
pub mod ps4_gamepad;
pub mod xbox_gamepad;
pub mod utils;

pub use keyboard::KeyboardDevice;
pub use mouse::MouseDevice;
pub use ps4_gamepad::Ps4GamepadDevice;
pub use xbox_gamepad::XboxGamepadDevice;

// Constants (same numeric values as in linux headers)
pub const EV_SYN: u16 = 0x00;
pub const EV_KEY: u16 = 0x01;
pub const EV_REL: u16 = 0x02;
pub const EV_ABS: u16 = 0x03;

/// Trait for input devices
pub trait Device {
    fn name() -> &'static str;
    fn get_event_device(sysname: &str) -> Result<File, io::Error>;
    
    /// Phase 1: open uinput, configure keys, call ui_dev_setup
    fn setup(device:Option<&str>,name: &str) -> Result<i32, io::Error>;
    
    /// Phase 2: call ui_dev_create, get sysname and devnode
    fn create(fd: i32) -> Result<String, io::Error>;
    
    /// Phase 3: call ui_dev_destroy and close fd
    fn destroy(fd: i32);
}