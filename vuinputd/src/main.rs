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


use libc::O_CLOEXEC;
use libc::{iovec, off_t, size_t, EBADRQC, EIO, ENOENT};
use libc::{uinput_abs_setup, uinput_ff_erase, uinput_ff_upload, uinput_setup};
use libfuse_sys::cuse_lowlevel;
use libfuse_sys::fuse_lowlevel;
use log::{debug, error, info, trace};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::io::{self, ErrorKind};
use std::os::fd::AsRawFd;
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use uinput_ioctls::*;

pub mod namespace;
pub mod monitor_udev;
use crate::container::inject_in_container_job::InjectInContainerJob;
use crate::container::netlink_message;
use crate::container::remove_from_container_job::RemoveFromContainerJob;
use crate::jobs::closure_job::ClosureJob;
use crate::monitor_udev::MonitorBackgroundLoop;
use crate::namespace::*;

pub mod jobs;
use crate::jobs::job::*;

pub mod container;


#[derive(Debug)]
struct VuInputDevice {
    cuse_fh : u64,
    major : u64,
    minor : u64,
    syspath: String,
    devnode: String,
    runtime_data: Option<String>,
    netlink_data: Option<String>
}

#[derive(Debug)]
struct VuInputState {
    file: File,
    ns_of_requestor: Namespaces,
    input_device: Option<VuInputDevice>
}

#[derive(Debug,Eq, Hash, PartialEq, Clone)]
enum VuFileHandle {
    Fh(u64)
}

impl VuFileHandle {
    fn from_fuse_file_info(fi: &fuse_lowlevel::fuse_file_info) -> VuFileHandle {
        VuFileHandle::Fh(fi.fh)
    }
}

impl std::fmt::Display for VuFileHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VuFileHandle::Fh(fh) => writeln!(f, "VuFileHandle({:?})",fh)?,
        }
        Ok(())
    }
}

static VUINPUT_COUNTER: OnceLock<AtomicU64> = OnceLock::new();
static VUINPUT_STATE: OnceLock<RwLock<HashMap<VuFileHandle, Arc<Mutex<VuInputState>>>>> = OnceLock::new();
static JOB_DISPATCHER: OnceLock<Mutex<Dispatcher>>= OnceLock::new();
static SELF_NAMESPACES: OnceLock<Namespaces>= OnceLock::new();


const SYS_INPUT_DIR: &str = "/sys/devices/virtual/input/";

fn get_vuinput_state(
    fh:&VuFileHandle,
) -> Result<Arc<Mutex<VuInputState>>, String> {
    let map = VUINPUT_STATE
        .get()
        .ok_or("global not initialized".to_string())?;
    let guard = map.read().map_err(|e| e.to_string())?;
    guard
        .get(&fh)
        .cloned()
        .ok_or("handle not opened".to_string())
}

fn get_fresh_filehandle() -> u64 {
    let ctr = VUINPUT_COUNTER.get().unwrap();
    ctr.fetch_add(1, Ordering::SeqCst).into()
}

fn insert_vuinput_state(
    fh:&VuFileHandle,
    state: VuInputState,
) -> Result<(), String> {
    let map = VUINPUT_STATE
        .get()
        .ok_or("global not initialized".to_string())?;
    let mut guard = map.write().map_err(|e| e.to_string())?;

    if guard.contains_key(&fh) {
        return Err(format!(
            "file handle {} already exists. file handles must not be reused!",
            &fh
        ));
    }

    let _ = guard.insert(fh.clone(), Arc::new(Mutex::new(state)));
    Ok(())
}

fn remove_vuinput_state(
    fh:&VuFileHandle,
) -> Result<Arc<Mutex<VuInputState>>, String> {
    let map = VUINPUT_STATE
        .get()
        .ok_or("global not initialized".to_string())?;
    let mut guard = map.write().map_err(|e| e.to_string())?;
    let old_value = guard.remove(&fh).ok_or("fh unknown")?;
    Ok(old_value)
}

fn fetch_device_node(path: &str) -> io::Result<String> {
    for entry in fs::read_dir(path)? {
        let entry = entry?; // propagate per-entry errors
        if let Some(name) = entry.file_name().to_str() {
            if name.starts_with("event") {
                return Ok(format!("/dev/input/{}", name));
            }
        }
    }
    // If no device is found, return an error
    Err(io::Error::new(ErrorKind::NotFound, "no device found"))
}

