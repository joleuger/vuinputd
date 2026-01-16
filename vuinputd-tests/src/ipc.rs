// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::{
    io,
    os::{
        fd::{FromRawFd, RawFd},
        unix::net::UnixDatagram,
    },
    time::Duration,
};

/// IPC handle kept by the parent.
pub struct SandboxIpc {
    pub sock: UnixDatagram,
}

impl SandboxIpc {
    pub fn recv(&self, read_timeout: Option<Duration>) -> io::Result<Vec<u8>> {
        let mut buf = vec![0u8; 4096];
        self.sock.set_read_timeout(read_timeout)?;
        let n = self.sock.recv(&mut buf)?;
        buf.truncate(n);
        Ok(buf)
    }

    pub fn send(&self, data: &[u8]) -> io::Result<()> {
        self.sock.send(data)?;
        Ok(())
    }
}

/// IPC handle inside the container.
pub struct SandboxChildIpc {
    sock: UnixDatagram,
}

impl SandboxChildIpc {
    /// FD number is fixed and known.
    pub const FD: RawFd = 3;

    /// # Safety
    /// Must only be called once in the child.
    pub unsafe fn from_fd() -> Self {
        let sock = UnixDatagram::from_raw_fd(Self::FD);
        Self { sock }
    }

    pub fn send(&self, data: &[u8]) -> io::Result<()> {
        self.sock.send(data)?;
        Ok(())
    }

    pub fn recv(&self, read_timeout: Option<Duration>) -> io::Result<Vec<u8>> {
        let mut buf = vec![0u8; 4096];
        self.sock.set_read_timeout(read_timeout)?;
        let n = self.sock.recv(&mut buf)?;
        buf.truncate(n);
        Ok(buf)
    }
}
