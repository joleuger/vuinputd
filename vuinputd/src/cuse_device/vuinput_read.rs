// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use crate::cuse_device::*;
use ::cuse_lowlevel::*;
use libc::{__s32, __u16, input_event, EAGAIN};
use libc::{off_t, size_t, EIO};
use log::{debug, trace};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use uinput_ioctls::*;

// TODO: compat-mode+ ensure sizeof(struct input_event)
pub unsafe extern "C" fn vuinput_read(
    _req: fuse_lowlevel::fuse_req_t,
    _size: size_t,
    _off: off_t,
    _fi: *mut fuse_lowlevel::fuse_file_info,
) {
    assert!(
        _off == 0,
        "vuinput_read: offset needs to be 0 but is {}",
        _off
    );

    let fh = &(*_fi).fh;
    let vuinput_state_mutex =
        get_vuinput_state(&VuFileHandle::from_fuse_file_info(_fi.as_ref().unwrap())).unwrap();
    let mut vuinput_state = vuinput_state_mutex.lock().unwrap();

    const NORMAL_SIZE: usize = std::mem::size_of::<libc::input_event>();
    let is_compat = vuinput_state.requesting_process.is_compat;
    // TODO: ARM: && !compat_uses_64bit_time()

    let mut buffer: [u8; 24] = [0; 24];

    vuinput_state.poll.pollphase = PollPhase::Reading;
    // read up to 24 bytes
    let result = vuinput_state.file.read(&mut buffer);
    match result {
        Ok(NORMAL_SIZE) => {
            if !is_compat {
                let buffer = buffer.as_ptr() as *const i8;
                fuse_lowlevel::fuse_reply_buf(_req, buffer, 24);
            } else {
                debug!(
                    "fh {}: error reading from uinput: not implemented yet for 32 bit users",
                    fh
                );
                // details how to implement it can be found in vuinput_write.rs
                fuse_lowlevel::fuse_reply_err(_req, EIO);
            }
        }
        Err(e) => {
            if e.kind() == io::ErrorKind::WouldBlock {
                // EAGAIN / EWOULDBLOCK
                //println!("Received EAGAIN: The read would block!");
                vuinput_state.poll.pollphase = PollPhase::Empty;
                fuse_lowlevel::fuse_reply_err(_req, EAGAIN);
            } else {
                debug!("fh {}: error reading from uinput: {e:?}", fh);
                fuse_lowlevel::fuse_reply_err(_req, EIO);
            }
        }
        Ok(_) => {
            debug!("fh {}: error reading from uinput: wrong size", fh);
            fuse_lowlevel::fuse_reply_err(_req, EIO);
        }
    }
}
