// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use libc::uinput_setup;
use libc::{c_int, close, open, write, O_NONBLOCK, O_WRONLY};
use std::ffi::{CStr, CString};
use std::io;
use std::mem::{size_of, zeroed};
use std::os::raw::{c_char, c_void};
use std::ptr;
use std::thread::sleep;
use std::time::Duration;
pub use uinput_ioctls::*;

// Constants (same numeric values as in linux headers)
const EV_SYN: i32 = 0x00;
const EV_KEY: i32 = 0x01;
const SYN_REPORT: i32 = 0;
const BUS_USB: u16 = 0x03;

/// Key codes. Those are used by udev to recognize a device as a keyboard.
const KEY_ESC: i32 = 1;
const KEY_1: i32 = 2;
const KEY_2: i32 = 3;
const KEY_3: i32 = 4;
const KEY_4: i32 = 5;
const KEY_5: i32 = 6;
const KEY_6: i32 = 7;
const KEY_7: i32 = 8;
const KEY_8: i32 = 9;
const KEY_9: i32 = 10;
const KEY_0: i32 = 11;
const KEY_MINUS: i32 = 12;
const KEY_EQUAL: i32 = 13;
const KEY_BACKSPACE: i32 = 14;
const KEY_TAB: i32 = 15;
const KEY_Q: i32 = 16;
const KEY_W: i32 = 17;
const KEY_E: i32 = 18;
const KEY_R: i32 = 19;
const KEY_T: i32 = 20;
const KEY_Y: i32 = 21;
const KEY_U: i32 = 22;
const KEY_I: i32 = 23;
const KEY_O: i32 = 24;
const KEY_P: i32 = 25;
const KEY_LEFTBRACE: i32 = 26;
const KEY_RIGHTBRACE: i32 = 27;
const KEY_ENTER: i32 = 28;
const KEY_LEFTCTRL: i32 = 29;
const KEY_A: i32 = 30;
const KEY_S: i32 = 31;

/// Space and other common keys
const KEY_D: i32 = 32;
const KEY_F: i32 = 33;
const KEY_G: i32 = 34;
const KEY_H: i32 = 35;
const KEY_J: i32 = 36;
const KEY_K: i32 = 37;
const KEY_L: i32 = 38;
const KEY_SEMICOLON: i32 = 39;
const KEY_APOSTROPHE: i32 = 40;
const KEY_GRAVE: i32 = 41;
const KEY_LEFTSHIFT: i32 = 42;
const KEY_BACKSLASH: i32 = 43;
const KEY_Z: i32 = 44;
const KEY_X: i32 = 45;
const KEY_C: i32 = 46;
const KEY_V: i32 = 47;
const KEY_B: i32 = 48;
const KEY_N: i32 = 49;
const KEY_M: i32 = 50;
const KEY_COMMA: i32 = 51;
const KEY_DOT: i32 = 52;
const KEY_SLASH: i32 = 53;
const KEY_RIGHTSHIFT: i32 = 54;
const KEY_KPASTERISK: i32 = 55;
const KEY_LEFTALT: i32 = 56;
const KEY_SPACE: i32 = 57;
const KEY_CAPSLOCK: i32 = 58;

/// Function keys
const KEY_F1: i32 = 59;
const KEY_F2: i32 = 60;
const KEY_F3: i32 = 61;
const KEY_F4: i32 = 62;
const KEY_F5: i32 = 63;
const KEY_F6: i32 = 64;
const KEY_F7: i32 = 65;
const KEY_F8: i32 = 66;
const KEY_F9: i32 = 67;
const KEY_F10: i32 = 68;
const KEY_NUMLOCK: i32 = 69;
const KEY_SCROLLLOCK: i32 = 70;
const KEY_KP7: i32 = 71;
const KEY_KP8: i32 = 72;
const KEY_KP9: i32 = 73;
const KEY_KPMINUS: i32 = 74;
const KEY_KP4: i32 = 75;
const KEY_KP5: i32 = 76;
const KEY_KP6: i32 = 77;
const KEY_KPPLUS: i32 = 78;
const KEY_KP1: i32 = 79;
const KEY_KP2: i32 = 80;
const KEY_KP3: i32 = 81;
const KEY_KP0: i32 = 82;
const KEY_KPDOT: i32 = 83;

