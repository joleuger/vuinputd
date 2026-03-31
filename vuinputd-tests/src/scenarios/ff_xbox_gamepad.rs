// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::thread;
use std::time::Duration;

use crate::devices::xbox_gamepad::{self, upload_effect, XboxGamepadDevice, FF_RUMBLE};
use crate::devices::{Device, EV_FF};
use crate::scenarios::ScenarioArgs;
use crate::test_log::{LoggedInputEvent, TestLog};
use libc::{self, ff_effect, ff_replay, ff_trigger};

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

        eprintln!("upload a simple RUMBLE effect");
        let mut effect: ff_effect = unsafe { std::mem::zeroed() };
        effect.type_ = FF_RUMBLE;
        effect.id = -1; // new effect
        effect.direction = 0;
        effect.trigger.button = 0;
        effect.trigger.interval = 0;
        effect.replay.length = 5000;
        effect.replay.delay = 1000;
        effect.u = xbox_gamepad::create_rumble_array(0x8000, 0x0);

        // ensure uploaded effect gets processed
        gamepad.read_process_ff_event_from_uinput();
        // Upload effect via ioctl
        let effect_id = upload_effect(gamepad.state().event_device_fd, &mut effect)?;

        eprintln!("Uploaded effect with id: {} {}", effect_id, effect.id);
        thread::sleep(Duration::from_secs(1));

        // Play effect (value=1)
        let _play_effect_event =
            gamepad.emit_read_and_log(EV_FF, effect_id.try_into().unwrap(), 1)?;
        thread::sleep(Duration::from_secs(1));
        let _stop_effect_event =
            gamepad.emit_read_and_log(EV_FF, effect_id.try_into().unwrap(), 0)?;
        thread::sleep(Duration::from_secs(1));

        let eventlog = TestLog {
            events: gamepad.event_log().to_vec(),
        };
        let serialized = serde_json::to_string(&eventlog).unwrap();
        println!("Event log: {}", serialized);

        XboxGamepadDevice::destroy(gamepad);
        Ok(())
    }
}
