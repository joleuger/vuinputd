// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::collections::HashMap;
use std::fs::File;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use ::cuse_lowlevel::*;

use crate::process_tools::RequestingProcess;

#[derive(Debug)]
pub struct VuInputDevice {
    pub major: u64,
    pub minor: u64,
    pub syspath: String,
    pub devnode: String,
}

#[derive(Debug)]
pub struct KeyTracker {
    pub left_alt_down: bool,
    pub right_alt_down: bool,
    pub left_ctrl_down: bool,
    pub right_ctrl_down: bool,
}

impl KeyTracker {
    pub fn new() -> Self {
        Self {
            left_alt_down: false,
            right_alt_down: false,
            left_ctrl_down: false,
            right_ctrl_down: false,
        }
    }
}

#[derive(Debug)]
pub struct VuInputState {
    pub file: File,
    pub requesting_process: RequestingProcess,
    pub input_device: Option<VuInputDevice>,
    pub keytracker: KeyTracker,
}

#[derive(Debug, Eq, Hash, PartialEq, Clone)]
pub enum VuFileHandle {
    Fh(u64),
}

impl VuFileHandle {
    pub fn from_fuse_file_info(fi: &fuse_lowlevel::fuse_file_info) -> VuFileHandle {
        VuFileHandle::Fh(fi.fh)
    }
}

impl std::fmt::Display for VuFileHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VuFileHandle::Fh(fh) => writeln!(f, "VuFileHandle({:?})", fh)?,
        }
        Ok(())
    }
}

pub fn get_vuinput_state(fh: &VuFileHandle) -> Result<Arc<Mutex<VuInputState>>, String> {
    let map = VUINPUT_STATE
        .get()
        .ok_or("global not initialized".to_string())?;
    let guard = map.read().map_err(|e| e.to_string())?;
    guard
        .get(&fh)
        .cloned()
        .ok_or("handle not opened".to_string())
}

pub fn insert_vuinput_state(fh: &VuFileHandle, state: VuInputState) -> Result<(), String> {
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

pub fn remove_vuinput_state(fh: &VuFileHandle) -> Result<Arc<Mutex<VuInputState>>, String> {
    let map = VUINPUT_STATE
        .get()
        .ok_or("global not initialized".to_string())?;
    let mut guard = map.write().map_err(|e| e.to_string())?;
    let old_value = guard.remove(&fh).ok_or("fh unknown")?;
    Ok(old_value)
}

pub fn initialize_vuinput_state() {
    VUINPUT_STATE
        .set(RwLock::new(HashMap::new()))
        .expect("failed to initialize global state");
}

pub fn initialize_dedup_last_error() {
    DEDUP_LAST_ERROR
        .set(Mutex::new(None))
        .expect("failed to initialize the log deduplication state");
}

#[derive(Debug)]
pub enum VuError {
    WriteError,
}

pub static VUINPUT_STATE: OnceLock<RwLock<HashMap<VuFileHandle, Arc<Mutex<VuInputState>>>>> =
    OnceLock::new();

// For log limiting. Idea: Move to log_limit crate
pub static DEDUP_LAST_ERROR: OnceLock<Mutex<Option<(u64, VuError)>>> = OnceLock::new();
