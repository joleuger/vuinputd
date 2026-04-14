// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use crate::{
    container_runtime::injection_strategy::{
        GenericPlacementInContainer, GenericPlacementOnHost, GenericSendNetlinkMessageOnly, INCUS, InjectionStrategy, PLACEMENT_IN_CONTAINER, PLACEMENT_ON_HOST, SEND_NETLINK_ONLY
    },
    global_config::get_vudevname,
};

pub mod injection_strategy;

/// Container runtime used for name resolution and lifecycle events
#[derive(Debug, Clone, clap::ValueEnum, Default, PartialEq, Eq)]
pub enum ContainerRuntime {
    #[default]
    /// Probe for installed runtimes (default). Currently just falls back to "generic placement in container"
    Auto,
    /// Generic linux namespaces. This technique uses nsenter and tries to create files and devices directly in the filesystem inside the container.
    GenericPlacementInContainer,
    /// Generic linux namespaces. This technique creates files and devices directly in the filesystem of the host. It is the job of the user to bind mount those devices to make them available in the container.
    GenericPlacementOnHost,
    /// Generic linux namespaces. This technique just sends the netlink message. Works if the user bind mounds the whole /dev/input and /var/run/udev-folder
    GenericSendNetlinkMessageOnly,
    /// Incus (incus info / incus list). Not implemented, yet.
    Incus,
    /// Docker (docker inspect / Docker socket). This currently falls back to GenericPlacementInContainer.
    Docker,
    /// Podman (podman inspect / Podman socket).  This currently falls back to GenericPlacementOnHost
    Podman,
    /// systemd-nspawn via machinectl. This currently falls back to GenericPlacementInContainer.
    Nspawn,
    /// bubblewrap. This currently falls back to GenericPlacementOnHost
    Bubblewrap,
    /// Custom engine, please define a --strategie-file
    CustomEngine,
}

impl ContainerRuntime {
    fn uses_run_folder(&self) -> bool {
        match self {
            ContainerRuntime::Auto => false,
            ContainerRuntime::GenericPlacementInContainer => false,
            ContainerRuntime::GenericPlacementOnHost => true,
            ContainerRuntime::GenericSendNetlinkMessageOnly => false,
            ContainerRuntime::Incus => false,
            ContainerRuntime::Docker => false,
            ContainerRuntime::Podman => false,
            ContainerRuntime::Nspawn => false,
            ContainerRuntime::Bubblewrap => true,
            ContainerRuntime::CustomEngine => false,
        }
    }

    pub fn initialize(&self) {
        if self.uses_run_folder() {
            let path_prefix = format!("/run/vuinputd/{}", get_vudevname());
            let _ = crate::input_realizer::host_fs::ensure_host_fs_structure(&path_prefix);
        }
    }

    pub fn injection_strategy(&self) -> &'static dyn InjectionStrategy {
        match self {
            ContainerRuntime::Auto => &PLACEMENT_IN_CONTAINER,
            ContainerRuntime::GenericPlacementInContainer => &PLACEMENT_IN_CONTAINER,
            ContainerRuntime::GenericPlacementOnHost => &PLACEMENT_ON_HOST,
            ContainerRuntime::GenericSendNetlinkMessageOnly => &SEND_NETLINK_ONLY,
            ContainerRuntime::Incus => &INCUS,
            ContainerRuntime::Docker => &PLACEMENT_IN_CONTAINER,
            ContainerRuntime::Podman => &PLACEMENT_IN_CONTAINER,
            ContainerRuntime::Nspawn => &PLACEMENT_IN_CONTAINER,
            ContainerRuntime::Bubblewrap => &PLACEMENT_ON_HOST,
            ContainerRuntime::CustomEngine => todo!("not implemented yet"),
        }
    }
}
