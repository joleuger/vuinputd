// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use clap::Parser;
use libc::{CLOCK_MONOTONIC, input_event, timespec, uinput_setup};
use libc::{c_int, close, open, write, O_NONBLOCK, O_WRONLY};
use vuinputd_tests::test_log::{LoggedInputEvent, TestLog};
use std::ffi::{CStr, CString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind};
use std::mem::{self, size_of, zeroed};
use std::os::fd::AsRawFd;
use std::os::raw::{c_char, c_void};
use std::ptr;
use std::thread::sleep;
use std::time::Duration;
pub use uinput_ioctls::*;

// Constants (same numeric values as in linux headers)
const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const SYN_REPORT: u16 = 0;
const BUS_USB: u16 = 0x03;

/// Key codes. Those are used by udev to recognize a device as a keyboard.
const KEY_ESC: u16 = 1;
const KEY_1: u16 = 2;
const KEY_2: u16 = 3;
const KEY_3: u16 = 4;
const KEY_4: u16 = 5;
const KEY_5: u16 = 6;
const KEY_6: u16 = 7;
const KEY_7: u16 = 8;
const KEY_8: u16 = 9;
const KEY_9: u16 = 10;
const KEY_0: u16 = 11;
const KEY_MINUS: u16 = 12;
const KEY_EQUAL: u16 = 13;
const KEY_BACKSPACE: u16 = 14;
const KEY_TAB: u16 = 15;
const KEY_Q: u16 = 16;
const KEY_W: u16 = 17;
const KEY_E: u16 = 18;
const KEY_R: u16 = 19;
const KEY_T: u16 = 20;
const KEY_Y: u16 = 21;
const KEY_U: u16 = 22;
const KEY_I: u16 = 23;
const KEY_O: u16 = 24;
const KEY_P: u16 = 25;
const KEY_LEFTBRACE: u16 = 26;
const KEY_RIGHTBRACE: u16 = 27;
const KEY_ENTER: u16 = 28;
const KEY_LEFTCTRL: u16 = 29;
const KEY_A: u16 = 30;
const KEY_S: u16 = 31;

/// Space and other common keys
const KEY_D: u16 = 32;
const KEY_F: u16 = 33;
const KEY_G: u16 = 34;
const KEY_H: u16 = 35;
const KEY_J: u16 = 36;
const KEY_K: u16 = 37;
const KEY_L: u16 = 38;
const KEY_SEMICOLON: u16 = 39;
const KEY_APOSTROPHE: u16 = 40;
const KEY_GRAVE: u16 = 41;
const KEY_LEFTSHIFT: u16 = 42;
const KEY_BACKSLASH: u16 = 43;
const KEY_Z: u16 = 44;
const KEY_X: u16 = 45;
const KEY_C: u16 = 46;
const KEY_V: u16 = 47;
const KEY_B: u16 = 48;
const KEY_N: u16 = 49;
const KEY_M: u16 = 50;
const KEY_COMMA: u16 = 51;
const KEY_DOT: u16 = 52;
const KEY_SLASH: u16 = 53;
const KEY_RIGHTSHIFT: u16 = 54;
const KEY_KPASTERISK: u16 = 55;
const KEY_LEFTALT: u16 = 56;
const KEY_SPACE: u16 = 57;
const KEY_CAPSLOCK: u16 = 58;

/// Function keys
const KEY_F1: u16 = 59;
const KEY_F2: u16 = 60;
const KEY_F3: u16 = 61;
const KEY_F4: u16 = 62;
const KEY_F5: u16 = 63;
const KEY_F6: u16 = 64;
const KEY_F7: u16 = 65;
const KEY_F8: u16 = 66;
const KEY_F9: u16 = 67;
const KEY_F10: u16 = 68;
const KEY_NUMLOCK: u16 = 69;
const KEY_SCROLLLOCK: u16 = 70;
const KEY_KP7: u16 = 71;
const KEY_KP8: u16 = 72;
const KEY_KP9: u16 = 73;
const KEY_KPMINUS: u16 = 74;
const KEY_KP4: u16 = 75;
const KEY_KP5: u16 = 76;
const KEY_KP6: u16 = 77;
const KEY_KPPLUS: u16 = 78;
const KEY_KP1: u16 = 79;
const KEY_KP2: u16 = 80;
const KEY_KP3: u16 = 81;
const KEY_KP0: u16 = 82;
const KEY_KPDOT: u16 = 83;

/// Arrow keys and navigation
const KEY_ZENKAKUHANKAKU: u16 = 85;
const KEY_102ND: u16 = 86;
const KEY_F11: u16 = 87;
const KEY_F12: u16 = 88;
const KEY_RO: u16 = 89;
const KEY_KATAKANA: u16 = 90;
const KEY_HIRAGANA: u16 = 91;
const KEY_HENKAN: u16 = 92;
const KEY_KATAKANAHIRAGANA: u16 = 93;
const KEY_MUHENKAN: u16 = 94;
const KEY_KPJPCOMMA: u16 = 95;
const KEY_KPENTER: u16 = 96;
const KEY_RIGHTCTRL: u16 = 97;
const KEY_KPSLASH: u16 = 98;
const KEY_SYSRQ: u16 = 99;
const KEY_RIGHTALT: u16 = 100;
const KEY_LINEFEED: u16 = 101;
const KEY_HOME: u16 = 102;
const KEY_UP: u16 = 103;
const KEY_PAGEUP: u16 = 104;
const KEY_LEFT: u16 = 105;
const KEY_RIGHT: u16 = 106;
const KEY_END: u16 = 107;
const KEY_DOWN: u16 = 108;
const KEY_PAGEDOWN: u16 = 109;
const KEY_INSERT: u16 = 110;
const KEY_DELETE: u16 = 111;

