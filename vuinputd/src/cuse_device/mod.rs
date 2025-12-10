// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

pub mod state;
pub mod vuinput_ioctl;
pub mod vuinput_write;
pub mod vuinput_release;
pub mod vuinput_open;

use std::{fs, io};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::io::{ErrorKind};

use ::cuse_lowlevel::*;
use state::*;

pub const BUS_USB: u16 = 0x03;


// Instance of cuse_lowlevel_ops with all stubs assigned.
// Setting to None leads to e.g. "write error: Function not implemented".
// You can find the implementations of the uinput default (open, release ,read, write, poll,
// and ioctl) in uinput_fops of uinput.c.
// See: https://github.com/torvalds/linux/blob/master/drivers/input/misc/uinput.c,
pub fn vuinput_make_cuse_ops() -> cuse_lowlevel::cuse_lowlevel_ops {
    cuse_lowlevel::cuse_lowlevel_ops {
        init: None,
        init_done: None,
        destroy: None,
        open: Some(vuinput_open::vuinput_open),
        read: None,
        write: Some(vuinput_write::vuinput_write),
        flush: None,
        release: Some(vuinput_release::vuinput_release),
        fsync: None,
        ioctl: Some(vuinput_ioctl::vuinput_ioctl),
        poll: None,
    }
}
