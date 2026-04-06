// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use crate::test_log::LoggedInputEvent;
use libc::{c_int, close, open, write, O_NONBLOCK, O_RDWR, O_WRONLY};
use libc::{input_event, timespec, uinput_setup, CLOCK_MONOTONIC};
use std::ffi::{CStr, CString};
use std::fs::File;
use std::io;
use std::mem::{size_of, zeroed};
use std::os::fd::AsRawFd;
use std::os::raw::{c_char, c_void};
use uinput_ioctls::*;

// Constants (same numeric values as in linux headers)
pub const EV_SYN: u16 = 0x00;
pub const EV_KEY: u16 = 0x01;
pub const EV_REL: u16 = 0x02;
pub const EV_ABS: u16 = 0x03;
pub const EV_FF: u16 = 0x15;
pub const SYN_REPORT: u16 = 0;
pub const BUS_USB: u16 = 0x03;
pub const SYS_INPUT_DIR: &str = "/sys/devices/virtual/input/";

// Absolute Axes
pub const ABS_X: u16 = 0x00;
pub const ABS_Y: u16 = 0x01;
pub const ABS_Z: u16 = 0x02;
pub const ABS_RX: u16 = 0x03;
pub const ABS_RY: u16 = 0x04;
pub const ABS_RZ: u16 = 0x05;
pub const ABS_HAT0X: u16 = 0x10;
pub const ABS_HAT0Y: u16 = 0x11;

/// Struct holding device state
pub struct DeviceState {
    pub uinput_fd: i32,
    pub sysname: String,
    pub device_name: String,
    pub event_device_node: String,
    pub event_device_fd: i32,
    pub events: Vec<LoggedInputEvent>,
}

/// Trait for input devices
pub trait Device: Sized {
    fn name() -> &'static str;

    /// open uinput, configure keys, call ui_dev_setup, call ui_dev_create, get sysname and devnode
    /// open event device
    fn create(device: Option<&str>, name: &str) -> Result<Self, io::Error>;

    /// call ui_dev_destroy and close fds
    fn destroy(self);

    /// Get the device state for internal operations
    fn state(&self) -> &DeviceState;

    /// Get mutable access to device state for updating events
    fn state_mut(&mut self) -> &mut DeviceState;

    /// Get the uinput file descriptor
    fn uinput_fd(&self) -> i32 {
        self.state().uinput_fd
    }

    /// Get the sysname path
    fn sysname(&self) -> &str {
        &self.state().sysname
    }

    /// Get the device name
    fn device_name(&self) -> &str {
        &self.state().device_name
    }

    fn get_event_device(&self) -> Result<c_int, io::Error>;

    /// Emit an event to the device
    fn emit(&self, ev_type: u16, code: u16, val: i32) -> io::Result<()> {
        emit(self.uinput_fd(), ev_type, code, val)
    }

    /// Read an event from the event device
    fn read_event(&self) -> io::Result<input_event> {
        let event_device_fd = self.get_event_device()?;
        read_event(event_device_fd)
    }

    /// Emit (to uinput) and read (from evdev) an event with logging
    fn emit_read_and_log(
        &mut self,
        ev_type: u16,
        code: u16,
        val: i32,
    ) -> io::Result<LoggedInputEvent> {
        let event_device_fd = self.get_event_device()?;
        let event = emit_read_and_log(self.uinput_fd(), event_device_fd, ev_type, code, val, true)?;
        self.state_mut().events.push(event.clone());
        Ok(event)
    }

    /// Emit and read an event with logging
    fn emit_to_evdev_read_from_uinput_and_log(
        &mut self,
        ev_type: u16,
        code: u16,
        val: i32,
    ) -> io::Result<LoggedInputEvent> {
        let event_device_fd = self.get_event_device()?;
        let event =
            emit_read_and_log(event_device_fd, self.uinput_fd(), ev_type, code, val, false)?;
        self.state_mut().events.push(event.clone());
        Ok(event)
    }

    /// Get the event log
    fn event_log(&self) -> &[LoggedInputEvent] {
        &self.state().events
    }

    /// Reset the event log
    fn reset_event_log(&mut self) {
        self.state_mut().events.clear();
    }

    /// Get the event log as mutable slice
    fn event_log_mut(&mut self) -> &mut Vec<LoggedInputEvent> {
        &mut self.state_mut().events
    }

