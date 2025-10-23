use std::{collections::HashMap, future::Future, pin::Pin, time::Duration};

use async_io::Timer;
use async_pidfd::AsyncPidFd;
use log::debug;

use crate::{
    container::{mknod_input_device::ensure_input_device, netlink_message::send_udev_monitor_message_with_properties, runtime_data::{self, ensure_udev_structure, read_udev_data, write_udev_data}}, jobs::job::{Job, JobTarget}, monitor_udev::EVENT_STORE, namespace::{run_in_net_and_mnt_namespace, Namespaces}
};

#[derive(Clone,Debug)]
pub struct InjectInContainerJob {
    namespaces: Namespaces,
    target: JobTarget,
    dev_path: String,
    sys_path: String,
    major: u64,
    minor: u64,
}

impl InjectInContainerJob {
    pub fn new(namespaces: Namespaces,dev_path: String, sys_path: String, major: u64, minor: u64) -> Self {
        Self {
            namespaces: namespaces.clone(),
            target: JobTarget::Container(namespaces),
            dev_path: dev_path,
            sys_path: sys_path,
            major: major ,
            minor: minor,
        }
    }
}

impl Job for InjectInContainerJob {
    fn desc(&self) -> &str {
        "Inject input device into container"
    }

    fn execute_after_cancellation(&self) -> bool {
        false
    }

    fn create_task(self: &InjectInContainerJob) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(self.clone().inject_in_container())
    }

    fn job_target(&self) -> JobTarget {
        self.target.clone()
    }
}

impl InjectInContainerJob {
    async fn inject_in_container(self) {
        // temporary hack that needs to be replaced. We try 50 times
        // Should be: Wait for the device to be created, the runtime data to be written and the
        // netlink message to be sent
        let mut netlink_data: Option<HashMap<String,String>> = None;
        let mut runtime_data: Option<String> = None;
        let mut number_of_attempt = 1;
        while number_of_attempt<=50 && !(netlink_data.is_some() && runtime_data.is_some()) {

            if netlink_data.is_none() {

                if let Some(netlink_event)=EVENT_STORE.get().unwrap().lock().unwrap().take(&self.sys_path) {
                    if netlink_event.tombstone || netlink_event.remove_data.is_some() {
                        debug!("do nothing, because the device has already been removed in the meantime");
                        return;
                    }
                    netlink_data=netlink_event.add_data;
                };
            }
            if runtime_data.is_none() {
                runtime_data = read_udev_data(self.major,self.minor).ok();

            }

            number_of_attempt+=1;
            // wait a maximum of 5 seconds == 50 attempts
            Timer::after(Duration::from_millis(100)).await;
        }
        if (netlink_data.is_none() || runtime_data.is_none()) {
            if netlink_data.is_none() {
                debug!("Give up reading netlink data");
            }
            if runtime_data.is_none() {
                debug!("Give up reading runtime data");
            }
            return;
        }

        // define for capturing
        let major = self.major;
        let minor=self.minor;
        let runtime_data = runtime_data.unwrap();
        let netlink_data = netlink_data.unwrap();

        let child_pid = run_in_net_and_mnt_namespace(self.namespaces, Box::new(move || {
            
            ensure_input_device(self.dev_path.clone(), self.major, self.minor).unwrap();
            ensure_udev_structure().unwrap();
            write_udev_data(runtime_data.as_str(), major, minor).unwrap();
            send_udev_monitor_message_with_properties(netlink_data.clone());

        }))
        .expect("subprocess should work");
        let pid_fd = AsyncPidFd::from_pid(child_pid.as_raw()).unwrap();
        let _exit_info = pid_fd.wait().await.unwrap();

    }
}
