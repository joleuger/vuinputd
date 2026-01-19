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
// Filter out Ctrl+Alt+Fx. "sysrq" keys or the low-level VT switching combos.

use ::cuse_lowlevel::*;
use base64::prelude::BASE64_STANDARD;
use base64::Engine as _;
use log::info;
use std::ffi::CString;
use std::os::raw::c_char;
use std::sync::atomic::AtomicU64;
use std::sync::Mutex;

pub mod cuse_device;

use crate::cuse_device::state::{initialize_dedup_last_error, initialize_vuinput_state};
use crate::cuse_device::vuinput_make_cuse_ops;
use crate::cuse_device::vuinput_open::VUINPUT_COUNTER;
use crate::global_config::{DevicePolicy, Placement};
use crate::jobs::monitor_udev_job::MonitorBackgroundLoop;

pub mod process_tools;

pub mod job_engine;
use crate::job_engine::{job::*, JOB_DISPATCHER};
use crate::process_tools::*;

pub mod actions;

pub mod global_config;
pub mod jobs;
pub mod vt_tools;

use clap::Parser;

const DEV_PREFIX: &str = "/dev/";
const DEVNAME_MAX_LEN: usize = 128 - DEV_PREFIX.len();

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Args {
    /// Major device number
    #[arg(long)]
    major: Option<u32>,

    /// Minor device number
    #[arg(long)]
    minor: Option<u32>,

    /// Device name (without /dev/)
    #[arg(long)]
    devname: Option<String>,

    /// Action to execute (JSON encoded). Note that this excludes all other options.
    #[arg(long, value_name = "JSON")]
    pub action: Option<String>,

    /// Action to execute (base64-encoded JSON). Note that this excludes all other options.
    #[arg(long = "action-base64", value_name = "BASE64")]
    pub action_base64: Option<String>,

    /// Path to the target process's /proc/<pid>/ns directory used as namespace source.
    #[arg(
        long = "target-namespace",
        value_name = "NS_PATH",
        help = "Path to /proc/<pid>/ns used as the namespace source (e.g. /proc/1234/ns or /proc/self/ns)"
    )]
    pub target_namespace: Option<String>,

    #[arg(
        long = "vt-guard",
        help = "Prevent all keyboard input from reaching the VT by setting K_OFF on /dev/tty0.",
        long_help = "Disable VT keyboard handling (K_OFF on /dev/tty0) to prevent uinput leakage.\n\
                 This disables all keyboard input on the virtual terminals, including physical keyboards.\n\
                 Loss of local access may require recovery via SSH or a rescue boot."
    )]
    pub vt_guard: bool,

    /// Enforce a device policy on created devices
    #[arg(long, value_enum, default_value_t)]
    device_policy: DevicePolicy,

    /// Placement of device nodes and udev data
    #[arg(long, value_enum, default_value_t)]
    pub placement: Placement,
}

fn validate_args(args: &Args) -> Result<(), String> {
    let action: &Option<String> = match (&args.action, &args.action_base64) {
        (None, None) => &None,
        (None, Some(_)) => &args.action_base64,
        (Some(_), None) => &args.action,
        (Some(_), Some(_)) => {
            return Err("--action and --action-base64 may not be used together".into());
        }
    };

    // action might only occur with target-namespace
    match (
        &args.major,
        &args.minor,
        &args.devname,
        action,
        &args.target_namespace,
    ) {
        (None, None, None, Some(_), _) => {}
        (_, _, _, None, None) => {}
        _ => {
            return Err("--action or --action-base64 must not be used in combination with any other argument other than target-namespace".into());
        }
    }

    // major/minor must appear together
    match (&args.major, &args.minor) {
        (Some(_), Some(_)) | (None, None) => {}
        _ => {
            return Err("--major and --minor must be specified together or not at all".into());
        }
    }

    // devname length constraint
    if let Some(devname) = &args.devname {
        if devname.len() >= DEVNAME_MAX_LEN {
            return Err(format!(
                "--devname must be shorter than {} bytes",
                DEVNAME_MAX_LEN
            ));
        }
    }

    Ok(())
}

fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    let args = Args::parse();
    let argv0 = std::env::args_os()
        .next()
        .expect("Couldn't retrieve program name");

    if let Err(e) = validate_args(&args) {
        eprintln!("Error: {e}");
        std::process::exit(2);
    }

    let action = match (&args.action, &args.action_base64) {
        (Some(json), None) => Some(json.clone()),
        (None, Some(b64)) => {
            let decoded = BASE64_STANDARD
                .decode(&b64)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

            let decoded = String::from_utf8(decoded)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            Some(decoded)
        }
        (None, None) => None,
        _ => unreachable!("validate_args enforces mutual exclusion"),
    };

    if action.is_some() {
        if let Some(target_namespace) = args.target_namespace {
            process_tools::run_in_net_and_mnt_namespace(target_namespace.as_str()).unwrap();
        }
        let error_code = actions::handle_action::handle_cli_action(action.unwrap());
        std::process::exit(error_code);
    }

    if args.vt_guard {
        vt_tools::mute_keyboard()?;
        std::process::exit(0);
    }

    check_permissions().expect("failed to read the capabilities of the vuinputd process");
    vt_tools::check_vt_status();

    global_config::initialize_global_config(&args.device_policy, &args.placement);
    initialize_vuinput_state();
    VUINPUT_COUNTER.set(AtomicU64::new(3)).expect(
        "failed to initialize the counter that provides the values of the CUSE file handles",
    ); // 3, because 1 and 2 are usually STDOUT and STDERR
    JOB_DISPATCHER
        .set(Mutex::new(Dispatcher::new()))
        .expect("failed to initialize the job dispatcher");
    SELF_NAMESPACES
        .set(get_namespace(Pid::SelfPid))
        .expect("failed to retrieve the namespaces of the vuinputd process");
    initialize_dedup_last_error();
    JOB_DISPATCHER
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .dispatch(Box::new(MonitorBackgroundLoop::new()));

    info!("Starting vuinputd");

    let cuse_ops = vuinput_make_cuse_ops();

    let vuinput_devicename = match &args.devname {
        None => "vuinput",
        Some(devname) => devname,
    };
    let vuinput_devicename = CString::new(format!("DEVNAME={}", vuinput_devicename)).unwrap();

    let mut dev_info_argv: Vec<*const c_char> = vec![
        vuinput_devicename.as_ptr(), // pointer to the C string
        std::ptr::null(),            // null terminator, often required by C APIs
    ];

    // setting dev_major and dev_minor to 0 leads to a dynamic assignment of the major and minor, very likely beginning with 234:0
    // see  in https://www.kernel.org/doc/Documentation/admin-guide/devices.txt
    let (major, minor) = match ((&args).major, (&args).minor) {
        (Some(major), Some(minor)) => (major, minor),
        _ => (0, 0),
    };
    let ci = cuse_lowlevel::cuse_info {
        dev_major: major,
        dev_minor: minor,
        dev_info_argc: 1,
        dev_info_argv: dev_info_argv.as_mut_ptr(),
        flags: cuse_lowlevel::CUSE_UNRESTRICTED_IOCTL,
    };

    let arg_program_name = CString::new(argv0.as_encoded_bytes()).unwrap();
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
    JOB_DISPATCHER
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .wait_until_finished();

    Ok(())
}
