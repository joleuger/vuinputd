// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

use async_io::Timer;
use log::debug;

use crate::{
    actions::{action::Action, runtime_data::read_udev_data},
    job_engine::job::{Job, JobTarget},
    jobs::monitor_udev_job::EVENT_STORE,
    process_tools::{self, await_process, Pid, RequestingProcess},
};

#[derive(Clone, Debug, Copy, PartialOrd, PartialEq)]
pub enum State {
    Initialized,
    Started,
    Finished,
}

#[derive(Clone, Debug)]
pub struct MknodDeviceInContainerJob {
    requesting_process: RequestingProcess,
    target: JobTarget,
    dev_path: String,
    sys_path: String,
    major: u64,
    minor: u64,
    sync_state: Arc<(Mutex<State>, Condvar)>,
}

impl MknodDeviceInContainerJob {
    pub fn new(
        requesting_process: RequestingProcess,
        dev_path: String,
        sys_path: String,
        major: u64,
        minor: u64,
    ) -> Self {
        Self {
            requesting_process: requesting_process.clone(),
            target: JobTarget::Container(requesting_process),
            dev_path: dev_path,
            sys_path: sys_path,
            major: major,
            minor: minor,
            sync_state: Arc::new((Mutex::new(State::Initialized), Condvar::new())),
        }
    }

    fn set_state(&self, new_state: &State) -> () {
        let (lock, cvar) = &*self.sync_state;
        let mut current_state = lock.lock().unwrap();
        *current_state = *new_state;
        // We notify the condvar that the value has changed.
        cvar.notify_all();
    }

    pub fn get_awaiter_for_state(&self) -> impl FnOnce(&State) -> () {
        // pattern is described on https://doc.rust-lang.org/stable/std/sync/struct.Condvar.html
        let sync_state = self.sync_state.clone();
        let awaiter = move |state: &State| {
            let (lock, cvar) = &*sync_state;
            let mut current_state = lock.lock().unwrap();
            while *current_state < *state {
                current_state = cvar.wait(current_state).unwrap();
            }
        };
        awaiter
    }
}

impl Job for MknodDeviceInContainerJob {
    fn desc(&self) -> &str {
        "mknod input device in container"
    }

    fn execute_after_cancellation(&self) -> bool {
        false
    }

    fn create_task(self: &MknodDeviceInContainerJob) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(self.clone().inject_in_container())
    }

    fn job_target(&self) -> JobTarget {
        self.target.clone()
    }
}

impl MknodDeviceInContainerJob {
    async fn inject_in_container(self) {
        let mknod_device_action = Action::MknodDevice {
            path: self.dev_path.clone(),
            major: self.major,
            minor: self.minor,
        };

        let child_pid = process_tools::start_action(mknod_device_action, &self.requesting_process)
            .expect("subprocess should work");

        let _exit_info = await_process(Pid::Pid(child_pid)).await.unwrap();
        self.set_state(&State::Finished);
    }
}
