// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::collections::HashMap;
use std::fs::File;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use ::cuse_lowlevel::*;
use smallvec::SmallVec;

use crate::process_tools::RequestingProcess;

pub type PendingPollHandles = SmallVec<[*mut fuse_lowlevel::fuse_pollhandle; 1]>;

#[derive(Debug)]
pub struct VuInputDevice {
    pub major: u64,
    pub minor: u64,
    pub syspath: String,
    pub devname: String,
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

/// EMPTY -> READY -> READING -> { EMPTY | READY }
/// EMPTY -> READABLE        (new data arrives / watcher can observe)
/// READABLE -> READING      (read callback starts draining)
/// READING -> EMPTY         (read drained everything)
/// READING -> READABLE      (read finished, but more data still remains)
#[derive(Debug)]
pub enum PollPhase {
    Empty,
    Readable,
    Reading,
}

impl Default for PollPhase {
    fn default() -> Self {
        Self::Empty
    }
}

#[derive(Debug)]
pub struct PollHandle {
    ptr: NonNull<fuse_lowlevel::fuse_pollhandle>,
    has_been_completed: bool,
}

impl PollHandle {
    pub fn new(ptr: NonNull<fuse_lowlevel::fuse_pollhandle>) -> Self {
        Self {
            ptr: ptr,
            has_been_completed: false,
        }
    }
    pub fn notify(&mut self) {
        if !self.has_been_completed {
            unsafe {
                fuse_lowlevel::fuse_lowlevel_notify_poll(self.ptr.as_ptr());
                fuse_lowlevel::fuse_pollhandle_destroy(self.ptr.as_ptr());
            }
            self.has_been_completed = true;
        }
    }
}

impl Drop for PollHandle {
    fn drop(&mut self) {
        if !self.has_been_completed {
            unsafe {
                fuse_lowlevel::fuse_pollhandle_destroy(self.ptr.as_ptr());
            }
            self.has_been_completed = true;
        }
    }
}

unsafe impl Send for PollHandle {}

/// this data structure ensures poll and read are synchronized.
/// poll() and read() must synchronize through one shared readines
/// state, and the state transitions must be done under the same per-handle mutex.
/// Ensure, we have no lost-wakeup races like:
/// 1) watcher sets readable
/// 2) read() drains and clears readable
/// 3) poll() stores waiter too late
/// 4) nobody wakes it anymore
#[derive(Debug, Default)]
pub struct PollState {
    /// Sticky readiness latch:
    /// true once evdev became readable, false again after read/drain.
    pub pollphase: PollPhase,

    /// Pending FUSE poll waiters for this device.
    /// Optimized for the common case of 0 or 1 waiter, but supports
    /// multiple concurrent poll() callers correctly.
    pending: Option<PollHandle>,
}

impl PollState {
    pub fn new() -> PollState {
        PollState {
            pollphase: PollPhase::Empty,
            pending: None,
        }
    }
    pub fn has_waiters(&self) -> bool {
        !self.pending.is_some()
    }

    pub fn set_waiter(&mut self, handle: NonNull<fuse_lowlevel::fuse_pollhandle>) {
        self.pending = Some(PollHandle::new(handle));
    }

    pub fn take_waiters(&mut self) -> Option<PollHandle> {
        std::mem::take(&mut self.pending)
    }
}

impl Drop for PollState {
    fn drop(&mut self) {
        //when the device closes, notify all pending waiters
        let old_handle = self.take_waiters();
        if let Some(mut old_handle) = old_handle {
            old_handle.notify();
        }
    }
}

#[derive(Debug)]
pub struct VuInputState {
    pub file: File,
    pub requesting_process: RequestingProcess,
    pub input_device: Option<VuInputDevice>,
    pub keytracker: KeyTracker,
    pub poll: PollState,
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
