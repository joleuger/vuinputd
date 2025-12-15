// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::{
    process::{Child, Command},
    sync::OnceLock,
    time::Duration,
    thread,
};

use nix::sys::signal::{self,Signal};
use nix::unistd::Pid;

/// Global singleton
static VUINPUTD: OnceLock<VuinputdGuard> = OnceLock::new();

pub fn ensure_vuinputd_running() {
    VUINPUTD.get_or_init(|| {
        VuinputdGuard::start()
    });
}

struct VuinputdGuard {
    child: Child,
}

impl VuinputdGuard {
    fn start() -> Self {
        println!("Executing vuinputd located via cargo run");
        let child = Command::new("cargo")
            .args(["run", "-p", "vuinputd", "--","--major","120","--minor","414796","--devname","vuinputd-test"])
            // adjust args/env if needed
            .spawn()
            .expect("failed to start vuinputd");

        // Optional: give it time to create /dev/vuinput
        thread::sleep(Duration::from_millis(300));

        Self { child }
    }
}

impl Drop for VuinputdGuard {
    fn drop(&mut self) {
        let pid = Pid::from_raw(self.child.id() as i32);

        // First: SIGTERM
        let _ = signal::kill(pid, Signal::SIGTERM);

        // Wait a bit
        for _ in 0..10 {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }

        // Still alive → SIGKILL
        let _ = signal::kill(pid, Signal::SIGKILL);
        let _ = self.child.wait();
    }
}
