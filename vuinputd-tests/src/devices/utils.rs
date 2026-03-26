// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use crate::test_log::{LoggedInputEvent, TestLog};
use libc::{c_int, close, open, write, O_NONBLOCK, O_WRONLY};
use libc::{input_event, timespec, uinput_setup, CLOCK_MONOTONIC};
use std::ffi::{CStr, CString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind};
use std::mem::{self, size_of, zeroed};
use std::os::fd::AsRawFd;
use std::os::raw::{c_char, c_void};
use std::ptr;
pub use uinput_ioctls::*;

// Constants (same numeric values as in linux headers)
const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const SYN_REPORT: u16 = 0;
const BUS_USB: u16 = 0x03;

pub fn emit(fd: c_int, ev_type: u16, code: u16, val: i32) -> io::Result<()> {
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

    //println!("write to {} {} {} {} ",fd,ev_type,code,val);
    let written = unsafe { write(fd, buf_ptr, bytes) };
    //println!("written");
    if written as usize != bytes {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

// Note that before we can read, a SYN needs to be sent. Thus combine it.
pub fn emit_read_and_log(
    emit_to: c_int,
    read_from: &File,
    ev_type: u16,
    code: u16,
    val: i32,
) -> io::Result<LoggedInputEvent> {
    let (time_sent_sec, time_sent_nsec) = monotonic_time();
    emit(emit_to, ev_type, code, val)?;
    emit(emit_to, EV_SYN, SYN_REPORT, 0)?;
    let input_event_recv = read_event(&read_from).unwrap();
    let _syn_recv = read_event(&read_from).unwrap();
    let (time_recv_sec, time_recv_nsec) = monotonic_time();
    let duration_usec =
        (time_recv_sec - time_sent_sec) * 1_000_000 + (time_recv_nsec - time_sent_nsec) / 1000;
    let send_and_receive_match = input_event_recv.type_ == ev_type
        && input_event_recv.code == code
        && input_event_recv.value == val;

    Ok(LoggedInputEvent {
        tv_sec: time_sent_sec,
        tv_nsec: time_sent_nsec,
        duration_usec: duration_usec,
        type_: ev_type,
        code: code,
        value: val,
        send_and_receive_match: send_and_receive_match,
    })
}

pub fn fetch_device_node(path: &str) -> io::Result<String> {
    println!("Read dir {}", &path);
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

pub fn read_event(event_dev: &File) -> io::Result<input_event> {
    let mut ev: input_event = unsafe { mem::zeroed() };
    let ret = unsafe {
        libc::read(
            event_dev.as_raw_fd(),
            &mut ev as *mut _ as *mut c_void,
            mem::size_of::<input_event>(),
        )
    };
    if ret as usize != mem::size_of::<input_event>() {
        return Err(io::Error::last_os_error());
    }
    Ok(ev)
}

pub fn monotonic_time() -> (i64, i64) {
    let mut ts = timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };

    unsafe {
        libc::clock_gettime(CLOCK_MONOTONIC, &mut ts);
    }
    (ts.tv_sec, ts.tv_nsec)
}

pub fn open_uinput(device: Option<&str>) -> io::Result<i32> {
    let device = match device {
        Some(dev_path) => dev_path,
        _ => "/dev/uinput",
    };

    let path = CString::new(device).unwrap();
    let fd = unsafe { open(path.as_ptr(), O_WRONLY | O_NONBLOCK) };
    if fd < 0 {
        eprintln!("error opening uinput");
        return Err(io::Error::last_os_error());
    }
    Ok(fd)
}
