// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

pub mod vuinput_ioctl;
pub mod vuinput_write;
pub mod vuinput_release;
pub mod vuinput_open;

use std::collections::HashMap;
use std::fs::{self, File};
use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::io::{ErrorKind};

use ::cuse_lowlevel::*;

use crate::process_tools::RequestingProcess;

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
pub struct VuInputState {
    file: File,
    requesting_process: RequestingProcess,
    input_device: Option<VuInputDevice>
}

#[derive(Debug,Eq, Hash, PartialEq, Clone)]
pub enum VuFileHandle {
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

#[derive(Debug)]
pub enum VuError {
    WriteError
}

pub static VUINPUT_STATE: OnceLock<RwLock<HashMap<VuFileHandle, Arc<Mutex<VuInputState>>>>> = OnceLock::new();

// For log limiting. Idea: Move to log_limit crate
pub static DEDUP_LAST_ERROR: OnceLock<Mutex<Option<(u64,VuError)>>> = OnceLock::new(); 


pub const SYS_INPUT_DIR: &str = "/sys/devices/virtual/input/";
pub const BUS_USB: u16 = 0x03;

pub fn get_vuinput_state(
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


pub fn insert_vuinput_state(
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

pub fn remove_vuinput_state(
    fh:&VuFileHandle,
) -> Result<Arc<Mutex<VuInputState>>, String> {
    let map = VUINPUT_STATE
        .get()
        .ok_or("global not initialized".to_string())?;
    let mut guard = map.write().map_err(|e| e.to_string())?;
    let old_value = guard.remove(&fh).ok_or("fh unknown")?;
    Ok(old_value)
}

pub fn fetch_device_node(path: &str) -> io::Result<String> {
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
