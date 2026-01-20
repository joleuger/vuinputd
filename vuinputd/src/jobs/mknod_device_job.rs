// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Condvar, Mutex},
};

use crate::{
    actions::action::Action,
    global_config::{self, Placement},
    input_realizer::input_device,
    job_engine::job::{Job, JobTarget},
    process_tools::{self, await_process, Pid, RequestingProcess},
};

use crate::input_realizer::runtime_data::write_udev_data;

#[derive(Clone, Debug, Copy, PartialOrd, PartialEq)]
pub enum State {
    Initialized,
    Started,
    Finished,
}

#[derive(Clone, Debug)]
pub struct MknodDeviceJob {
    requesting_process: RequestingProcess,
    target: JobTarget,
    devname: String,
    sys_path: String,
    major: u64,
    minor: u64,
    sync_state: Arc<(Mutex<State>, Condvar)>,
}

impl MknodDeviceJob {
    pub fn new(
        requesting_process: RequestingProcess,
        devname: String,
        sys_path: String,
        major: u64,
        minor: u64,
    ) -> Self {
        Self {
            requesting_process: requesting_process.clone(),
            target: JobTarget::Container(requesting_process),
            devname: devname,
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

impl Job for MknodDeviceJob {
    fn desc(&self) -> &str {
        "mknod input device in container"
    }

    fn execute_after_cancellation(&self) -> bool {
        false
    }

    fn create_task(self: &MknodDeviceJob) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(self.clone().mknod_device())
    }

    fn job_target(&self) -> JobTarget {
        self.target.clone()
    }
}

impl MknodDeviceJob {
    async fn mknod_device(self) {
        match global_config::get_placement() {
            Placement::InContainer => {
                let mknod_device_action = Action::MknodDevice {
                    path: format!("/dev/input/{}", &self.devname),
                    major: self.major,
                    minor: self.minor,
                };

                let child_pid =
                    process_tools::start_action(mknod_device_action, &self.requesting_process)
                        .expect("subprocess should work");

                let _exit_info = await_process(Pid::Pid(child_pid)).await.unwrap();
            }
            Placement::OnHost => {
                let path = format!(
                    "/run/vuinputd/{}/dev-input/{}",
                    global_config::get_devname(),
                    self.devname
                );
                input_device::ensure_input_device(path.clone(), self.major, self.minor)
                    .expect(&format!("VUI-DEV-001: could not create {}", &path));
                //TODO: somewhat costly
            }
            Placement::None => {}
        }

        self.set_state(&State::Finished);
    }
}