    /// Setup the uinput device (calls ui_dev_setup and ui_get_sysname)
    fn setup_device(
        &self,
        name: &str,
        vendor: u16,
        product: u16,
        bustype: u16,
        ff_effects_max: u32,
    ) -> io::Result<()> {
        unsafe {
            let mut usetup: uinput_setup = zeroed();
            usetup.id.bustype = bustype;
            usetup.id.vendor = vendor;
            usetup.id.product = product;
            usetup.ff_effects_max = ff_effects_max;

            let name_cstr = CString::new(name).unwrap();
            let name_ptr = usetup.name.as_mut_ptr() as *mut c_char;
            std::ptr::copy_nonoverlapping(
                name_cstr.as_ptr(),
                name_ptr,
                name_cstr.to_bytes_with_nul().len(),
            );

            let usetup_ptr = &mut usetup as *mut uinput_setup;
            ui_dev_setup(self.uinput_fd(), usetup_ptr).map_err(|e| {
                eprintln!("ui_dev_setup failed: {:?}", e);
                e
            })?;
            Ok(())
        }
    }

    /// Get the sysname from the uinput fd
    fn get_sysname(&self) -> io::Result<String> {
        unsafe {
            let mut resultbuf: [c_char; 64] = [0; 64];
            ui_get_sysname(self.uinput_fd(), resultbuf.as_mut_slice()).map_err(|e| {
                eprintln!("ui_get_sysname failed: {:?}", e);
                e
            })?;
            Ok(format!(
                "{}{}",
                SYS_INPUT_DIR,
                CStr::from_ptr(resultbuf.as_ptr()).to_string_lossy()
            ))
        }
    }
}

/// Emit an event to the uinput device
pub fn emit(fd: c_int, ev_type: u16, code: u16, val: i32) -> io::Result<()> {
    let mut ie: libc::input_event = unsafe { zeroed() };

    ie.time.tv_sec = 0;
    ie.time.tv_usec = 0;

    ie.type_ = ev_type;
    ie.code = code;
    ie.value = val;

    let buf_ptr = &ie as *const libc::input_event as *const c_void;
    let bytes = size_of::<libc::input_event>();

    let written = unsafe { write(fd, buf_ptr, bytes) };
    if written as usize != bytes {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Emit event, sync, and read back with logging
pub fn emit_read_and_log(
    emit_to: c_int,
    read_from: c_int,
    ev_type: u16,
    code: u16,
    val: i32,
    emit_syn: bool,
) -> io::Result<LoggedInputEvent> {
    let (time_sent_sec, time_sent_nsec) = monotonic_time();
    emit(emit_to, ev_type, code, val)?;
    if emit_syn {
        emit(emit_to, EV_SYN, SYN_REPORT, 0)?;
    }
    let input_event_recv = read_event(read_from).unwrap();
    if emit_syn {
        let _syn_recv = read_event(read_from).unwrap();
    }
    let (time_recv_sec, time_recv_nsec) = monotonic_time();
    let duration_usec =
        (time_recv_sec - time_sent_sec) * 1_000_000 + (time_recv_nsec - time_sent_nsec) / 1000;
    let send_and_receive_match = input_event_recv.type_ == ev_type
        && input_event_recv.code == code
        && input_event_recv.value == val;

    Ok(LoggedInputEvent {
        tv_sec: time_sent_sec,
        tv_nsec: time_sent_nsec,
        duration_usec,
        type_: ev_type,
        code,
        value: val,
        send_and_receive_match,
    })
}

/// Read an event from the event device
pub fn read_event(event_dev_fd: c_int) -> io::Result<input_event> {
    let mut ev: input_event = unsafe { zeroed() };
    let ret = unsafe {
        libc::read(
            event_dev_fd,
            &mut ev as *mut _ as *mut c_void,
            size_of::<input_event>(),
        )
    };
    if ret as usize != size_of::<input_event>() {
        return Err(io::Error::last_os_error());
    }
    Ok(ev)
}

/// Get monotonic time
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

/// Open uinput device
pub fn open_uinput(device: Option<&str>) -> io::Result<i32> {
    let device = match device {
        Some(dev_path) => dev_path,
        _ => "/dev/uinput",
    };

    let path = std::ffi::CString::new(device).unwrap();
    let fd = unsafe { open(path.as_ptr(), O_RDWR | O_NONBLOCK) };
    if fd < 0 {
        eprintln!("error opening uinput");
        return Err(io::Error::last_os_error());
    }
    Ok(fd)
}

/// Fetch the event device node from the sysname path
pub fn fetch_device_node(sysname: &str) -> io::Result<String> {
    use std::fs;
    use std::io::ErrorKind;

    for entry in fs::read_dir(sysname)? {
        let entry = entry?;
        if let Some(name) = entry.file_name().to_str() {
            if name.starts_with("event") {
                return Ok(format!("/dev/input/{}", name));
            }
        }
    }
    Err(io::Error::new(ErrorKind::NotFound, "no device found"))
}
