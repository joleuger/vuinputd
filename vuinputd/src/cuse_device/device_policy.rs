// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use libc::input_event;

// event types and codes from https://github.com/torvalds/linux/blob/master/include/uapi/linux/input-event-codes.h

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_REL: u16 = 0x02;
const EV_ABS: u16 = 0x03;
const EV_MSC: u16 = 0x04;
const EV_SW: u16 = 0x05;
const EV_LED: u16 = 0x11;
const EV_SND: u16 = 0x12;
const EV_REP: u16 = 0x14;
const EV_FF: u16 = 0x15;
const EV_PWR: u16 = 0x16;
const EV_FF_STATUS: u16 = 0x17;
const EV_MAX: u16 = 0x1f;

// special keyboard keys
const KEY_LEFTALT: u16 = 56;
const KEY_RIGHTALT: u16 = 100;
const KEY_LEFTCTRL: u16 = 29;
const KEY_RIGHTCTRL: u16 = 97;
const KEY_F1: u16 = 59;
const KEY_F10: u16 = 68;
const KEY_F11: u16 = 87;
const KEY_F12: u16 = 88;
const KEY_SYSRQ: u16 = 99;
const KEY_DELETE: u16 = 111;
const KEY_KPDOT: u16 = 83;
const KEY_POWER: u16 = 116;
const KEY_SLEEP: u16 = 142;
const KEY_WAKEUP: u16 = 143;

// Gamepad keys from https://github.com/torvalds/linux/blob/master/Documentation/input/gamepad.rst
// First range
const BTN_SOUTH: u16 = 0x130;
const BTN_THUMBR: u16 = 0x13e;
// Second range
const BTN_DPAD_UP: u16 = 0x220;
const BTN_GRIPR2: u16 = 0x227;

use crate::{cuse_device::state::KeyTracker, global_config::DevicePolicy};

fn is_allowed(keytracker: &mut KeyTracker, policy: &DevicePolicy, event: &input_event) -> bool {
    match policy {
        DevicePolicy::None => true,
        DevicePolicy::Sanitized => is_allowed_in_sanitized_mode(keytracker, event),
        DevicePolicy::StrictGamepad => is_allowed_in_strict_gamepad_mode(keytracker, event),
    }
}

fn is_allowed_in_sanitized_mode(keytracker: &mut KeyTracker, event: &input_event) -> bool {
    let type_ = event.type_;
    let code = event.code;
    let value = event.value;

    if type_ == EV_KEY {
        match code {
            v if v == KEY_LEFTALT => keytracker.left_alt_down = value > 0,
            v if v == KEY_RIGHTALT => keytracker.right_alt_down = value > 0,
            v if v == KEY_LEFTCTRL => keytracker.left_ctrl_down = value > 0,
            v if v == KEY_RIGHTCTRL => keytracker.right_ctrl_down = value > 0,
            _ => {}
        }
    }

    if type_ == EV_KEY {
        // 1. Block SysRq in general
        if code == KEY_SYSRQ {
            return false;
        }

        let alt_down = keytracker.left_alt_down || keytracker.right_alt_down;
        let ctrl_down = keytracker.left_ctrl_down || keytracker.right_ctrl_down;

        // 2. Block VT Switching
        // To block VT Switching, all CONSOLE_ actions need to be ignored.
        // In standard Linux keymaps (defkeymap)
        // https://github.com/torvalds/linux/blob/master/drivers/tty/vt/defkeymap.map
        //  - Left Alt + F1–F12 usually maps to Console_1 – Console_12.
        //  - Right Alt (AltGr) + F1–F12 usually maps to Console_13 – Console_24.
        // Note: Alt + Left/Right (Decr_Console / Incr_Console) is still allowed. We assume
        // this is blocked in any other way.
        if alt_down && (code >= KEY_F1 && code <= KEY_F10) {
            return false;
        }
        if alt_down && (code >= KEY_F11 && code <= KEY_F12) {
            return false;
        }

        // 3. Block CAD (Ctrl + Alt + Del)
        // Block basically all Boot from defkeymap.map
        if alt_down && ctrl_down && (code == KEY_DELETE || code == KEY_KPDOT) {
            return false;
        }

        // 4. Block standalone dangerous keys
        match code {
            KEY_POWER | KEY_SLEEP | KEY_WAKEUP => return false,
            _ => {}
        }
    }
    true
}

fn is_allowed_in_strict_gamepad_mode(keytracker: &mut KeyTracker, event: &input_event) -> bool {
    let type_ = event.type_;
    let code = event.code;

    if type_ == EV_SYN {
        return true;
    }
    if type_ == EV_ABS {
        return true;
    }
    if type_ == EV_FF {
        return true;
    }

    if type_ == EV_KEY {
        return match code {
            BTN_SOUTH..BTN_THUMBR => true,
            BTN_DPAD_UP..BTN_GRIPR2 => true,
            _ => false,
        };
    }
    false
}
