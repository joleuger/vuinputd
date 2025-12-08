// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::sync::OnceLock;
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
use std::sync::{Arc, Mutex, RwLock};
use uinput_ioctls::*;

use crate::process_tools::{Pid, get_requesting_process};
use crate::cuse_device::*;

pub static VUINPUT_COUNTER: OnceLock<AtomicU64> = OnceLock::new();

fn get_fresh_filehandle() -> u64 {
    let ctr = VUINPUT_COUNTER.get().unwrap();
    ctr.fetch_add(1, Ordering::SeqCst).into()
}

pub unsafe extern "C" fn vuinput_open(
    _req: fuse_lowlevel::fuse_req_t,
    _fi: *mut fuse_lowlevel::fuse_file_info,
) {
    let fh = get_fresh_filehandle();
    let ctx = fuse_lowlevel::fuse_req_ctx(_req);
    debug!("fh {}: opened by process id {} (host view)", fh, (*ctx).pid);
    let requesting_process = get_requesting_process(Pid::Pid((*ctx).pid));
    debug!("fh {}: namespaces {}", fh, requesting_process);
    // namespaces net:4026531840, uts:4026531838, ipc:4026531839, pid:4026531836, pid_for_children:4026531836, user:4026531837, mnt:4026531841, cgroup:4026531835, time:4026531834, time_for_children:4026531834
    (*_fi).fh = fh;
    // Open the path in read-only mode, returns `io::Result<File>`
    let open_vuinput_result = OpenOptions::new()
        .read(true)
        .write(true)
        //.custom_flags(O_NONBLOCK)
        .custom_flags(O_CLOEXEC)
        .open(Path::new("/dev/uinput"));
    match open_vuinput_result {
        Ok(v) => {
            insert_vuinput_state(
                &VuFileHandle::from_fuse_file_info(_fi.as_ref().unwrap()),
                VuInputState {
                    file: v,
                    requesting_process,
                    input_device: None
                },
            )
            .unwrap();
            fuse_lowlevel::fuse_reply_open(_req, _fi);
        }
        Err(e) => {
            error!("couldn't open /dev/uinput: {}", e);
            fuse_lowlevel::fuse_reply_err(_req, ENOENT);
        }
    }
}