/// Arrow keys and navigation
const KEY_ZENKAKUHANKAKU: i32 = 85;
const KEY_102ND: i32 = 86;
const KEY_F11: i32 = 87;
const KEY_F12: i32 = 88;
const KEY_RO: i32 = 89;
const KEY_KATAKANA: i32 = 90;
const KEY_HIRAGANA: i32 = 91;
const KEY_HENKAN: i32 = 92;
const KEY_KATAKANAHIRAGANA: i32 = 93;
const KEY_MUHENKAN: i32 = 94;
const KEY_KPJPCOMMA: i32 = 95;
const KEY_KPENTER: i32 = 96;
const KEY_RIGHTCTRL: i32 = 97;
const KEY_KPSLASH: i32 = 98;
const KEY_SYSRQ: i32 = 99;
const KEY_RIGHTALT: i32 = 100;
const KEY_LINEFEED: i32 = 101;
const KEY_HOME: i32 = 102;
const KEY_UP: i32 = 103;
const KEY_PAGEUP: i32 = 104;
const KEY_LEFT: i32 = 105;
const KEY_RIGHT: i32 = 106;
const KEY_END: i32 = 107;
const KEY_DOWN: i32 = 108;
const KEY_PAGEDOWN: i32 = 109;
const KEY_INSERT: i32 = 110;
const KEY_DELETE: i32 = 111;

/// Configure a full 101-key standard keyboard
unsafe fn set_standard_keyboard_keys(fd: i32) -> Result<(), std::io::Error> {
    // We need to set more bits so that systemd recognizes a keyboard as a keyboard.
    // At least the first 32 bits are ESC, numbers, and Q to D, except KEY_RESERVED need to be considered.
    // udev-builtin-input_id.c consideres the mask = 0xFFFFFFFE

    // EV_KEY
    ui_set_evbit(fd, EV_KEY.try_into().unwrap())?;

    // All standard keys (1..101+)
    let all_keys = [
        // Modifier + main keys
        KEY_ESC,
        KEY_1,
        KEY_2,
        KEY_3,
        KEY_4,
        KEY_5,
        KEY_6,
        KEY_7,
        KEY_8,
        KEY_9,
        KEY_0,
        KEY_MINUS,
        KEY_EQUAL,
        KEY_BACKSPACE,
        KEY_TAB,
        KEY_Q,
        KEY_W,
        KEY_E,
        KEY_R,
        KEY_T,
        KEY_Y,
        KEY_U,
        KEY_I,
        KEY_O,
        KEY_P,
        KEY_LEFTBRACE,
        KEY_RIGHTBRACE,
        KEY_ENTER,
        KEY_LEFTCTRL,
        KEY_A,
        KEY_S,
        KEY_D,
        KEY_F,
        KEY_G,
        KEY_H,
        KEY_J,
        KEY_K,
        KEY_L,
        KEY_SEMICOLON,
        KEY_APOSTROPHE,
        KEY_GRAVE,
        KEY_LEFTSHIFT,
        KEY_BACKSLASH,
        KEY_Z,
        KEY_X,
        KEY_C,
        KEY_V,
        KEY_B,
        KEY_N,
        KEY_M,
        KEY_COMMA,
        KEY_DOT,
        KEY_SLASH,
        KEY_RIGHTSHIFT,
        KEY_KPASTERISK,
        KEY_LEFTALT,
        KEY_SPACE,
        KEY_CAPSLOCK,
        // Function keys
        KEY_F1,
        KEY_F2,
        KEY_F3,
        KEY_F4,
        KEY_F5,
        KEY_F6,
        KEY_F7,
        KEY_F8,
        KEY_F9,
        KEY_F10,
        KEY_F11,
        KEY_F12,
        KEY_NUMLOCK,
        KEY_SCROLLLOCK,
        // Keypad
        KEY_KP7,
        KEY_KP8,
        KEY_KP9,
        KEY_KPMINUS,
        KEY_KP4,
        KEY_KP5,
        KEY_KP6,
        KEY_KPPLUS,
        KEY_KP1,
        KEY_KP2,
        KEY_KP3,
        KEY_KP0,
        KEY_KPDOT,
        KEY_KPENTER,
        KEY_KPSLASH,
        KEY_KPJPCOMMA,
        // Arrows / navigation
        KEY_HOME,
        KEY_UP,
        KEY_PAGEUP,
        KEY_LEFT,
        KEY_RIGHT,
        KEY_END,
        KEY_DOWN,
        KEY_PAGEDOWN,
        KEY_INSERT,
        KEY_DELETE,
        KEY_RIGHTCTRL,
        KEY_RIGHTALT,
        // Optional Japanese / additional keys
        KEY_ZENKAKUHANKAKU,
        KEY_102ND,
        KEY_RO,
        KEY_KATAKANA,
        KEY_HIRAGANA,
        KEY_HENKAN,
        KEY_KATAKANAHIRAGANA,
        KEY_MUHENKAN,
        KEY_LINEFEED,
        KEY_SYSRQ,
    ];

    for &key in all_keys.iter() {
        ui_set_keybit(fd, key.try_into().unwrap())?;
    }

    Ok(())
}

