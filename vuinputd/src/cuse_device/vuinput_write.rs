// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use libc::{O_CLOEXEC, input_id};
use libc::{iovec, off_t, size_t, EBADRQC, EIO, ENOENT};
use libc::{uinput_abs_setup, uinput_ff_erase, uinput_ff_upload, uinput_setup};
use ::cuse_lowlevel::*;
use log::{debug, error, info, trace};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::{fs, ptr};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::io::{self, ErrorKind};
use std::os::fd::AsRawFd;
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use uinput_ioctls::*;
use libc::{__s32, __u16, c_ulong, input_event};
use crate::cuse_device::*;



// TODO: compat-mode+ ensure sizeof(struct input_event)
pub unsafe extern "C" fn vuinput_write(
    _req: fuse_lowlevel::fuse_req_t,
    _buf: *const c_char,
    _size: size_t,
    _off: off_t,
    _fi: *mut fuse_lowlevel::fuse_file_info,
) {
    assert!(
        _off == 0,
        "vuinput_write: offset needs to be 0 but is {}",
        _off
    );
    
    let fh = &(*_fi).fh;
    let slice = std::slice::from_raw_parts(_buf as *const u8, _size);
    let vuinput_state_mutex = get_vuinput_state(&VuFileHandle::from_fuse_file_info(_fi.as_ref().unwrap())).unwrap();
    let mut vuinput_state = vuinput_state_mutex.lock().unwrap();
    
    if vuinput_state.input_device.is_none() {
        debug!("{}: legacy device setup recognized! Ignore the data and use hardcoded values",fh);

        assert!(_size == std::mem::size_of::<libc::uinput_user_dev>());
        let legacy_uinput_user_dev = _buf as *const libc::uinput_user_dev;

        let mut usetup: uinput_setup = unsafe { std::mem::zeroed() };
        usetup.id.bustype = BUS_USB;
        // The pid is registered for vuinputd, see https://pid.codes/1209/5020/
        usetup.id.vendor = 0x1209;
        usetup.id.product = 0x5020;
        usetup.id.version = (*legacy_uinput_user_dev).id.version;
        usetup.ff_effects_max=(*legacy_uinput_user_dev).ff_effects_max;
        usetup.name=(*legacy_uinput_user_dev).name;

        // Call IOCTLs to setup and create the device
        // Assuming your wrappers accept (fd, ptr_to_usetup) etc.
        // We'll pass pointer to usetup
        let usetup_ptr = &mut usetup as *mut uinput_setup;
        let fd = vuinput_state.file.as_raw_fd();
        ui_dev_setup(fd, usetup_ptr).unwrap();

        // setup abs
        for code in  0..libc::ABS_CNT{
            if (*legacy_uinput_user_dev).absmax[code] != 0 || (*legacy_uinput_user_dev).absmin[code] != 0 {
                let mut abs_setup: uinput_abs_setup = unsafe { std::mem::zeroed() };
                abs_setup.code=code.try_into().unwrap();
                abs_setup.absinfo.maximum = (*legacy_uinput_user_dev).absmax[code];
                abs_setup.absinfo.minimum = (*legacy_uinput_user_dev).absmin[code];
                abs_setup.absinfo.fuzz = (*legacy_uinput_user_dev).absfuzz[code];
                abs_setup.absinfo.flat = (*legacy_uinput_user_dev).absflat[code];

                let abs_setup_ptr = &mut abs_setup as *mut uinput_abs_setup;
                ui_abs_setup(fd, abs_setup_ptr).unwrap();
            }
        }

        fuse_lowlevel::fuse_reply_write(_req, _size);
        return;
    }

    let mut bytes = 0;
    let mut result = Result::Ok(0);

    let compat_size= std::mem::size_of::<input_event_compat>();
    let normal_size= std::mem::size_of::<libc::input_event>();
    let is_compat = vuinput_state.requesting_process.is_compat;
    // TODO: ARM: && !compat_uses_64bit_time()
    
    if !is_compat {
        while bytes + normal_size <= _size && result.is_ok() {
            result = vuinput_state.file.write(&slice[bytes..bytes + normal_size]);
            bytes += normal_size; 
        }
    } else {
        while bytes + compat_size <= _size && result.is_ok() {
            let position= _buf.byte_add(bytes);
            let compat = position as *const input_event_compat;
            let normal = map_to_64_bit(&*compat);
            let normal_ptr=(&normal as *const libc::input_event) as *const u8;
            let slice = std::slice::from_raw_parts(normal_ptr,normal_size);
            result = vuinput_state.file.write(&slice);
            bytes += compat_size; 
        }
    };
    
    match result {
        Ok(_) => {
            trace!("wrote {} of {} bytes (compat {})", bytes,_size,is_compat);
            fuse_lowlevel::fuse_reply_write(_req, bytes);
        }
        Err(e) => {
            let mut last_error = DEDUP_LAST_ERROR.get().unwrap().lock().unwrap();
            
            match *last_error {
                Some((last_fh,VuError::WriteError)) if *fh == last_fh => {},
                _ => {debug!("fh {}: error writing to uinput: {e:?}",fh);}
            }
            
            *last_error = Some((*fh,VuError::WriteError));
            
            fuse_lowlevel::fuse_reply_err(_req, EIO);
        }
    }
}



#[repr(C)]
pub struct input_event_compat {
    pub input_event_sec: u32,
    pub input_event_usec: u32,
    pub type_: __u16,
    pub code: __u16,
    pub value: __s32,
}

// this is static for the architecture
pub fn compat_uses_64bit_time() -> bool {
    let uname = nix::sys::utsname::uname().unwrap();
    let arch = uname.machine().to_str().unwrap();

    match arch {
        "x86_64" => false,
        "ppc64" => false, // some setups still 32-bit time_t
        _ => true, // arm64, riscv64, s390x all use 64-bit
    }
}

pub fn map_to_64_bit(compat: &input_event_compat) -> input_event{
    let mut mapped: input_event = unsafe { std::mem::zeroed() };
    mapped.time.tv_sec=compat.input_event_sec.into();
    mapped.time.tv_usec=compat.input_event_usec.into();
    mapped.type_=compat.type_;
    mapped.code=compat.code;
    mapped.value=compat.value;

    mapped
}