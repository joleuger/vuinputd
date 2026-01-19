// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use clap::{Parser, ValueEnum};
use std::sync::OnceLock;

#[derive(Debug)]
pub struct GlobalConfig {
    pub policy: DevicePolicy,
    pub placement: Placement,
}

// The actual static variable. It starts empty and is set once in main().
pub static CONFIG: OnceLock<GlobalConfig> = OnceLock::new();

/// The device policy decides what events stay and what is filtered out.
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
/// Where to create runtime artifacts (device nodes + udev data)
#[derive(Debug, Clone, ValueEnum, Default)]
pub enum Placement {
    #[default]
    /// Create inside the container
    Inject,
    /// Create on the host (user is expected to bind-mount)
    Host,
    /// Do not create any artifacts
    None,
}

pub fn initialize_global_config(device_policy: &DevicePolicy, placement: &Placement) {
    if CONFIG
        .set(GlobalConfig {
            policy: device_policy.clone(),
            placement: placement.clone(),
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

pub fn get_placement<'a>() -> &'a Placement {
    &CONFIG.get().unwrap().placement
}
