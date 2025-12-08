// SPDX-License-Identifier: MIT
// vuinputd: container-safe mediation daemon for /dev/uinput
//
// - Exposes a fake /dev/uinput inside the container (via CUSE).
// - Forwards ioctls + writes to the real /dev/uinput on the host.
//
// Author: Johannes Leupolz <dev@leupolz.eu>

// TODOS:
// preliminary close
// remove test
// correct char device for vuinput
// renaming
// use in container
// cancellation token
// distinguish between cleanup jobs that must not be cancelled and other jobs (especially background jobs)
// naming: dev_path vs dev_node. I guess I mean the same.
// Send warning, if udev monitor does not exist


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

pub mod cuse_device;

use crate::cuse_device::vuinput_open::VUINPUT_COUNTER;
use crate::cuse_device::{DEDUP_LAST_ERROR, VUINPUT_STATE, vuinput_make_cuse_ops};
use crate::jobs::inject_in_container_job::InjectInContainerJob;
use crate::jobs::monitor_udev_job::MonitorBackgroundLoop;
use crate::jobs::remove_from_container_job::RemoveFromContainerJob;

pub mod process_tools;

pub mod job_engine;
use crate::job_engine::{JOB_DISPATCHER, job::*};
use crate::process_tools::*;

pub mod jobs;


fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    check_permissions().expect("failed to read the capabilities of the vuinputd process");;

    let args: Vec<String> = std::env::args().collect();

    VUINPUT_STATE.set(RwLock::new(HashMap::new())).expect("failed to initialize global state");
    VUINPUT_COUNTER.set(AtomicU64::new(3)).expect("failed to initialize the counter that provides the values of the CUSE file handles"); // 3, because 1 and 2 are usually STDOUT and STDERR
    JOB_DISPATCHER.set(Mutex::new(Dispatcher::new())).expect("failed to initialize the job dispatcher");
    SELF_NAMESPACES.set(get_namespace(Pid::SelfPid)).expect("failed to retrieve the namespaces of the vuinputd process");
    DEDUP_LAST_ERROR.set(Mutex::new(None)).expect("failed to initialize the log deduplication state");
    JOB_DISPATCHER.get().unwrap().lock().unwrap().dispatch(Box::new(MonitorBackgroundLoop::new()));

    info!("Starting vuinputd");

    let cuse_ops = vuinput_make_cuse_ops();

    let vuinput_devicename = CString::new(format!("DEVNAME=vuinput")).unwrap();

    let mut dev_info_argv: Vec<*const c_char> = vec![
        vuinput_devicename.as_ptr(), // pointer to the C string
        std::ptr::null(),          // null terminator, often required by C APIs
    ];

    // setting dev_major and dev_minor to 0 leads to a dynamic assignment of the major and minor, very likely beginning with 234:0
    // see  in https://www.kernel.org/doc/Documentation/admin-guide/devices.txt
    // major 120 is reserved for local/experimental use. I picked minor 414795 with the use
    // of a random number generator to omit conflicts.
    let ci = cuse_lowlevel::cuse_info {
        dev_major: 120,
        dev_minor: 414795,
        dev_info_argc: 1,
        dev_info_argv: dev_info_argv.as_mut_ptr(),
        flags: cuse_lowlevel::CUSE_UNRESTRICTED_IOCTL,
    };

    let arg_program_name = CString::new(args[0].clone()).unwrap();
    let parg_program_name = arg_program_name.into_raw();
    let arg_foreground = CString::new("-f").unwrap();
    let parg_foreground = arg_foreground.into_raw();
    let arg_singlethreaded = CString::new("-s").unwrap();
    let parg_singlethreaded = arg_singlethreaded.into_raw();
    let mut stripped_argv: Vec<*mut c_char> = vec![
        parg_program_name,
        parg_foreground,
        parg_singlethreaded,
        std::ptr::null_mut(), // null terminator, often required by C APIs
    ];

    unsafe {
        cuse_lowlevel::cuse_lowlevel_main(
            3,
            stripped_argv.as_mut_ptr(),
            &ci,
            &cuse_ops,
            std::ptr::null_mut(),
        );
        let _reclaim_arg_program_name = CString::from_raw(parg_program_name);
        let _reclaim_arg_foreground = CString::from_raw(parg_foreground);
        let _reclaim_arg_singlethreaded = CString::from_raw(parg_singlethreaded);
    }
    info!("Stopping vuinputd");
    JOB_DISPATCHER.get().unwrap().lock().unwrap().close();
    JOB_DISPATCHER.get().unwrap().lock().unwrap().wait_until_finished();

    Ok(())
}
