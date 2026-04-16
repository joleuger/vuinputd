// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::collections::HashMap;

use anyhow::bail;
use async_trait::async_trait;

use crate::{
    actions::action::Action,
    global_config::{self, get_scope},
    input_realizer::{input_device, runtime_data},
    process_tools::{self, Pid, RequestingProcess},
};
pub static PLACEMENT_IN_CONTAINER: GenericPlacementInContainer = GenericPlacementInContainer {};
pub static PLACEMENT_ON_HOST: GenericPlacementOnHost = GenericPlacementOnHost {};
pub static SEND_NETLINK_ONLY: GenericSendNetlinkMessageOnly = GenericSendNetlinkMessageOnly {};
pub static INCUS: Incus = Incus {};

#[async_trait]
pub trait InjectionStrategy {
    /// Create the device node.
    async fn mknod_device_node(
        &self,
        requesting_process: &RequestingProcess,
        devname: &str,
        major: u64,
        minor: u64,
    ) -> anyhow::Result<()>;

    /// Remove device.
    async fn remove_device_node(
        &self,
        requesting_process: &RequestingProcess,
        devname: &str,
        major: u64,
        minor: u64,
    ) -> anyhow::Result<()>;

    /// Write udev metadata.
    async fn write_udev_runtime_data(
        &self,
        requesting_process: &RequestingProcess,
        runtime_data: &str,
        major: u64,
        minor: u64,
    ) -> anyhow::Result<()>;

    /// Remove runtime data.
    async fn remove_udev_runtime_data(
        &self,
        requesting_process: &RequestingProcess,
        major: u64,
        minor: u64,
    ) -> anyhow::Result<()>;

    /// Emit netlink message.
    async fn emit_netlink_message(
        &self,
        requesting_process: &RequestingProcess,
        netlink_message: HashMap<String, String>,
    ) -> anyhow::Result<()>;
}

pub struct GenericPlacementInContainer {}
pub struct GenericPlacementOnHost {}
pub struct GenericSendNetlinkMessageOnly {}
pub struct Incus {}

#[async_trait]
impl InjectionStrategy for GenericPlacementInContainer {
    async fn mknod_device_node(
        &self,
        requesting_process: &RequestingProcess,
        devname: &str,
        major: u64,
        minor: u64,
    ) -> anyhow::Result<()> {
        let mknod_device_action = Action::MknodDevice {
            path: format!("/dev/input/{}", &devname),
            major: major,
            minor: minor,
        };

        let child_pid = process_tools::start_action(mknod_device_action, &requesting_process)
            .expect("subprocess should work");

        let _exit_info = process_tools::await_process(Pid::Pid(child_pid))
            .await
            .unwrap();
        Ok(())
    }

    async fn remove_device_node(
        &self,
        requesting_process: &RequestingProcess,
        devname: &str,
        major: u64,
        minor: u64,
    ) -> anyhow::Result<()> {
        let dev_path = format!("/dev/input/{}", devname);
        let remove_device_action = Action::RemoveDevice {
            path: dev_path,
            major: major,
            minor: minor,
        };

        let child_pid_1 = process_tools::start_action(remove_device_action, &requesting_process)
            .expect("subprocess should work");

        let _exit_info = process_tools::await_process(Pid::Pid(child_pid_1)).await;
        Ok(())
    }

    async fn write_udev_runtime_data(
        &self,
        requesting_process: &RequestingProcess,
        runtime_data: &str,
        major: u64,
        minor: u64,
    ) -> anyhow::Result<()> {
        let write_udev_runtime_data = Action::WriteUdevRuntimeData {
            runtime_data: Some(runtime_data.to_string()),
            major: major,
            minor: minor,
        };

        let child_pid = process_tools::start_action(write_udev_runtime_data, &requesting_process)
            .expect("subprocess should work");

        let _exit_info = process_tools::await_process(Pid::Pid(child_pid))
            .await
            .unwrap();
        Ok(())
    }

    async fn remove_udev_runtime_data(
        &self,
        requesting_process: &RequestingProcess,
        major: u64,
        minor: u64,
    ) -> anyhow::Result<()> {
        let write_udev_runtime_data_action = Action::WriteUdevRuntimeData {
            runtime_data: None,
            major: major,
            minor: minor,
        };

        let child_pid_2 =
            process_tools::start_action(write_udev_runtime_data_action, &requesting_process)
                .expect("subprocess should work");
        let _exit_info = process_tools::await_process(Pid::Pid(child_pid_2)).await;
        Ok(())
    }

    /// Emit netlink message.
    async fn emit_netlink_message(
        &self,
        requesting_process: &RequestingProcess,
        netlink_message: HashMap<String, String>,
    ) -> anyhow::Result<()> {
        let emit_netlink_message = Action::EmitNetlinkMessage {
            netlink_message: netlink_message,
        };

        let child_pid = process_tools::start_action(emit_netlink_message, requesting_process)
            .expect("subprocess should work");

        let _exit_info = process_tools::await_process(Pid::Pid(child_pid)).await;
        Ok(())
    }
}

#[async_trait]
impl InjectionStrategy for GenericPlacementOnHost {
    async fn mknod_device_node(
        &self,
        _requesting_process: &RequestingProcess,
        devname: &str,
        major: u64,
        minor: u64,
    ) -> anyhow::Result<()> {
        let path_prefix = format!("/run/vuinputd/{}", global_config::get_vudevname());
        let path = format!("{}/dev-input/{}", path_prefix, devname);
        input_device::ensure_input_device(path.clone(), major, minor)
            .expect(&format!("VUI-DEV-001: could not create {}", &path));
        //TODO: somewhat costly
        Ok(())
    }

