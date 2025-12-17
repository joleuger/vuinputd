// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum Action {
    #[serde(rename = "mknod-device")]
    MknodDevice {
        path: String,
        major: u32,
        minor: u32,
        mode: u32,
    },

    #[serde(rename = "announce-via-netlink")]
    AnnounceViaNetlink { message: String },

    #[serde(rename = "remove-device")]
    RemoveDevice { path: String },
}
