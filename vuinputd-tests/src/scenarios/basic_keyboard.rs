// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::thread;
use std::time::Duration;

use crate::devices::keyboard::KeyboardDevice;
use crate::devices::{Device, EV_KEY};
use crate::scenarios::ScenarioArgs;
use crate::test_log::TestLog;

const KEY_SPACE: u16 = 57;

pub struct BasicKeyboard;

impl BasicKeyboard {
    pub fn run(args: &ScenarioArgs) -> Result<(), std::io::Error> {
        let device = args
            .dev_path
            .clone()
            .unwrap_or_else(|| "/dev/uinput".to_string());
        let mut keyboard = KeyboardDevice::create(Some(&device), "Example Keyboard")?;
        eprintln!("sysname: {}", keyboard.sysname());

        thread::sleep(Duration::from_secs(1));

        let _ev1 = keyboard.emit_read_and_log(EV_KEY, KEY_SPACE, 1)?;
        let _ev2 = keyboard.emit_read_and_log(EV_KEY, KEY_SPACE, 0)?;

        let eventlog = TestLog {
            events: keyboard.event_log().to_vec(),
        };
        let serialized = serde_json::to_string(&eventlog).unwrap();
        println!("Event log: {}", serialized);

        KeyboardDevice::destroy(keyboard);
        Ok(())
    }
}
