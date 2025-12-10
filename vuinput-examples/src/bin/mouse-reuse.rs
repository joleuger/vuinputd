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
const EV_REL: i32 = 2;
const BTN_LEFT: i32 = 272;
const REL_X: i32 = 0;
const REL_Y: i32 = 1;
const SYN_REPORT: i32 = 0;
const BUS_USB: u16 = 0x03;

///

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

    let args: Vec<String> = std::env::args().collect();
    let device = match args.len() {
        2 => args[1].clone(),
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

        ui_set_evbit(fd, EV_KEY.try_into().unwrap()).unwrap_or_else(|e| {
            eprintln!("ui_set_evbit(EV_KEY) failed: {:?}", e);
            std::process::exit(1);
        });

        ui_set_keybit(fd, BTN_LEFT.try_into().unwrap()).unwrap_or_else(|e| {
            eprintln!("ui_set_keybit(BTN_LEFT) failed: {:?}", e);
            std::process::exit(1);
        });

        ui_set_evbit(fd, EV_REL.try_into().unwrap()).unwrap_or_else(|e| {
            eprintln!("ui_set_evbit(EV_REL) failed: {:?}", e);
            std::process::exit(1);
        });

        ui_set_relbit(fd, REL_X.try_into().unwrap()).unwrap_or_else(|e| {
            eprintln!("ui_set_relbit(REL_X) failed: {:?}", e);
            std::process::exit(1);
        });

        ui_set_relbit(fd, REL_Y.try_into().unwrap()).unwrap_or_else(|e| {
            eprintln!("ui_set_relbit(REL_Y) failed: {:?}", e);
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
        eprintln!("ui_dev_setup first time");
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
        sleep(Duration::from_secs(1));

        eprintln!("emit some input");
        for n in 1..10 {
            if n % 10 <= 5 {
                emit(fd, EV_REL, REL_X, 5)?;
                emit(fd, EV_REL, REL_Y, 5)?;
            } else {
                emit(fd, EV_REL, REL_X, -5)?;
                emit(fd, EV_REL, REL_Y, -5)?;
            }
            emit(fd, EV_SYN, SYN_REPORT, 0)?;
            sleep(Duration::from_millis(300));
        }

        // Give userspace time to read events
        sleep(Duration::from_secs(5));

        eprintln!("ui_dev_destroy");
        // Destroy device and close fd
        ui_dev_destroy(fd).unwrap_or_else(|e| {
            eprintln!("ui_dev_destroy failed: {:?}", e);
            close(fd);
            std::process::exit(1);
        });

        // Give userspace time to read events
        sleep(Duration::from_secs(5));

        // Reuse device

        eprintln!("reuse device");
        ui_dev_setup(fd, usetup_ptr).unwrap_or_else(|e| {
            eprintln!("ui_dev_setup failed: {:?}", e);
            close(fd);
            std::process::exit(1);
        });

        ui_dev_create(fd).unwrap_or_else(|e| {
            eprintln!("ui_dev_create a second time failed: {:?}", e);
            close(fd);
            std::process::exit(1);
        });

        eprintln!("emit some input");
        for n in 1..10 {
            if n % 10 <= 5 {
                emit(fd, EV_REL, REL_X, 5)?;
                emit(fd, EV_REL, REL_Y, 5)?;
            } else {
                emit(fd, EV_REL, REL_X, -5)?;
                emit(fd, EV_REL, REL_Y, -5)?;
            }
            emit(fd, EV_SYN, SYN_REPORT, 0)?;
            sleep(Duration::from_millis(300));
        }

        // Give userspace time to read events
        sleep(Duration::from_secs(5));

        eprintln!("ui_dev_destroy");
        // Destroy device and close fd
        ui_dev_destroy(fd).unwrap_or_else(|e| {
            eprintln!("ui_dev_destroy failed: {:?}", e);
            close(fd);
            std::process::exit(1);
        });

        // Give userspace time to read events
        sleep(Duration::from_secs(2));

        close(fd);
    }

    Ok(())
}
