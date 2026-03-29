// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>
use std::thread;
use std::time::Duration;

use crate::devices::mouse::MouseDevice;
use crate::devices::{Device, EV_KEY};
use crate::scenarios::ScenarioArgs;
use crate::test_log::{LoggedInputEvent, TestLog};

const BTN_LEFT: u16 = 272;

pub struct BasicMouse;

impl BasicMouse {
    pub fn run(args: &ScenarioArgs) -> Result<(), std::io::Error> {
        let device = args
            .dev_path
            .clone()
            .unwrap_or_else(|| "/dev/uinput".to_string());

        let mut mouse = MouseDevice::create(Some(&device), "Example Mouse")?;
        eprintln!("sysname: {}", mouse.sysname());

        thread::sleep(Duration::from_secs(1));

        let _ev1 = mouse.emit_read_and_log(EV_KEY, BTN_LEFT, 1)?;
        let _ev2 = mouse.emit_read_and_log(EV_KEY, BTN_LEFT, 0)?;

        let eventlog = TestLog {
            events: mouse.event_log().to_vec(),
        };
        let serialized = serde_json::to_string(&eventlog).unwrap();
        println!("Event log: {}", serialized);

        MouseDevice::destroy(mouse);
        Ok(())
    }
}
