// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>
use std::thread;
use std::time::Duration;

use crate::devices::ps4_gamepad::Ps4GamepadDevice;
use crate::devices::{utils, Device};
use crate::scenarios::ScenarioArgs;
use crate::test_log::{LoggedInputEvent, TestLog};

const BTN_SOUTH: u16 = 304;

pub struct BasicPs4Gamepad;

impl BasicPs4Gamepad {
    pub fn run(args: &ScenarioArgs) -> Result<(), std::io::Error> {
        let device = args
            .dev_path
            .clone()
            .unwrap_or_else(|| "/dev/uinput".to_string());

        let fd = Ps4GamepadDevice::setup(Some(&device), "PS4 Gamepad")?;
        let sysname = Ps4GamepadDevice::create(fd)?;
        eprintln!("sysname: {}", sysname);

        thread::sleep(Duration::from_secs(1));

        let event_device = std::fs::OpenOptions::new()
            .read(true)
            .open(&utils::fetch_device_node(&sysname)?)?;

        let ev1 = utils::emit_read_and_log(fd, &event_device, 0x01, BTN_SOUTH, 1)?;
        let ev2 = utils::emit_read_and_log(fd, &event_device, 0x01, BTN_SOUTH, 0)?;

        let eventlog = TestLog {
            events: vec![ev1, ev2],
        };
        let serialized = serde_json::to_string(&eventlog).unwrap();
        println!("Event log: {}", serialized);

        Ps4GamepadDevice::destroy(fd);
        Ok(())
    }
}
