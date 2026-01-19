// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use ::cuse_lowlevel::*;
use libc::{iovec, size_t, EBADRQC};
use libc::{uinput_abs_setup, uinput_ff_erase, uinput_ff_upload, uinput_setup};
use log::debug;
use std::ffi::CStr;
use std::os::fd::AsRawFd;
use std::os::raw::{c_char, c_int, c_uint, c_void};
use uinput_ioctls::*;

use crate::cuse_device::{get_vuinput_state, VuFileHandle};
use crate::job_engine::JOB_DISPATCHER;
use crate::jobs::emit_udev_event_job::EmitUdevEventJob;
use crate::jobs::mknod_device_job::MknodDeviceJob;
use crate::jobs::remove_device_job::RemoveDeviceJob;
use crate::process_tools::SELF_NAMESPACES;
use crate::{cuse_device::*, jobs};

pub const SYS_INPUT_DIR: &str = "/sys/devices/virtual/input/";

pub unsafe extern "C" fn vuinput_ioctl(
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
    let vufh = VuFileHandle::from_fuse_file_info(_fi.as_ref().unwrap());
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

            let mut resultbuf: [c_char; 64] = [0; 64];
            ui_get_sysname(fd, resultbuf.as_mut_slice()).unwrap();
            let sysname = format!(
                "{}{}",
                SYS_INPUT_DIR,
                CStr::from_ptr(resultbuf.as_ptr()).to_string_lossy()
            );
            debug!("fh {}: syspath: {}", fh, sysname);
            let (devname, devnode) = fetch_device_node(&sysname).unwrap();
            debug!("fh {}: devnode: {}", fh, devnode);
            let (major, minor) = fetch_major_minor(&devnode).unwrap();
            debug!("fh {}: major: {} minor: {} ", fh, major, minor);
            vuinput_state.input_device = Some(VuInputDevice {
                major: major,
                minor: minor,
                syspath: sysname.clone(),
                devnode: devnode.clone(),
            });

            // Create device in container, if the request was really from another namespace
            if !SELF_NAMESPACES
                .get()
                .unwrap()
                .equal_mnt_and_net(&vuinput_state.requesting_process.namespaces)
            {
                let mknod_job = MknodDeviceJob::new(
                    vuinput_state.requesting_process.clone(),
                    devname.clone(),
                    sysname.clone(),
                    major,
                    minor,
                );
                let awaiter = mknod_job.get_awaiter_for_state();
                JOB_DISPATCHER
                    .get()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .dispatch(Box::new(mknod_job));
                awaiter(&jobs::mknod_device_job::State::Finished);
                debug!("fh {}: mknod_device in container has been finished ", fh);
                fuse_lowlevel::fuse_reply_ioctl(_req, 0, std::ptr::null(), 0);

                // we do not wait for the udev stuff
                let emit_udev_event_job = EmitUdevEventJob::new(
                    vuinput_state.requesting_process.clone(),
                    devnode.clone(),
                    sysname.clone(),
                    major,
                    minor,
                );
                JOB_DISPATCHER
                    .get()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .dispatch(Box::new(emit_udev_event_job));
            } else {
                fuse_lowlevel::fuse_reply_ioctl(_req, 0, std::ptr::null(), 0);
            }
        }
        UI_DEV_DESTROY => {
            debug!("fh {}: ioctl UI_DEV_DESTROY", fh);
            let input_device = vuinput_state.input_device.take();

            // Remove device in container, if the request was really from another namespace
            if input_device.is_some()
                && !SELF_NAMESPACES
                    .get()
                    .unwrap()
                    .equal_mnt_and_net(&vuinput_state.requesting_process.namespaces)
            {
                let input_device = input_device.unwrap();
                let remove_job = RemoveDeviceJob::new(
                    vuinput_state.requesting_process.clone(),
                    input_device.devnode.clone(),
                    input_device.syspath.clone(),
                    input_device.major,
                    input_device.minor,
                );
                let awaiter = remove_job.get_awaiter_for_state();
                JOB_DISPATCHER
                    .get()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .dispatch(Box::new(remove_job));
                awaiter(&jobs::remove_device_job::State::Finished);
                debug!(
                    "fh {}: removing dev-nodes from container has been finished ",
                    fh
                );
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
            // The pid is registered for vuinputd, see https://pid.codes/1209/5020/
            (*setup_ptr).id.bustype = BUS_USB;
            (*setup_ptr).id.product = 0x5020;
            (*setup_ptr).id.vendor = 0x1209;
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
            let mut resultbuf: [c_char; 64] = [0; 64];
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

pub fn fetch_device_node(path: &str) -> io::Result<(String, String)> {
    for entry in fs::read_dir(path)? {
        let entry = entry?; // propagate per-entry errors
        if let Some(name) = entry.file_name().to_str() {
            if name.starts_with("event") {
                return Ok((name.to_string(), format!("/dev/input/{}", name)));
            }
        }
    }
    // If no device is found, return an error
    Err(io::Error::new(ErrorKind::NotFound, "no device found"))
}

/// Returns (major, minor) numbers of a device node at `path`
pub fn fetch_major_minor(path: &str) -> io::Result<(u64, u64)> {
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
