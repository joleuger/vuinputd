// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use ::cuse_lowlevel::*;
use libc::ENOENT;
use libc::O_CLOEXEC;
use libc::O_NONBLOCK;
use log::{debug, error};
use std::fs::OpenOptions;
use std::os::fd::AsFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

use crate::cuse_device::evdev_write_watcher::EVDEV_WRITE_WATCHER;
use crate::cuse_device::*;
use crate::process_tools::{get_requesting_process, Pid};

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
    let pid = Pid::Pid(
        (*ctx)
            .pid
            .try_into()
            .expect("pid must be a positive integer"),
    );
    let requesting_process = get_requesting_process(pid);
    debug!("fh {}: namespaces {}", fh, requesting_process);
    // namespaces net:4026531840, uts:4026531838, ipc:4026531839, pid:4026531836, pid_for_children:4026531836, user:4026531837, mnt:4026531841, cgroup:4026531835, time:4026531834, time_for_children:4026531834
    (*_fi).fh = fh;
    // Open the path in read-only mode, returns `io::Result<File>`
    let open_vuinput_result = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(O_NONBLOCK)
        .custom_flags(O_CLOEXEC)
        .open(Path::new("/dev/uinput"));
    match open_vuinput_result {
        Ok(v) => {
            let vu_fh: VuFileHandle = VuFileHandle::Fh(fh);
            let uinput_fd: std::os::unix::prelude::BorrowedFd<'_> = v.as_fd();
            insert_vuinput_state(
                &vu_fh,
                VuInputState {
                    file: v,
                    requesting_process,
                    input_device: None,
                    keytracker: KeyTracker::new(),
                    poll: PollState::new(),
                },
            )
            .unwrap();
            EVDEV_WRITE_WATCHER
                .get()
                .unwrap()
                .lock()
                .unwrap()
                .add_device(vu_fh)
                .unwrap();
            fuse_lowlevel::fuse_reply_open(_req, _fi);
        }
        Err(e) => {
            error!("couldn't open /dev/uinput: {}", e);
            fuse_lowlevel::fuse_reply_err(_req, ENOENT);
        }
    }
}
