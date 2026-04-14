// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use clap::ValueEnum;
use std::sync::OnceLock;

use crate::container_runtime::ContainerRuntime;

#[derive(Debug)]
pub struct GlobalConfig {
    pub policy: DevicePolicy,
    pub container_runtime: ContainerRuntime,
    pub vudevname: String,
    pub device_owner: DeviceOwner,
    pub scope: Scope,
}

// The actual static variable. It starts empty and is set once in main().
pub static CONFIG: OnceLock<GlobalConfig> = OnceLock::new();

/// Defines the operational scope of the vuinputd instance
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Scope {
    #[default]
    /// Watch all running containers of the configured runtime and manage lifecycle.
    Multi,
    /// Bind to a single named container. The name is passed directly to the engine's CLI/API.
    Single(String),
}

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
/// Deprecated, use --container-runtime instead. Currently just maps to
/// --container-runtime
#[derive(Debug, Clone, ValueEnum, Default, PartialEq, Eq)]
pub enum Placement {
    #[default]
    /// Create inside the container
    InContainer,
    /// Create on the host (user is expected to bind-mount)
    OnHost,
    /// Do not create any artifacts (netlink message in container is unaffected)
    None,
}

/// Device owner of the created devices
#[derive(Debug, Clone, ValueEnum, Default, PartialEq, Eq)]
pub enum DeviceOwner {
    #[default]
    /// Automatically derive useful settings (how might change in the future)
    Auto,
    /// Use the uid and gid of vuinputd
    Vuinputd,
    /// Same as dev folder in container
    ContainerDevFolder,
}

impl DeviceOwner {
    pub fn to_string_rep(&self) -> String {
        match self {
            DeviceOwner::Auto => "auto".to_string(),
            DeviceOwner::Vuinputd => "vuinputd".to_string(),
            DeviceOwner::ContainerDevFolder => "container-dev-folder".to_string(),
        }
    }
}

pub fn initialize_global_config(
    device_policy: &DevicePolicy,
    container_runtime: &ContainerRuntime,
    devname: &Option<String>,
    device_owner: &DeviceOwner,
    scope: &Scope,
) {
    if CONFIG
        .set(GlobalConfig {
            policy: device_policy.clone(),
            container_runtime: container_runtime.clone(),
            vudevname: devname.clone().unwrap_or("vuinput".to_string()),
            device_owner: device_owner.clone(),
            scope: scope.clone(),
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

pub fn get_container_runtime<'a>() -> &'a ContainerRuntime {
    &CONFIG.get().unwrap().container_runtime
}

pub fn get_vudevname<'a>() -> &'a String {
    &CONFIG.get().unwrap().vudevname
}

pub fn get_device_owner<'a>() -> &'a DeviceOwner {
    &CONFIG.get().unwrap().device_owner
}

pub fn get_scope<'a>() -> &'a Scope {
    &CONFIG.get().unwrap().scope
}
