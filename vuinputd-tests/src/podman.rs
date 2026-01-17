// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use nix::errno::Errno;
use nix::sys::socket::{AddressFamily, SockFlag, SockType};
use nix::unistd::close;
use std::io;
use std::os::fd::{FromRawFd, IntoRawFd, OwnedFd};
use std::os::unix::net::UnixDatagram;
use std::os::unix::process::CommandExt;
use std::process::{Command, Output};

use crate::ipc::{SandboxChildIpc, SandboxIpc};

/// Check if podman is available.
pub fn podman_available() -> bool {
    Command::new("podman")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Builder for podman run invocations.
#[derive(Default)]
pub struct PodmanBuilder {
    args: Vec<String>,
    ipc_child_fd: Option<OwnedFd>,
}

impl PodmanBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// `podman run`
    pub fn run_cmd(mut self) -> Self {
        self.args.push("run".into());
        self
    }

    pub fn rm(mut self) -> Self {
        self.args.push("--rm".into());
        self
    }

    pub fn detach(mut self) -> Self {
        self.args.push("--detach".into());
        self
    }

    pub fn name(mut self, name: &str) -> Self {
        self.args.push("--name".into());
        self.args.push(name.into());
        self
    }

    pub fn tty(mut self) -> Self {
        self.args.push("--tty".into());
        self
    }

    pub fn interactive(mut self) -> Self {
        self.args.push("--interactive".into());
        self
    }

    pub fn device(mut self, spec: &str) -> Self {
        self.args.push("--device".into());
        self.args.push(spec.into());
        self
    }

    pub fn allow_input_devices(mut self) -> Self {
        self.args.push("--device-cgroup-rule=\"c 13:* rwm\"".into());
        self
    }

    pub fn volume(mut self, spec: &str) -> Self {
        self.args.push("-v".into());
        self.args.push(spec.into());
        self
    }

    pub fn publish(mut self, spec: &str) -> Self {
        self.args.push("--publish".into());
        self.args.push(spec.into());
        self
    }

    pub fn env(mut self, key: &str, value: &str) -> Self {
        self.args.push("-e".into());
        self.args.push(format!("{key}={value}"));
        self
    }

    pub fn group_add(mut self, group: u32) -> Self {
        self.args.push("--group-add".into());
        self.args.push(group.to_string());
        self
    }

    pub fn security_opt(mut self, opt: &str) -> Self {
        self.args.push("--security-opt".into());
        self.args.push(opt.into());
        self
    }

    /// Enable bidirectional IPC using a Unix seqpacket socketpair.
    pub fn with_ipc(mut self) -> io::Result<(Self, SandboxIpc)> {
        let (parent, child) = nix::sys::socket::socketpair(
            AddressFamily::Unix,
            SockType::SeqPacket,
            None,
            SockFlag::empty(),
        )
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        // Parent side
        let parent_sock = unsafe { UnixDatagram::from_raw_fd(parent.into_raw_fd()) };

        // Child side must become FD 3 inside container
        self.ipc_child_fd = Some(child);

        self.args.push("--preserve-fds=1".into());

        Ok((self, SandboxIpc { sock: parent_sock }))
    }

    /// Final image reference
    pub fn image(mut self, image: &str) -> Self {
        self.args.push(image.into());
        self
    }

    /// Optional command override inside the container
    pub fn command(mut self, cmd: &[&str]) -> Self {
        self.args.extend(cmd.iter().map(|s| s.to_string()));
        self
    }

    pub fn run(mut self) -> io::Result<Output> {
        println!("Arguments for podman: {:?}", &self.args);

        let mut cmd = Command::new("podman");

        if let Some(fd) = self.ipc_child_fd.take() {
            // give up ownership of ipc_child_fd in host process.
            let fd = fd.into_raw_fd();

            // Move child FD to 3. Note that the FD 3 needs to be linked at the
            // beginning of the child program.
            unsafe {
                cmd.pre_exec(move || {
                    let res = libc::dup2(fd, SandboxChildIpc::FD);
                    Errno::result(res)
                        .map(drop)
                        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                    close(fd).ok();
                    Ok(())
                })
            };
        }

        cmd.args(&self.args).output()
    }
}

#[cfg(feature = "requires-podman")]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn podman_builder_smoke() {
        if !podman_available() {
            panic!("podman not available");
        }

        let out = PodmanBuilder::new()
            .run_cmd()
            .rm()
            //.detach()
            .name(&format!("vuinputd-podman-tests"))
            .image("localhost/vuinputd-tests:latest")
            .command(&["/test-ok"])
            .run()
            .unwrap();

        println!("Output");
        println!("stdout: {}", str::from_utf8(&out.stdout).unwrap());
        println!("stderr: {}", str::from_utf8(&out.stderr).unwrap());

        assert!(out.status.success());
    }
}
