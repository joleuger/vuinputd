// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::thread;
use std::time::Duration;

use crate::devices::keyboard::KeyboardDevice;
use crate::scenarios::ScenarioArgs;
use crate::devices::{Device, utils};
use crate::test_log::{TestLog};

const KEY_SPACE: u16 = 57;

pub struct BasicKeyboard;

impl BasicKeyboard {
    pub fn run(args: &ScenarioArgs) -> Result<(), std::io::Error> {
        let device = args.dev_path.clone().unwrap_or_else(|| "/dev/uinput".to_string());
        let fd = KeyboardDevice::setup(Some(&device),"Example Keyboard")?;
        let sysname = KeyboardDevice::create(fd)?;
        eprintln!("sysname: {}", sysname);

        thread::sleep(Duration::from_secs(1));

        let event_device = std::fs::OpenOptions::new()
            .read(true)
            .open(&utils::fetch_device_node(&sysname)?)?;

        let ev1 = utils::emit_read_and_log(fd, &event_device, 0x01, KEY_SPACE, 1)?;
        let ev2 = utils::emit_read_and_log(fd, &event_device, 0x01, KEY_SPACE, 0)?;

        let eventlog = TestLog { events: vec![ev1, ev2] };
        let serialized = serde_json::to_string(&eventlog).unwrap();
        println!("Event log: {}", serialized);

        KeyboardDevice::destroy(fd);
        Ok(())
    }
}