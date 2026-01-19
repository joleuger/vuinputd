// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Condvar, Mutex},
};

use log::debug;

use crate::{
    actions::action::Action,
    global_config::{self, Placement},
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
pub struct RemoveDeviceJob {
    requesting_process: RequestingProcess,
    target: JobTarget,
    dev_path: String,
    sys_path: String,
    major: u64,
    minor: u64,
    sync_state: Arc<(Mutex<State>, Condvar)>,
}

impl RemoveDeviceJob {
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

impl Job for RemoveDeviceJob {
    fn desc(&self) -> &str {
        "Remove input device from container"
    }

    fn execute_after_cancellation(&self) -> bool {
        false
    }

    fn create_task(self: &RemoveDeviceJob) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(self.clone().remove_device())
    }

    fn job_target(&self) -> JobTarget {
        self.target.clone()
    }
}

impl RemoveDeviceJob {
    async fn remove_device(self) {
        self.set_state(&State::Started);

        let netlink_event = match EVENT_STORE
            .get()
            .unwrap()
            .lock()
            .unwrap()
            .take(&self.sys_path)
        {
            Some(netlink_event) => netlink_event,
            None => {
                debug!("do nothing, because the device has never been announced via netlink");
                self.set_state(&State::Finished);
                return;
            }
        };

        if netlink_event.tombstone {
            debug!("do nothing, because the device has already been removed in the meantime");
            self.set_state(&State::Finished);
            return;
        }
        let netlink_data = netlink_event.add_data;

        let mut netlink_data = netlink_data.unwrap().clone();
        let dev_path = self.dev_path.clone();

        let _ = netlink_data.insert("ACTION".to_string(), "remove".to_string());

        match global_config::get_placement() {
            Placement::InContainer => {
                let remove_device_action = Action::RemoveDevice {
                    path: dev_path.clone(),
                    major: self.major,
                    minor: self.minor,
                };

                let child_pid_1 =
                    process_tools::start_action(remove_device_action, &self.requesting_process)
                        .expect("subprocess should work");

                let write_udev_runtime_data_action = Action::WriteUdevRuntimeData {
                    runtime_data: None,
                    major: self.major,
                    minor: self.minor,
                };

                let child_pid_2 = process_tools::start_action(
                    write_udev_runtime_data_action,
                    &self.requesting_process,
                )
                .expect("subprocess should work");

                let _exit_info = await_process(Pid::Pid(child_pid_1)).await;
                let _exit_info = await_process(Pid::Pid(child_pid_2)).await;
            }
            Placement::OnHost => {
                todo!();
            }
            Placement::None => {}
        }

        // this is always in the container
        let emit_netlink_message = Action::EmitNetlinkMessage {
            netlink_message: netlink_data.clone(),
        };

        let child_pid_netlink =
            process_tools::start_action(emit_netlink_message, &self.requesting_process)
                .expect("subprocess should work");

        let _exit_info = await_process(Pid::Pid(child_pid_netlink)).await;

        self.set_state(&State::Finished);
    }
}
