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
        major: u64,
        minor: u64,
    },

    #[serde(rename = "write-udev-runtime-data")]
    WriteUdevRuntimeData {
        runtime_data: Option<String>,
        major: u64,
        minor: u64,
    },

    #[serde(rename = "emit-netlink-message")]
    EmitNetlinkMessage {
        netlink_message: HashMap<String, String>,
    },

    #[serde(rename = "remove-device")]
    RemoveDevice {
        path: String,
        major: u64,
        minor: u64,
    },
}
