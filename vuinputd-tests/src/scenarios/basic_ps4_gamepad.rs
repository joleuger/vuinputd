// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>
use std::thread;
use std::time::Duration;

use crate::devices::ps4_gamepad::Ps4GamepadDevice;
use crate::devices::{Device, EV_KEY};
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

        let mut gamepad = Ps4GamepadDevice::create(Some(&device), "PS4 Gamepad")?;
        eprintln!("sysname: {}", gamepad.sysname());

        thread::sleep(Duration::from_secs(1));

        let _ev1 = gamepad.emit_read_and_log(EV_KEY, BTN_SOUTH, 1)?;
        let _ev2 = gamepad.emit_read_and_log(EV_KEY, BTN_SOUTH, 0)?;

        let eventlog = TestLog {
            events: gamepad.event_log().to_vec(),
        };
        let serialized = serde_json::to_string(&eventlog).unwrap();
        println!("Event log: {}", serialized);

        Ps4GamepadDevice::destroy(gamepad);
        Ok(())
    }
}
