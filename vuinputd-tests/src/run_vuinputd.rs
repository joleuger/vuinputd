// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::{
    os::unix::process::CommandExt,
    process::{Child, Command},
    sync::OnceLock,
    thread,
    time::Duration,
};

use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;

/// Global singleton
static VUINPUTD: OnceLock<VuinputdGuard> = OnceLock::new();

pub fn ensure_vuinputd_running() {
    VUINPUTD.get_or_init(|| VuinputdGuard::start());
}

struct VuinputdGuard {
    child: Child,
}

impl VuinputdGuard {
    fn start() -> Self {
        println!("Executing vuinputd located via cargo run");
        let child = unsafe {
            Command::new("cargo")
                .args([
                    "run",
                    "-p",
                    "vuinputd",
                    "--",
                    "--major",
                    "120",
                    "--minor",
                    "414796",
                    "--devname",
                    "vuinputd-test",
                ])
                .pre_exec(|| {
                    // Last resort, if the parent just is killed.
                    libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
                    Ok(())
                })
                .spawn()
                .expect("failed to start vuinputd")
        };

        // Optional: give it time to create /dev/vuinput
        thread::sleep(Duration::from_millis(1000));

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