const SYS_INPUT_DIR: &str = "/sys/devices/virtual/input/";

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


#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Args {
    /// Use IPC
    #[arg(long, default_value_t = false)]
    ipc: bool,

    /// Device path (with /dev/)
    #[arg(long)]
    dev_path: Option<String>,
}

fn emit(fd: c_int, ev_type: u16, code: u16, val: i32) -> io::Result<()> {
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

    ie.type_ = ev_type; // note: in libc the field is `type_`
    ie.code = code;
    ie.value = val;

    // write the struct to the uinput fd
    let buf_ptr = &ie as *const libc::input_event as *const c_void;
    let bytes = size_of::<libc::input_event>();

    let written = unsafe { write(fd, buf_ptr, bytes) };
    if written as usize != bytes {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}


fn emit_read_and_log(emit_to: c_int, read_from:&File, ev_type: u16, code: u16, val: i32) -> io::Result<LoggedInputEvent> {
    let (time_sent_sec,time_sent_nsec) = monotonic_time();
    emit(emit_to, ev_type, code, val)?;
    let input_event_recv=read_event(&read_from).unwrap();
    let (time_recv_sec,time_recv_nsec) = monotonic_time();
    let duration_nsec =(time_recv_sec-time_sent_sec)*1_000_000+(time_recv_nsec-time_sent_nsec)/1000;
    let send_and_receive_match = input_event_recv.type_==ev_type && input_event_recv.code==code && input_event_recv.value==val;

    Ok(LoggedInputEvent {
        tv_sec: time_sent_sec,
        tv_usec: time_sent_nsec,
        duration_nsec: duration_nsec,
        type_: ev_type,
        code: code,
        value: val,
        send_and_receive_match: send_and_receive_match
    })
}


pub fn fetch_device_node(path: &str) -> io::Result<String> {
    println!("Read dir {}",&path);
    for entry in fs::read_dir(path)? {
        let entry = entry?; // propagate per-entry errors
        if let Some(name) = entry.file_name().to_str() {
            if name.starts_with("event") {
                return Ok(format!("/dev/input/{}", name));
            }
        }
    }
    // If no device is found, return an error
    Err(io::Error::new(ErrorKind::NotFound, "no device found"))
}

pub fn read_event(event_dev : &File) -> io::Result<input_event> {

    let mut ev: input_event = unsafe { mem::zeroed() };/*
    let ret = unsafe {
            libc::read(
                event_dev.as_raw_fd(),
                &mut ev as *mut _ as *mut c_void,
                mem::size_of::<input_event>(),
            )
        };
    if ret as usize != mem::size_of::<input_event>() {
        return Err(io::Error::last_os_error());
    }*/
    Ok(ev)
}

fn monotonic_time() -> (i64,i64) {
    let mut ts = timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };

    unsafe {
        libc::clock_gettime(CLOCK_MONOTONIC, &mut ts);
    }
    (ts.tv_sec ,ts.tv_nsec)
}


fn main() -> io::Result<()> {
    // open device - matches: open("/dev/uinput", O_WRONLY | O_NONBLOCK);
    let args=Args::parse();

    let device = match args.dev_path {
        Some(dev_path) => dev_path,
        _ => "/dev/uinput".to_string(),
    };

    let path = CString::new(device).unwrap();
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

        // Sleep 2 second to allow userspace to detect the device (same as C example)
        sleep(Duration::from_secs(2));

        let mut resultbuf: [c_char; 64] = [0; 64];
        ui_get_sysname(fd, resultbuf.as_mut_slice()).unwrap();
        let sysname = format!(
            "{}{}",
            SYS_INPUT_DIR,
            CStr::from_ptr(resultbuf.as_ptr()).to_string_lossy()
        );
        println!("syspath: {}", sysname);
        let devnode = fetch_device_node(&sysname).unwrap_or_else(|e| panic!("failed to fetch device node!: {e}"));
        println!("devnode: {}", devnode);

        eprintln!("sysname: {}", sysname);

        let event_device = OpenOptions::new()
        .read(true)
        .open(&devnode)
        .unwrap_or_else(|err| panic!("Could not open event device {}, Error {}",&devnode,err));

        // Emit press + syn + release + syn
        let ev1 = emit_read_and_log(fd, &event_device, EV_KEY, KEY_SPACE, 1)?;
        let ev2 = emit_read_and_log(fd, &event_device,EV_SYN, SYN_REPORT, 0)?;
        let ev3 = emit_read_and_log(fd, &event_device,EV_KEY, KEY_SPACE, 0)?;
        let ev4 = emit_read_and_log(fd, &event_device,EV_SYN, SYN_REPORT, 0)?;

        let eventlog = TestLog{events:vec![ev1,ev2,ev3,ev4]};
        let serialized = serde_json::to_string(&eventlog).unwrap();
        println!("Event log: {}",serialized);

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