fn emit(fd: c_int, ev_type: i32, code: i32, val: i32) -> io::Result<()> {
    // libc's input_event struct layout:
    // struct input_event {
    //   struct timeval time;
    //   __u16 type;
    //   __u16 code;
    //   __s32 value;
    // };
    //
    // libc provides input_event as `libc::input_event` on Linux.
    let mut ie: libc::input_event = unsafe { zeroed() };

    // time fields are ignored by kernel for synthetic events - set zero
    ie.time.tv_sec = 0;
    ie.time.tv_usec = 0;

    // input_event fields: type and code are u16 in C; value is i32
    ie.type_ = ev_type as u16; // note: in libc the field is `type_`
    ie.code = code as u16;
    ie.value = val as i32;

    // write the struct to the uinput fd
    let buf_ptr = &ie as *const libc::input_event as *const c_void;
    let bytes = size_of::<libc::input_event>();

    let written = unsafe { write(fd, buf_ptr, bytes) };
    if written as usize != bytes {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn main() -> io::Result<()> {
    // open device - matches: open("/dev/uinput-test", O_WRONLY | O_NONBLOCK);
    let path = CString::new("/dev/uinput-test").unwrap();
    let fd = unsafe { open(path.as_ptr(), O_WRONLY | O_NONBLOCK) };
    if fd < 0 {
        eprintln!("error opening uinput");
        return Err(io::Error::last_os_error());
    }

    // In your snippet you supplied value.into() to the wrappers. The wrappers may accept different types.
    // We follow your earlier usage pattern:
    unsafe {
        let mut version_of_uinput = 0;
        let pversion_of_uinput = std::ptr::from_mut(&mut version_of_uinput);
        eprintln!("ioctl UI_GET_VERSION request");
        ui_get_version(fd, pversion_of_uinput).unwrap_or_else(|e| {
            eprintln!("ui_get_version failed: {:?}", e);
            std::process::exit(1);
        });
        eprintln!("ioctl UI_GET_VERSION {}", version_of_uinput);

        let _ = set_standard_keyboard_keys(fd).unwrap_or_else(|e| {
            eprintln!("set_standard_keyboard_keys failed: {:?}", e);
            std::process::exit(1);
        });
    }

    // Prepare uinput_setup struct
    let mut usetup: uinput_setup = unsafe { zeroed() };

    // Fill id and name fields
    // `id` has bustype, vendor, product fields (types may vary slightly by libc version)
    // set bustype/vendor/product as in C example
    // Note: make sure the fields exist as below in your libc version; adapt if names differ.
    usetup.id.bustype = BUS_USB;
    usetup.id.vendor = 0xbeef;
    usetup.id.product = 0xdead;

    // Copy device name into the C char array in the struct
    let name = CString::new("Example device").unwrap();
    // uinput_setup::name is usually [c_char; UINPUT_MAX_NAME_SIZE]
    unsafe {
        // Fill with zeros first (already zeroed by zeroed()) then copy bytes
        let name_ptr = usetup.name.as_mut_ptr() as *mut c_char;
        ptr::copy_nonoverlapping(name.as_ptr(), name_ptr, name.to_bytes_with_nul().len());
    }

    // Call IOCTLs to setup and create the device
    // Assuming your wrappers accept (fd, ptr_to_usetup) etc.
    // We'll pass pointer to usetup
    let usetup_ptr = &mut usetup as *mut uinput_setup;
    unsafe {
        ui_dev_setup(fd, usetup_ptr).unwrap_or_else(|e| {
            eprintln!("ui_dev_setup failed: {:?}", e);
            close(fd);
            std::process::exit(1);
        });

        ui_dev_create(fd).unwrap_or_else(|e| {
            eprintln!("ui_dev_create failed: {:?}", e);
            close(fd);
            std::process::exit(1);
        });

        let mut resultbuf: [c_char; 64] = [0; 64];
        ui_get_sysname(fd, &mut resultbuf).unwrap();
        let sysname = CStr::from_ptr(resultbuf.as_ptr()).to_string_lossy();
        eprintln!("sysname: {}", sysname);

        // Sleep 1 second to allow userspace to detect the device (same as C example)
        sleep(Duration::from_secs(10));

        // Emit press + syn + release + syn
        emit(fd, EV_KEY, KEY_SPACE, 1)?;
        emit(fd, EV_SYN, SYN_REPORT, 0)?;
        emit(fd, EV_KEY, KEY_SPACE, 0)?;
        emit(fd, EV_SYN, SYN_REPORT, 0)?;

        // Give userspace time to read events
        sleep(Duration::from_secs(10));

        // Destroy device and close fd
        ui_dev_destroy(fd).unwrap_or_else(|e| {
            eprintln!("ui_dev_destroy failed: {:?}", e);
            close(fd);
            std::process::exit(1);
        });

        close(fd);
    }

    Ok(())
}
