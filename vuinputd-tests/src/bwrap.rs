// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::io;
use std::os::fd::{FromRawFd, IntoRawFd, OwnedFd};
use std::os::unix::net::UnixDatagram;
use std::os::unix::process::CommandExt;
use std::process::{Command, Output};

use nix::errno::Errno;
use nix::sys::socket::{socketpair, AddressFamily, SockFlag, SockType};
use nix::unistd::close;

use crate::ipc::{SandboxChildIpc, SandboxIpc};

/// Check if bubblewrap is available.
pub fn bwrap_available() -> bool {
    Command::new("bwrap")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Builder for bubblewrap invocations.
#[derive(Default)]
pub struct BwrapBuilder {
    args: Vec<String>,
    ipc_child_fd: Option<OwnedFd>,
}

impl BwrapBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn unshare_all(mut self) -> Self {
        self.args.push("--unshare-all".into());
        self
    }

    pub fn unshare_net(mut self) -> Self {
        self.args.push("--unshare-net".into());
        self
    }

    pub fn proc(mut self) -> Self {
        self.args.push("--proc".into());
        self.args.push("/proc".into());
        self
    }

    pub fn dev(mut self) -> Self {
        // for our tests, we cannot simply use the "--dev"-flag, because it creates a tmpfs with the nodev flag
        // see SETUP_MOUNT_DEV and PRIV_SEP_OP_TMPFS_MOUNT
        // in https://github.com/containers/bubblewrap/blob/v0.11.0/bubblewrap.c#L1370-L1376 .
        // So, we mount a temporary directory that does not have this restrictions.
        self.args.extend([
            "--dev-bind".into(),
            "/run/vuinputd/vuinput-test/dev".into(),
            "/dev".into(),
            "--dev-bind".into(),
            "/run/vuinputd/vuinput-test/dev-input".into(),
            "/dev/input".into(),
        ]);
        self
    }

    pub fn tmpfs(mut self, path: &str) -> Self {
        self.args.push("--tmpfs".into());
        self.args.push(path.into());
        self
    }

    // https://superuser.com/questions/1577262/bwrap-execvp-no-such-file-or-directory-when-ro-binding-non-root-path
    pub fn ro_bind(mut self, src: &str, dst: &str) -> Self {
        self.args
            .extend(["--ro-bind".into(), src.into(), dst.into()]);
        self
    }

    pub fn bind(mut self, src: &str, dst: &str) -> Self {
        self.args.extend(["--bind".into(), src.into(), dst.into()]);
        self
    }

    pub fn dev_bind(mut self, src: &str, dst: &str) -> Self {
        self.args
            .extend(["--dev-bind".into(), src.into(), dst.into()]);
        self
    }

    /// Ensure the container dies if the parent dies.
    ///
    /// This uses bwrap's `--die-with-parent` flag, which internally
    /// uses a parent-death signal (PR_SET_PDEATHSIG).
    pub fn die_with_parent(mut self) -> Self {
        self.args.push("--die-with-parent".into());
        self
    }

    /// Enable bidirectional IPC using a Unix seqpacket socketpair.
    pub fn with_ipc(mut self) -> io::Result<(Self, SandboxIpc)> {
        let (parent, child) = socketpair(
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

        Ok((self, SandboxIpc { sock: parent_sock }))
    }

    /// Final command executed inside the container.
    pub fn command(mut self, cmd: &str, args: &[&str]) -> Self {
        self.args.push("--".into());
        self.args.push(cmd.into());
        self.args.extend(args.iter().map(|s| s.to_string()));
        self
    }

    pub fn run(mut self) -> io::Result<Output> {
        println!("Arguments for bwrap: {:?}", &self.args);

        let mut cmd = Command::new("bwrap");

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

#[cfg(feature = "requires-bwrap")]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bwrap_works() {
        if !bwrap_available() {
            panic!("bwrap not available");
        }

        let out = BwrapBuilder::new()
            .unshare_net()
            //.proc()
            .ro_bind("/", "/")
            .tmpfs("/tmp")
            .die_with_parent()
            .command("/usr/bin/echo", &[])
            .run()
            .unwrap_or_else(|e| panic!("failed to run bwrap!: {e}"));

        println!("Output");
        println!("stdout: {}", str::from_utf8(&out.stdout).unwrap());
        println!("stderr: {}", str::from_utf8(&out.stderr).unwrap());

        assert!(out.status.success());
    }
}
