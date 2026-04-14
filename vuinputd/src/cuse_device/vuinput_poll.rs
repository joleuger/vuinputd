// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use crate::cuse_device::*;
use crate::global_config::get_device_policy;
use ::cuse_lowlevel::*;
use libc::{__s32, __u16, input_event};
use libc::{off_t, size_t, EIO};
use libc::{uinput_abs_setup, uinput_setup};
use log::{debug, trace};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::raw::c_char;
use std::ptr::NonNull;
use uinput_ioctls::*;

// https://github.com/libfuse/libfuse/blob/master/example/poll.c
pub unsafe extern "C" fn vuinput_poll(
    req: fuse_lowlevel::fuse_req_t,
    fi: *mut fuse_lowlevel::fuse_file_info,
    ph: *mut fuse_lowlevel::fuse_pollhandle,
) {
    fuse_lowlevel::fuse_reply_err(req, EIO);
    return;

    let vuinput_state_mutex =
        get_vuinput_state(&VuFileHandle::from_fuse_file_info(fi.as_ref().unwrap())).unwrap();
    let mut vuinput_state = vuinput_state_mutex.lock().unwrap();

    debug!("poll");

    match vuinput_state.poll.pollphase {
        PollPhase::Empty => {
            let ph = NonNull::<fuse_lowlevel::fuse_pollhandle>::new(ph);
            vuinput_state.poll.add_waiter(ph.unwrap());
        }
        PollPhase::Readable => {
            fuse_lowlevel::fuse_lowlevel_notify_poll(ph);
            fuse_lowlevel::fuse_pollhandle_destroy(ph);
        }
        PollPhase::Reading => {
            fuse_lowlevel::fuse_lowlevel_notify_poll(ph);
            fuse_lowlevel::fuse_pollhandle_destroy(ph);
        }
    }
}
