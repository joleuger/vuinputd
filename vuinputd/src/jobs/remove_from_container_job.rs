// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::{collections::HashMap, future::Future, pin::Pin, sync::{Arc, Condvar, Mutex}, time::Duration};

use async_io::Timer;
use log::debug;

use crate::{job_engine::job::{Job, JobTarget}, jobs::{mknod_input_device::remove_input_device, monitor_udev_job::EVENT_STORE, netlink_message::send_udev_monitor_message_with_properties, runtime_data::{delete_udev_data, ensure_udev_structure, read_udev_data, write_udev_data}}, requesting_process::{Pid, RequestingProcess, await_process, run_in_net_and_mnt_namespace}};



#[derive(Clone,Debug,Copy,PartialOrd,PartialEq)]
pub enum State {
    Initialized,
    Started,
    Finished,
}


#[derive(Clone,Debug)]
pub struct RemoveFromContainerJob {
    requesting_process: RequestingProcess,
    target: JobTarget,
    dev_path: String,
    sys_path: String,
    major: u64,
    minor: u64,
    sync_state: Arc<(Mutex<State>,Condvar)>,
}

impl RemoveFromContainerJob {
    pub fn new(requesting_process: RequestingProcess,dev_path: String, sys_path: String, major: u64, minor: u64) -> Self {
        Self {
            requesting_process: requesting_process.clone(),
            target: JobTarget::Container(requesting_process),
            dev_path: dev_path,
            sys_path: sys_path,
            major: major ,
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
        let awaiter =  move | state: &State|  {
            let (lock, cvar) = &*sync_state;
            let mut current_state = lock.lock().unwrap();
            while *current_state < *state {
                current_state = cvar.wait(current_state).unwrap();
            }
        };
        awaiter
    }

}

impl Job for RemoveFromContainerJob {
    fn desc(&self) -> &str {
        "Remove input device from container"
    }

    fn execute_after_cancellation(&self) -> bool {
        false
    }

    fn create_task(self: &RemoveFromContainerJob) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(self.clone().remove_from_container())
    }

    fn job_target(&self) -> JobTarget {
        self.target.clone()
    }
}

impl RemoveFromContainerJob {
    async fn remove_from_container(self) {
        self.set_state(&State::Started);

        let netlink_event = match EVENT_STORE.get().unwrap().lock().unwrap().take(&self.sys_path) {
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
        let netlink_data=netlink_event.add_data;

        // define for capturing
        let mut netlink_data = netlink_data.unwrap().clone();
        let major = self.major;
        let minor=self.minor;
        let dev_path = self.dev_path.clone();

        let _ = netlink_data.insert("ACTION".to_string(),"remove".to_string());
        let child_pid = run_in_net_and_mnt_namespace(&self.requesting_process, Box::new(move || {
            // TODO: we should keep the same order as event_execute_rules_on_remove in 
            // https://github.com/systemd/systemd/blob/main/src/udev/udev-event.c
            
            send_udev_monitor_message_with_properties(netlink_data.clone());
            if let Err(e) = delete_udev_data(major,minor) {
                debug!("Error deleting udev data for {}:{}: {e}",major,minor);
            }
            if let Err(e) = remove_input_device(dev_path.clone(), self.major, self.minor) {
                debug!("Error removing input device {}: {e}",dev_path.clone());
            };

        }))
        .expect("subprocess should work");
        let _exit_info = await_process(Pid::Pid(child_pid.as_raw())).await;
        self.set_state(&State::Finished);

    }
}
