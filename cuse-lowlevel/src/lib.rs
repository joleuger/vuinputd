// SPDX-License-Identifier: MIT
//
// Author: Richard Wiedenhöft <richard@wiedenhoeft.xyz>
// Author: Johannes Leupolz <dev@leupolz.eu>
//
// This library is heavily baased on https://github.com/richard-w/libfuse-sys
// but adopted to only provide the low-level modules of fuse and cuse.

#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(clippy::useless_transmute)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::missing_safety_doc)]

use libc::*;

pub mod fuse_lowlevel {
    use super::*;
    include!(concat!(env!("OUT_DIR"), "/fuse_lowlevel.rs"));
}

pub mod cuse_lowlevel {
    use super::*;
    include!(concat!(env!("OUT_DIR"), "/cuse_lowlevel.rs"));

    use fuse_lowlevel::{
        fuse_args, fuse_conn_info, fuse_file_info, fuse_pollhandle, fuse_req_t, fuse_session,
    };
}
