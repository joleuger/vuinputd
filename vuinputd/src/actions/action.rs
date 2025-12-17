// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum Action {
    #[serde(rename = "mknod-device")]
    MknodDevice {
        path: String,
        major: u32,
        minor: u32,
    },

    #[serde(rename = "emit-udev-event")]
    EmitUdevEvent {
        netlink_message: HashMap<String, String>,
        runtime_data: Option<String>,
        major: u32,
        minor: u32,
    },

    #[serde(rename = "remove-device")]
    RemoveDevice {
        path: String,
        major: u32,
        minor: u32,
    },
}
