// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

pub use crate::devices::device_base::*;
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

// Re-export constants from device_base for backward compatibility
pub use crate::devices::device_base::{BUS_USB, EV_ABS, EV_FF, EV_KEY, EV_REL, EV_SYN, SYN_REPORT};
