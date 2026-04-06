// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

pub mod basic_keyboard;
pub mod basic_mouse;
pub mod basic_mouse_absolute;
pub mod basic_ps4_gamepad;
pub mod basic_xbox_gamepad;
pub mod ff_xbox_gamepad;
/*
pub mod reuse_keyboard;
pub mod reuse_xbox_gamepad;
pub mod stress_keyboard;
pub mod stress_xbox_gamepad;
*/

// Re-exports for type checking
pub use basic_keyboard::BasicKeyboard;
pub use basic_mouse::BasicMouse;
pub use basic_mouse_absolute::BasicMouseAbsolute;
pub use basic_ps4_gamepad::BasicPs4Gamepad;
pub use basic_xbox_gamepad::BasicXboxGamepad;
pub use ff_xbox_gamepad::FfXboxGamepad;
/*
pub use reuse_keyboard::ReuseKeyboard;
pub use reuse_xbox_gamepad::ReuseXboxGamepad;
pub use stress_keyboard::StressKeyboard;
pub use stress_xbox_gamepad::StressXboxGamepad;
 */

/// Common scenario arguments passed from CLI
#[derive(Debug, Clone)]
pub struct ScenarioArgs {
    pub ipc: bool,
    pub dev_path: Option<String>,
}