/// Returns (major, minor) numbers of a device node at `path`
fn fetch_major_minor(path: &str) -> io::Result<(u64, u64)> {
    let metadata = fs::metadata(path)?;

    // Ensure it's a character device
    if !metadata.file_type().is_char_device() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Not a character device",
        ));
    }

    let rdev = metadata.rdev();
    let major = ((rdev >> 8) & 0xfff) as u64;
    let minor = ((rdev & 0xff) | ((rdev >> 12) & 0xfff00)) as u64;

    Ok((major, minor))
}

unsafe extern "C" fn vuinput_open(
    _req: fuse_lowlevel::fuse_req_t,
    _fi: *mut fuse_lowlevel::fuse_file_info,
) {
    let fh = get_fresh_filehandle();
    let ctx = fuse_lowlevel::fuse_req_ctx(_req);
    debug!("fh {}: opened by process id {} (host view)", fh, (*ctx).pid);
    let namespaces = get_namespaces(Pid::Pid((*ctx).pid));
    debug!("fh {}: namespaces {}", fh, namespaces);
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
                    ns_of_requestor: namespaces,
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
unsafe extern "C" fn vuinput_write(
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
    let slice = std::slice::from_raw_parts(_buf as *const u8, _size);
    let vuinput_state_mutex = get_vuinput_state(&VuFileHandle::from_fuse_file_info(_fi.as_ref().unwrap())).unwrap();
    let mut vuinput_state = vuinput_state_mutex.lock().unwrap();
    
    assert!(
        vuinput_state.input_device.is_some(),
        "legacy device setup not supported, yet!"
    );
    let result = vuinput_state.file.write_all(slice);

    match result {
        Ok(_) => {
            trace!("wrote {} bytes", _size);
            fuse_lowlevel::fuse_reply_write(_req, _size);
        }
        Err(e) => {
            debug!("error writing to uinput: {e:?}");
            fuse_lowlevel::fuse_reply_err(_req, EIO);
        }
    }
}

unsafe extern "C" fn vuinput_release(
    _req: fuse_lowlevel::fuse_req_t,
    _fi: *mut fuse_lowlevel::fuse_file_info,
) {
    let fh = &(*_fi).fh;
    let vuinput_state_mutex = remove_vuinput_state(&VuFileHandle::from_fuse_file_info(_fi.as_ref().unwrap())).unwrap();
    
    let mut vuinput_state = vuinput_state_mutex.lock().unwrap();

    // remove device in container, if the request was really from another namespace
    if ! SELF_NAMESPACES.get().unwrap().equal_mnt_and_net(&vuinput_state.ns_of_requestor) {
        let input_device = vuinput_state.input_device.as_ref().unwrap();
        let remove_job=RemoveFromContainerJob::new(vuinput_state.ns_of_requestor.clone(),input_device.devnode.clone(),input_device.syspath.clone(),input_device.major,input_device.minor);
        JOB_DISPATCHER.get().unwrap().lock().unwrap().dispatch(Box::new(remove_job));
    }

    drop(vuinput_state);

    debug!(
        "{}: references left before releasing device {} (expected is 1)",
        fh,
        Arc::strong_count(&vuinput_state_mutex)
    );
    drop(vuinput_state_mutex); // this also closes the file when no other references are open
    // TODO: maybe also ensure that nothing is left in the containers
    fuse_lowlevel::fuse_reply_err(_req, 0);
}

unsafe extern "C" fn vuinput_ioctl(
    _req: fuse_lowlevel::fuse_req_t,
    _cmd: c_int,
    _arg: *mut c_void, //note: this is a pointer in the application space and should not be dereferenced at all
    _fi: *mut fuse_lowlevel::fuse_file_info,
    _flags: c_uint,
    _in_buf: *const c_void, // note: this was mapped by the kernel and can be read from
    _in_bufsz: size_t,
    _out_bufsz: size_t,
) {
    // fuse_reply_ioctl_retry is only necessary for variable length commands;
    // see comment "Now check variable-length commands" in uinput.c of the linux kernel.
    // Those are UI_GET_SYSNAME and UI_ABS_SETUP as of v0.4.

    // ioctl to map are listed on https://www.freedesktop.org/software/libevdev/doc/latest/ioctls.html
    // https://docs.rs/linux-raw-sys/0.11.0/src/linux_raw_sys/x86_64/ioctl.rs.html#529

    let cmd_u64 = (_cmd as c_uint).into();
    // normalize the variable length ones
    let cmd_without_size = cmd_u64 & !(nix::sys::ioctl::SIZEMASK << nix::sys::ioctl::SIZESHIFT);
    let cmd_normalized = match cmd_without_size {
        UI_GET_SYSNAME_WITHOUT_SIZE => UI_GET_SYSNAME_WITHOUT_SIZE,
        //UI_ABS_SETUP => UI_ABS_SETUP_WITHOUT_SIZE,
        _ => cmd_u64,
    };
    let vufh= VuFileHandle::from_fuse_file_info(_fi.as_ref().unwrap());
    let vuinput_state_mutex = get_vuinput_state(&vufh).unwrap();
    let fh = &(*_fi).fh;
    let mut vuinput_state = vuinput_state_mutex.lock().unwrap();

    // ensure for all ioctls that need mapped data, that we have the data correctly mapped
    match (_in_bufsz, _out_bufsz, cmd_normalized) {
        (0, _, UI_ABS_SETUP) => {
            //todo: i guess this needs to be reworked as this is variable size. i guess it is not reachable at all
            debug!("fh {}: submitting _in_bufsz for UI_ABS_SETUP", fh);
            let iov = iovec {
                iov_base: _arg,
                iov_len: ::std::mem::size_of::<uinput_abs_setup>(),
            };
            fuse_lowlevel::fuse_reply_ioctl_retry(_req, &iov, 1, std::ptr::null(), 0);
            return;
        }
        (_, 0, UI_GET_SYSNAME_WITHOUT_SIZE) => {
            let size = (cmd_u64 & nix::sys::ioctl::SIZEMASK) >> nix::sys::ioctl::SIZESHIFT;
            debug!(
                "fh {}: submitting _out_bufsz for UI_GET_SYSNAME({}) ",
                fh, size
            );
            let iov = iovec {
                iov_base: _arg,
                iov_len: 64,
            };
            fuse_lowlevel::fuse_reply_ioctl_retry(_req, std::ptr::null(), 0, &iov, 1);
            return;
        }
        (_, 0, UI_GET_VERSION) => {
            let size = (cmd_u64 & nix::sys::ioctl::SIZEMASK) >> nix::sys::ioctl::SIZESHIFT;
            debug!(
                "fh {}: submitting _out_bufsz for UI_GET_VERSION({}) ",
                fh, size
            );
            let iov = iovec {
                iov_base: _arg,
                iov_len: std::mem::size_of::<c_uint>(),
            };
            fuse_lowlevel::fuse_reply_ioctl_retry(_req, std::ptr::null(), 0, &iov, 1);
            return;
        }
        (0, _, UI_DEV_SETUP) => {
            debug!("fh {}: submitting _in_bufsz for UI_DEV_SETUP", fh);
            let iov = iovec {
                iov_base: _arg,
                iov_len: ::std::mem::size_of::<uinput_setup>(),
            };
            fuse_lowlevel::fuse_reply_ioctl_retry(_req, &iov, 1, std::ptr::null(), 0);
            return;
        }
        (0, _, UI_SET_PHYS) => {
            debug!("fh {}: submitting _in_bufsz for UI_SET_PHYS", fh);
            let iov = iovec {
                iov_base: _arg,
                iov_len: ::std::mem::size_of::<c_char>() * 1024,
            };
            fuse_lowlevel::fuse_reply_ioctl_retry(_req, &iov, 1, std::ptr::null(), 0);
            return;
        }
        (0, _, UI_BEGIN_FF_UPLOAD) => {
            debug!("fh {}: submitting _in_bufsz for UI_BEGIN_FF_UPLOAD", fh);
            let iov = iovec {
                iov_base: _arg,
                iov_len: ::std::mem::size_of::<uinput_ff_upload>(),
            };
            fuse_lowlevel::fuse_reply_ioctl_retry(_req, &iov, 1, &iov, 1);
            return;
        }
        (0, _, UI_END_FF_UPLOAD) => {
            debug!("fh {}: submitting _in_bufsz for UI_END_FF_UPLOAD", fh);
            let iov = iovec {
                iov_base: _arg,
                iov_len: ::std::mem::size_of::<uinput_ff_upload>(),
            };
            fuse_lowlevel::fuse_reply_ioctl_retry(_req, &iov, 1, std::ptr::null(), 0);
            return;
        }
        (0, _, UI_BEGIN_FF_ERASE) => {
            debug!("fh {}: submitting _in_bufsz for UI_BEGIN_FF_ERASE", fh);
            let iov = iovec {
                iov_base: _arg,
                iov_len: ::std::mem::size_of::<uinput_ff_erase>(),
            };
            fuse_lowlevel::fuse_reply_ioctl_retry(_req, &iov, 1, &iov, 1);
            return;
        }
        (0, _, UI_END_FF_ERASE) => {
            debug!("fh {}: submitting _in_bufsz for UI_END_FF_ERASE", fh);
            let iov = iovec {
                iov_base: _arg,
                iov_len: ::std::mem::size_of::<uinput_ff_erase>(),
            };
            fuse_lowlevel::fuse_reply_ioctl_retry(_req, &iov, 1, std::ptr::null(), 0);
            return;
        }
        _ => {
            //nothing to map
        }
    }

    let fd = vuinput_state.file.as_raw_fd();

    // now we can assume that the data is mapped or it is not required
    match cmd_normalized {
        UI_DEV_CREATE => {
            debug!("fh {}: ioctl UI_DEV_CREATE", fh);
            ui_dev_create(fd).unwrap();

            let mut resultbuf: [i8; 64] = [0i8; 64];
            ui_get_sysname(fd, resultbuf.as_mut_slice()).unwrap();
            let sysname = format!(
                "{}{}",
                SYS_INPUT_DIR,
                CStr::from_ptr(resultbuf.as_ptr()).to_string_lossy()
            );
            debug!("fh {}: syspath: {}", fh, sysname);
            let devnode = fetch_device_node(&sysname).unwrap();
            debug!("fh {}: devnode: {}", fh, devnode);
            let (major,minor) = fetch_major_minor(&devnode).unwrap();
            debug!("fh {}: major: {} minor: {} ", fh, major,minor);
            vuinput_state.input_device = Some(VuInputDevice {cuse_fh:*fh, major: major, minor: minor, syspath: sysname.clone(), devnode: devnode.clone(), runtime_data: None, netlink_data: None });

            // Create device in container, if the request was really from another namespace
            if ! SELF_NAMESPACES.get().unwrap().equal_mnt_and_net(&vuinput_state.ns_of_requestor) {
                let inject_job=InjectInContainerJob::new(vuinput_state.ns_of_requestor.clone(),devnode.clone(),sysname.clone(),major,minor);
                JOB_DISPATCHER.get().unwrap().lock().unwrap().dispatch(Box::new(inject_job));
            }

            // hard code 2 second sleep
            std::thread::sleep(std::time::Duration::from_secs(2));

            fuse_lowlevel::fuse_reply_ioctl(_req, 0, std::ptr::null(), 0);
        }
        UI_DEV_DESTROY => {
            debug!("fh {}: ioctl UI_DEV_DESTROY", fh);

            // Remove device in container, if the request was really from another namespace
            if ! SELF_NAMESPACES.get().unwrap().equal_mnt_and_net(&vuinput_state.ns_of_requestor) {
                let input_device = vuinput_state.input_device.as_ref().unwrap();
                let remove_job=RemoveFromContainerJob::new(vuinput_state.ns_of_requestor.clone(),input_device.devnode.clone(),input_device.syspath.clone(),input_device.major,input_device.minor);
                JOB_DISPATCHER.get().unwrap().lock().unwrap().dispatch(Box::new(remove_job));
            }


            ui_dev_destroy(fd).unwrap();
            fuse_lowlevel::fuse_reply_ioctl(_req, 0, std::ptr::null(), 0);
        }
        UI_DEV_SETUP => {
            debug!("fh {}: ioctl UI_DEV_SETUP", fh);
            assert!(_in_bufsz != 0, "should have _in_bufsz");
            let setup_ptr = _in_buf as *mut uinput_setup;
            debug!(
                "product: {:x} vendor: {:x}",
                (*setup_ptr).id.product,
                (*setup_ptr).id.vendor
            );
            // replace vendor and product id to the values from sunshine (see inputtino_common.h of sunshine)
            (*setup_ptr).id.product = 0xdead;
            (*setup_ptr).id.vendor = 0xbeef;
            ui_dev_setup(fd, setup_ptr).unwrap();
            fuse_lowlevel::fuse_reply_ioctl(_req, 0, std::ptr::null(), 0);
        }
        UI_ABS_SETUP => {
            //todo: i guess this needs to be reworked as this is variable size. i guess it is not reachable at all
            debug!("fh {}: ioctl UI_ABS_SETUP", fh);
            assert!(_in_bufsz != 0, "should have _in_bufsz");
            fuse_lowlevel::fuse_reply_ioctl(_req, 0, std::ptr::null(), 0);
        }
        UI_GET_SYSNAME_WITHOUT_SIZE => {
            debug!("fh {}: ioctl UI_GET_SYSNAME {_out_bufsz}", fh);
            assert!(
                _out_bufsz == 64,
                "should have _out_bufsz of length 64 (currently hardcoded)"
            );
            let mut resultbuf: [i8; 64] = [0i8; 64];
            ui_get_sysname(fd, resultbuf.as_mut_slice()).unwrap();
            let sysname = CStr::from_ptr(resultbuf.as_ptr()).to_string_lossy();
            debug!("fh {}: sysname: {}", fh, sysname);
            fuse_lowlevel::fuse_reply_ioctl(
                _req,
                0,
                resultbuf.as_mut_ptr() as *mut c_void,
                _out_bufsz,
            );
        }
        UI_GET_VERSION => {
            let mut version_of_kernel = 0;
            let pversion_of_kernel = std::ptr::from_mut(&mut version_of_kernel);
            ui_get_version(fd, pversion_of_kernel).unwrap();
            debug!("fh {}: ioctl UI_GET_VERSION {}", fh, version_of_kernel);
            let reply_arg = 5;
            let preply_arg = std::ptr::from_ref(&reply_arg);
            fuse_lowlevel::fuse_reply_ioctl(
                _req,
                0,
                preply_arg as *const c_void,
                std::mem::size_of::<c_uint>(),
            );
        }
        UI_SET_EVBIT => {
            let value = _arg as c_uint;
            debug!("fh {}: ioctl UI_SET_EVBIT {}", fh, value);
            ui_set_evbit(fd, value.into()).unwrap();
            fuse_lowlevel::fuse_reply_ioctl(_req, 0, std::ptr::null(), 0);
        }
        UI_SET_KEYBIT => {
            let value = _arg as c_uint;
            debug!("fh {}: ioctl UI_SET_KEYBIT {}", fh, value);
            ui_set_keybit(fd, value.into()).unwrap();
            fuse_lowlevel::fuse_reply_ioctl(_req, 0, std::ptr::null(), 0);
        }
        UI_SET_RELBIT => {
            let value = _arg as c_uint;
            debug!("fh {}: ioctl UI_SET_RELBIT {}", fh, value);
            ui_set_relbit(fd, value.into()).unwrap();
            fuse_lowlevel::fuse_reply_ioctl(_req, 0, std::ptr::null(), 0);
        }
        UI_SET_ABSBIT => {
            let value = _arg as c_uint;
            debug!("fh {}: ioctl UI_SET_ABSBIT {}", fh, value);
            ui_set_absbit(fd, value.into()).unwrap();
            fuse_lowlevel::fuse_reply_ioctl(_req, 0, std::ptr::null(), 0);
        }
        UI_SET_MSCBIT => {
            let value = _arg as c_uint;
            debug!("fh {}: ioctl UI_SET_MSCBIT {}", fh, value);
            ui_set_mscbit(fd, value.into()).unwrap();
            fuse_lowlevel::fuse_reply_ioctl(_req, 0, std::ptr::null(), 0);
        }
        UI_SET_LEDBIT => {
            let value = _arg as c_uint;
            debug!("fh {}: ioctl UI_SET_LEDBIT {}", fh, value);
            ui_set_ledbit(fd, value.into()).unwrap();
            fuse_lowlevel::fuse_reply_ioctl(_req, 0, std::ptr::null(), 0);
        }
        UI_SET_SNDBIT => {
            let value = _arg as c_uint;
            debug!("fh {}: ioctl UI_SET_SNDBIT {}", fh, value);
            ui_set_sndbit(fd, value.into()).unwrap();
            fuse_lowlevel::fuse_reply_ioctl(_req, 0, std::ptr::null(), 0);
        }
        UI_SET_FFBIT => {
            let value = _arg as c_uint;
            debug!("fh {}: ioctl UI_SET_FFBIT {}", fh, value);
            ui_set_ffbit(fd, value.into()).unwrap();
            fuse_lowlevel::fuse_reply_ioctl(_req, 0, std::ptr::null(), 0);
        }
        UI_SET_PHYS => {
            assert!(_in_bufsz != 0, "should have _in_bufsz");
            debug!("fh {}: ioctl UI_SET_PHYS", fh);
            // inbuf is actually a *const c_char, but
            // but the macro to generate ui_set_phys expects a ptr to the actual data structure.
            let phys = _in_buf as *const *const c_char;
            ui_set_phys(fd, phys).unwrap();
            fuse_lowlevel::fuse_reply_ioctl(_req, 0, std::ptr::null(), 0);
        }
        UI_SET_SWBIT => {
            let value = _arg as c_uint;
            debug!("fh {}: ioctl UI_SET_SWBIT {}", fh, value);
            ui_set_swbit(fd, value.into()).unwrap();
            fuse_lowlevel::fuse_reply_ioctl(_req, 0, std::ptr::null(), 0);
        }
        UI_SET_PROPBIT => {
            let value = _arg as c_uint;
            debug!("fh {}: ioctl UI_SET_PROPBIT {}", fh, value);
            ui_set_propbit(fd, value.into()).unwrap();
            fuse_lowlevel::fuse_reply_ioctl(_req, 0, std::ptr::null(), 0);
        }
        UI_BEGIN_FF_UPLOAD => {
            assert!(_in_bufsz != 0, "should have _in_bufsz");
            debug!("fh {}: ioctl UI_BEGIN_FF_UPLOAD", fh);
            let ff_upload_ptr = _in_buf as *mut uinput_ff_upload;
            debug!("request_id: {:x}", (*ff_upload_ptr).request_id);
            ui_begin_ff_upload(fd, ff_upload_ptr).unwrap();
            fuse_lowlevel::fuse_reply_ioctl(_req, 0, ff_upload_ptr as *mut c_void, _out_bufsz);
        }
        UI_END_FF_UPLOAD => {
            assert!(_in_bufsz != 0, "should have _in_bufsz");
            debug!("fh {}: ioctl UI_END_FF_UPLOAD", fh);
            let ff_upload_ptr = _in_buf as *const uinput_ff_upload;
            debug!("request_id: {:x}", (*ff_upload_ptr).request_id);
            ui_end_ff_upload(fd, ff_upload_ptr).unwrap();
            fuse_lowlevel::fuse_reply_ioctl(_req, 0, std::ptr::null(), 0);
        }
        UI_BEGIN_FF_ERASE => {
            assert!(_in_bufsz != 0, "should have _in_bufsz");
            debug!("fh {}: ioctl UI_BEGIN_FF_ERASE", fh);
            let ff_erase_ptr = _in_buf as *mut uinput_ff_erase;
            debug!("request_id: {:x}", (*ff_erase_ptr).request_id);
            ui_begin_ff_erase(fd, ff_erase_ptr).unwrap();
            fuse_lowlevel::fuse_reply_ioctl(_req, 0, ff_erase_ptr as *mut c_void, _out_bufsz);
        }
        UI_END_FF_ERASE => {
            assert!(_in_bufsz != 0, "should have _in_bufsz");
            debug!("fh {}: ioctl UI_END_FF_ERASE", fh);
            let ff_erase_ptr = _in_buf as *const uinput_ff_erase;
            debug!("request_id: {:x}", (*ff_erase_ptr).request_id);
            ui_end_ff_erase(fd, ff_erase_ptr).unwrap();
            fuse_lowlevel::fuse_reply_ioctl(_req, 0, std::ptr::null(), 0);
        }
        _ => {
            debug!("fh {}: ioctl cmd {}", fh, _cmd);
            fuse_lowlevel::fuse_reply_err(_req, EBADRQC);
        }
    }
}

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
        open: Some(vuinput_open),
        read: None,
        write: Some(vuinput_write),
        flush: None,
        release: Some(vuinput_release),
        fsync: None,
        ioctl: Some(vuinput_ioctl),
        poll: None,
    }
}



fn main() -> std::io::Result<()> {

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    let args: Vec<String> = std::env::args().collect();

    VUINPUT_STATE.set(RwLock::new(HashMap::new())).unwrap();
    VUINPUT_COUNTER.set(AtomicU64::new(3)).unwrap();
    JOB_DISPATCHER.set(Mutex::new(Dispatcher::new())).unwrap();
    SELF_NAMESPACES.set(get_namespaces(Pid::SelfPid)).unwrap();
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
        let _reclaim_arg_foreground = CString::from_raw(parg_singlethreaded);
    }
    info!("Stopping vuinputd");
    JOB_DISPATCHER.get().unwrap().lock().unwrap().close();
    JOB_DISPATCHER.get().unwrap().lock().unwrap().wait_until_finished();

    Ok(())
}
