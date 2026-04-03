// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::{
    fs::File,
    os::fd::{AsFd, BorrowedFd},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::Context;

use cuse_lowlevel::fuse_lowlevel;
use nix::sys::epoll::{Epoll, EpollCreateFlags, EpollEvent, EpollFlags};

use crate::cuse_device::state::{get_vuinput_state, PollPhase, VuFileHandle};

pub static EVDEV_WRITE_WATCHER: OnceLock<Mutex<EvdevWriteWatcher>> = OnceLock::new();

pub fn initialize_evdev_write_watcher() -> anyhow::Result<()> {
    EVDEV_WRITE_WATCHER
        .set(Mutex::new(EvdevWriteWatcher::new()?)) // Convert the error from Mutex<T> to a simple string
        .map_err(|_| anyhow::anyhow!("cell already full"))
        // Now .context() works because &str is compatible
        .context("failed to initialize evdev write watcher")?;
    Ok(())
}

#[derive(Debug)]
pub struct EvdevWriteWatcher {
    epoll: Arc<Epoll>,
    shutdown: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
}

impl EvdevWriteWatcher {
    fn new() -> anyhow::Result<Self> {
        let epoll = Arc::new(Epoll::new(EpollCreateFlags::empty())?);
        let shutdown = Arc::new(AtomicBool::new(false));
        let epoll_thread = epoll.clone();
        let shutdown_thread = shutdown.clone();
        let thread_handle = Some(thread::spawn(move || {
            evdev_write_watch_loop(shutdown_thread, epoll_thread);
        }));
        Ok(Self {
            thread_handle: thread_handle,
            shutdown: shutdown,
            epoll: epoll,
        })
    }

    pub fn add_device(&self, vu_fh: VuFileHandle) -> nix::Result<()> {
        let VuFileHandle::Fh(fh) = vu_fh;

        let vuinput_state_mutex = get_vuinput_state(&vu_fh).unwrap();
        let vuinput_state = vuinput_state_mutex.lock().unwrap();

        self.epoll.add(
            &vuinput_state.file,
            EpollEvent::new(EpollFlags::EPOLLIN, fh),
        )
    }

    pub fn remove_device<Fd: AsFd>(&self, uinput_fd: Fd) -> nix::Result<()> {
        self.epoll.delete(uinput_fd)
    }

    pub fn stop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);

        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }

    pub fn is_running(&self) -> bool {
        self.thread_handle.is_some()
    }
}

fn evdev_write_watch_loop(shutdown: Arc<AtomicBool>, epoll: Arc<Epoll>) {
    let mut events = vec![EpollEvent::empty(); 64];

    loop {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }

        let n = match epoll.wait(&mut events, 500u16) {
            Ok(n) => n,
            Err(err) => {
                eprintln!("evdev_write_watcher: epoll_wait failed: {err}");
                thread::sleep(Duration::from_millis(100));
                continue;
            }
        };

        for ev in &events[..n] {
            let fh_val = ev.data() as u64;
            let fh = VuFileHandle::Fh(fh_val);
            let state = super::state::get_vuinput_state(&fh);
            if let Ok(state) = state {
                let mut state = state.lock().unwrap();

                for handle in state.poll.take_waiters() {
                    unsafe {
                        fuse_lowlevel::fuse_lowlevel_notify_poll(handle.as_ptr());
                        fuse_lowlevel::fuse_pollhandle_destroy(handle.as_ptr());
                    }
                }
                state.poll.pollphase = PollPhase::Readable;
                state.poll.pending.clear();
            }
        }
    }
}
