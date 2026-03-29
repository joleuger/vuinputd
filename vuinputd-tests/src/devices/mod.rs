// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

pub mod device_base;
pub mod keyboard;
pub mod mouse;
pub mod ps4_gamepad;
pub mod utils;
pub mod xbox_gamepad;

pub use device_base::Device;
// Keep DeviceState exported for backward compatibility
pub use device_base::DeviceState;
pub use keyboard::KeyboardDevice;
pub use mouse::MouseDevice;
pub use ps4_gamepad::Ps4GamepadDevice;
pub use xbox_gamepad::XboxGamepadDevice;

// Re-export constants from device_base for backward compatibility
pub use device_base::{BUS_USB, EV_ABS, EV_FF, EV_KEY, EV_REL, EV_SYN, SYN_REPORT, SYS_INPUT_DIR};
