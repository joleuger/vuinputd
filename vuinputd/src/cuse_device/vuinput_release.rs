// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use crate::job_engine::JOB_DISPATCHER;
use crate::jobs::remove_from_container_job::RemoveFromContainerJob;
use crate::process_tools::SELF_NAMESPACES;
use crate::{cuse_device::*, jobs};
use ::cuse_lowlevel::*;
use log::debug;
use std::sync::Arc;

pub unsafe extern "C" fn vuinput_release(
    _req: fuse_lowlevel::fuse_req_t,
    _fi: *mut fuse_lowlevel::fuse_file_info,
) {
    let fh = &(*_fi).fh;
    let vuinput_state_mutex =
        remove_vuinput_state(&VuFileHandle::from_fuse_file_info(_fi.as_ref().unwrap())).unwrap();

    let mut vuinput_state = vuinput_state_mutex.lock().unwrap();
    let input_device = vuinput_state.input_device.take();

    // Remove device in container, if the request was really from another namespace
    // Only do this in case it has not already been done by the ioctl UI_DEV_DESTROY
    // this here is relevant if the process was killed and didn't have the chance to send the
    // ioctl UI_DEV_DESTROY.
    if input_device.is_some()
        && !SELF_NAMESPACES
            .get()
            .unwrap()
            .equal_mnt_and_net(&vuinput_state.requesting_process.namespaces)
    {
        let input_device = input_device.unwrap();
        let remove_job = RemoveFromContainerJob::new(
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
        awaiter(&jobs::remove_from_container_job::State::Finished);
    }

    drop(vuinput_state);

    debug!(
        "fh {}: references left before releasing device {} (expected is 1)",
        fh,
        Arc::strong_count(&vuinput_state_mutex)
    );
    drop(vuinput_state_mutex); // this also closes the file when no other references are open
                               // TODO: maybe also ensure that nothing is left in the containers

    // Note: For CUSE, the kernel always issues RELEASE via fuse_sync_release(),
    // which forces a *synchronous* request (fuse_simple_request()).
    //
    // That means the kernel thread blocks until userspace sends a reply header.
    // Calling fuse_reply_none() would send no header at all, causing the kernel
    // to wait forever and the caller to deadlock.
    //
    // Therefore we must always send a real reply for RELEASE.
    // `fuse_reply_err(req, 0)` is enough to wake the kernel and is safe here.
    fuse_lowlevel::fuse_reply_err(_req, 0);
}
