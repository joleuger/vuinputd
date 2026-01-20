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
    actions::action::Action,
    global_config::{self, get_placement, Placement},
    input_realizer::runtime_data,
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
pub struct EmitUdevEventJob {
    requesting_process: RequestingProcess,
    target: JobTarget,
    dev_path: String,
    sys_path: String,
    major: u64,
    minor: u64,
    sync_state: Arc<(Mutex<State>, Condvar)>,
}

impl EmitUdevEventJob {
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

impl Job for EmitUdevEventJob {
    fn desc(&self) -> &str {
        "emit udev event into container"
    }

    fn execute_after_cancellation(&self) -> bool {
        false
    }

    fn create_task(self: &EmitUdevEventJob) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(self.clone().emit_udev_event())
    }

    fn job_target(&self) -> JobTarget {
        self.target.clone()
    }
}

impl EmitUdevEventJob {
    async fn emit_udev_event(self) {
        // temporary hack that needs to be replaced. We try 50 times
        // Should be: Wait for the device to be created, the runtime data to be written and the
        // netlink message to be sent
        self.set_state(&State::Started);
        let mut netlink_data: Option<HashMap<String, String>> = None;
        let mut runtime_data: Option<String> = None;
        let mut number_of_attempt = 1;
        while number_of_attempt <= 50 && !(netlink_data.is_some() && runtime_data.is_some()) {
            if netlink_data.is_none() {
                if let Some(netlink_event) = EVENT_STORE
                    .get()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .take(&self.sys_path)
                {
                    if netlink_event.tombstone || netlink_event.remove_data.is_some() {
                        debug!("do nothing, because the device has already been removed in the meantime");
                        return;
                    }
                    netlink_data = netlink_event.add_data;
                };
            }
            if runtime_data.is_none() {
                runtime_data = runtime_data::read_udev_data(self.major, self.minor).ok();
            }

            number_of_attempt += 1;
            // wait a maximum of 5 seconds == 50 attempts
            Timer::after(Duration::from_millis(100)).await;
        }
        if netlink_data.is_none() || runtime_data.is_none() {
            if netlink_data.is_none() {
                debug!("Give up reading netlink data");
            }
            if runtime_data.is_none() {
                debug!("Give up reading runtime data");
            }
            self.set_state(&State::Finished);
            return;
        }

        let runtime_data = runtime_data.unwrap();
        let netlink_data = netlink_data.unwrap();

        match get_placement() {
            Placement::InContainer => {
                let write_udev_runtime_data = Action::WriteUdevRuntimeData {
                    runtime_data: Some(runtime_data),
                    major: self.major,
                    minor: self.minor,
                };

                let child_pid =
                    process_tools::start_action(write_udev_runtime_data, &self.requesting_process)
                        .expect("subprocess should work");

                let _exit_info = await_process(Pid::Pid(child_pid)).await.unwrap();
            }
            Placement::OnHost => {
                let path_prefix = format!("/run/vuinputd/{}", global_config::get_vudevname());
                runtime_data::write_udev_data(
                    &path_prefix,
                    &runtime_data,
                    self.major.into(),
                    self.minor.into(),
                )
                .expect(&format!(
                    "VUI-UDEV-002: could not write into {}",
                    &path_prefix
                )); //TODO: somewhat costly
            }
            Placement::None => {}
        }

        // this is always in the container
        let emit_netlink_message = Action::EmitNetlinkMessage {
            netlink_message: netlink_data.clone(),
        };

        let child_pid = process_tools::start_action(emit_netlink_message, &self.requesting_process)
            .expect("subprocess should work");

        let _exit_info = await_process(Pid::Pid(child_pid)).await.unwrap();

        self.set_state(&State::Finished);
    }
}
