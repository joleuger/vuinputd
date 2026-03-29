// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::thread;
use std::time::Duration;

use crate::devices::xbox_gamepad::XboxGamepadDevice;
use crate::devices::{Device, EV_FF};
use crate::scenarios::ScenarioArgs;
use crate::test_log::{LoggedInputEvent, TestLog};

const BTN_A: u16 = 304;

pub struct FfXboxGamepad;

impl FfXboxGamepad {
    pub fn run(args: &ScenarioArgs) -> Result<(), std::io::Error> {
        let device = args
            .dev_path
            .clone()
            .unwrap_or_else(|| "/dev/uinput".to_string());

        let mut gamepad = XboxGamepadDevice::create(Some(&device), "Xbox Gamepad")?;
        eprintln!("sysname: {}", gamepad.sysname());

        thread::sleep(Duration::from_secs(1));

        let effect = libc::ff_effect {
            type_: todo!(),
            id: todo!(),
            direction: todo!(),
            trigger: todo!(),
            replay: todo!(),
            u: todo!(),
        };

        let _ev_play_effect = gamepad.emit_read_and_log(EV_FF, effect.id.try_into().unwrap(), 3)?;
        thread::sleep(Duration::from_secs(1));
        let _ev_stop_effect = gamepad.emit_read_and_log(EV_FF, effect.id.try_into().unwrap(), 0)?;

        let eventlog = TestLog {
            events: gamepad.event_log().to_vec(),
        };
        let serialized = serde_json::to_string(&eventlog).unwrap();
        println!("Event log: {}", serialized);

        XboxGamepadDevice::destroy(gamepad);
        Ok(())
    }
}
