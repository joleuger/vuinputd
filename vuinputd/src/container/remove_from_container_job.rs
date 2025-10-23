use std::{collections::HashMap, future::Future, pin::Pin, time::Duration};

use async_io::Timer;
use async_pidfd::AsyncPidFd;
use log::debug;

use crate::{
    container::{mknod_input_device::ensure_input_device, netlink_message::send_udev_monitor_message_with_properties, runtime_data::{self, ensure_udev_structure, read_udev_data, write_udev_data}}, jobs::job::{Job, JobTarget}, monitor_udev::EVENT_STORE, namespace::{run_in_net_and_mnt_namespace, Namespaces}
};

#[derive(Clone,Debug)]
pub struct RemoveFromContainerJob {
    namespaces: Namespaces,
    target: JobTarget,
    sys_path: String,
}

impl RemoveFromContainerJob {
    pub fn new(namespaces: Namespaces,sys_path: String) -> Self {
        Self {
            namespaces: namespaces.clone(),
            target: JobTarget::Container(namespaces),
            sys_path: sys_path,
        }
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
        // TODO: Here is a race with inject in container.
        let netlink_event = match EVENT_STORE.get().unwrap().lock().unwrap().take(&self.sys_path) {
            Some(netlink_event) => netlink_event,
            None => {
                debug!("do nothing, because the device has never been announced via netlink");
                return;
            }
        };

        if netlink_event.tombstone {
            debug!("do nothing, because the device has already been removed in the meantime");
            return;
        }
        let netlink_data=netlink_event.add_data;

        // define for capturing
        let mut netlink_data = netlink_data.unwrap().clone();

        let _ = netlink_data.insert("ACTION".to_string(),"remove".to_string());
        let child_pid = run_in_net_and_mnt_namespace(self.namespaces, Box::new(move || {
            
            send_udev_monitor_message_with_properties(netlink_data.clone());

        }))
        .expect("subprocess should work");
        let pid_fd = AsyncPidFd::from_pid(child_pid.as_raw()).unwrap();
        let _exit_info = pid_fd.wait().await.unwrap();

    }
}
