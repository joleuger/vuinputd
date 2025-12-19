// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct LoggedInputEvent {
    pub tv_sec: i64,

    pub tv_nsec: i64,

    pub duration_usec: i64,

    pub type_: u16,

    pub code: u16,

    pub value: i32,

    pub send_and_receive_match: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TestLog {
    pub events: Vec<LoggedInputEvent>,
}
