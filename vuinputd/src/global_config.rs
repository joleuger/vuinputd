// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use clap::{Parser, ValueEnum};
use std::sync::OnceLock;

// --- 1. Define the Global State Container ---
// This struct is extensible. You can add more global settings here later.
#[derive(Debug)]
pub struct GlobalConfig {
    pub policy: DevicePolicy,
}

// The actual static variable. It starts empty and is set once in main().
pub static CONFIG: OnceLock<GlobalConfig> = OnceLock::new();

// --- 2. Define the Policy Enum ---
#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum, Default)]
#[clap(rename_all = "kebab-case")] // This ensures StrictGamepad becomes "strict-gamepad"
pub enum DevicePolicy {
    /// Allow all device capabilities
    None,
    #[default]
    /// Default: Block SysRq
    MuteSysRq,
    /// Default: Allow keyboards/mice but block dangerous keys (SysRq, VT switching)
    Sanitized,
    /// Only allow Gamepad-like devices. Block mice and keyboards.
    StrictGamepad,
}

pub fn initialize_global_config(device_policy: &DevicePolicy) {
    if CONFIG
        .set(GlobalConfig {
            policy: device_policy.clone(),
        })
        .is_err()
    {
        eprintln!("Failed to initialize global config");
        std::process::exit(1);
    }
}

pub fn get_device_policy<'a>() -> &'a DevicePolicy {
    &CONFIG.get().unwrap().policy
}