    async fn remove_device_node(
        &self,
        _requesting_process: &RequestingProcess,
        devname: &str,
        major: u64,
        minor: u64,
    ) -> anyhow::Result<()> {
        let path_prefix = format!("/run/vuinputd/{}", global_config::get_vudevname());
        let devnode = format!("{}/dev-input/{}", path_prefix, devname);
        input_device::remove_input_device(devnode.clone(), major, minor).expect(&format!(
            "VUI-DEV-003: could not remove device node {}",
            &devnode
        ));
        Ok(())
    }

    async fn write_udev_runtime_data(
        &self,
        _requesting_process: &RequestingProcess,
        runtime_data: &str,
        major: u64,
        minor: u64,
    ) -> anyhow::Result<()> {
        let path_prefix = format!("/run/vuinputd/{}", global_config::get_vudevname());
        runtime_data::write_udev_data(&path_prefix, &runtime_data, major.into(), minor.into())
            .expect(&format!(
                "VUI-UDEV-002: could not write into {}",
                &path_prefix
            )); //TODO: somewhat costly
        Ok(())
    }

    async fn remove_udev_runtime_data(
        &self,
        _requesting_process: &RequestingProcess,
        major: u64,
        minor: u64,
    ) -> anyhow::Result<()> {
        let path_prefix = format!("/run/vuinputd/{}", global_config::get_vudevname());
        runtime_data::delete_udev_data(&path_prefix, major, minor).expect(&format!(
            "VUI-UDEV-003: could not remove udev data from {}",
            &path_prefix
        ));
        Ok(())
    }


    /// Emit netlink message.
    async fn emit_netlink_message(
        &self,
        requesting_process: &RequestingProcess,
        netlink_message: HashMap<String, String>,
    ) -> anyhow::Result<()> {
        PLACEMENT_IN_CONTAINER
            .emit_netlink_message(requesting_process, netlink_message)
            .await
    }
}

#[async_trait]
impl InjectionStrategy for GenericSendNetlinkMessageOnly {
    async fn mknod_device_node(
        &self,
        _requesting_process: &RequestingProcess,
        _devname: &str,
        _major: u64,
        _minor: u64,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn remove_device_node(
        &self,
        _requesting_process: &RequestingProcess,
        _devname: &str,
        _major: u64,
        _minor: u64,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn write_udev_runtime_data(
        &self,
        _requesting_process: &RequestingProcess,
        _runtime_data: &str,
        _major: u64,
        _minor: u64,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn remove_udev_runtime_data(
        &self,
        _requesting_process: &RequestingProcess,
        _major: u64,
        _minor: u64,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Emit netlink message.
    async fn emit_netlink_message(
        &self,
        requesting_process: &RequestingProcess,
        netlink_message: HashMap<String, String>,
    ) -> anyhow::Result<()> {
        PLACEMENT_IN_CONTAINER
            .emit_netlink_message(requesting_process, netlink_message)
            .await
    }
}

#[async_trait]
impl InjectionStrategy for Incus {
    async fn mknod_device_node(
        &self,
        _requesting_process: &RequestingProcess,
        devname: &str,
        _major: u64,
        _minor: u64,
    ) -> anyhow::Result<()> {
        let hostpath = format!("path=/dev/input/{}", devname);
        let incuspath = format!("path=/dev/input/{}", devname);
        let container_name = get_scope();
        let container_name = match container_name {
            global_config::Scope::Multi => bail!("no container name given"),
            global_config::Scope::Single(container_name) => container_name,
        };
        let child = std::process::Command::new("/usr/bin/incus")
            .args([
                "config",
                "device",
                "add",
                container_name,
                devname,
                "unix-char",
                &incuspath,
                &hostpath,
                "mode=666",
            ])
            .spawn()?;
        let output = child.wait_with_output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("incus\n {}\n{}\n", stdout, stderr);
        Ok(())
    }

    async fn remove_device_node(
        &self,
        _requesting_process: &RequestingProcess,
        devname: &str,
        _major: u64,
        _minor: u64,
    ) -> anyhow::Result<()> {
        let container_name = get_scope();
        let container_name = match container_name {
            global_config::Scope::Multi => bail!("no container name given"),
            global_config::Scope::Single(container_name) => container_name,
        };
        let child = std::process::Command::new("/usr/bin/incus")
            .args(["config", "device", "remove", container_name, devname])
            .spawn()?;
        let output = child.wait_with_output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("incus\n {}\n{}\n", stdout, stderr);
        Ok(())
    }

    async fn write_udev_runtime_data(
        &self,
        requesting_process: &RequestingProcess,
        runtime_data: &str,
        major: u64,
        minor: u64,
    ) -> anyhow::Result<()> {
        PLACEMENT_IN_CONTAINER
            .write_udev_runtime_data(requesting_process, runtime_data, major, minor)
            .await
    }

    async fn remove_udev_runtime_data(
        &self,
        requesting_process: &RequestingProcess,
        major: u64,
        minor: u64,
    ) -> anyhow::Result<()> {
        PLACEMENT_IN_CONTAINER
            .remove_udev_runtime_data(requesting_process, major, minor)
            .await
    }

    /// Emit netlink message.
    async fn emit_netlink_message(
        &self,
        requesting_process: &RequestingProcess,
        netlink_message: HashMap<String, String>,
    ) -> anyhow::Result<()> {
        PLACEMENT_IN_CONTAINER
            .emit_netlink_message(requesting_process, netlink_message)
            .await
    }
}
