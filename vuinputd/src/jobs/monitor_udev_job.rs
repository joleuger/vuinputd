// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::{
    collections::HashMap,
    future::Future,
    os::fd::{AsFd, AsRawFd, BorrowedFd, RawFd},
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};

use async_io::Async;
use libudev::Monitor;
use log::debug;
use regex::Regex;

use crate::job_engine::job::{Job, JobTarget};

// === Basic types ===

#[derive(Debug, Clone)]
pub enum EventKind {
    Add,
    Remove,
}

#[derive(Debug, Clone)]
pub struct UdevEvent {
    pub syspath: String,
    pub seqnum: u64,
    pub kind: EventKind,
    pub payload: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub syspath: String,
    pub seqnum: u64,
    pub add_data: Option<HashMap<String, String>>,
    pub remove_data: Option<HashMap<String, String>>,
    pub add_processed: bool,
    pub tombstone: bool,
    pub last_update: Instant,
}

// === EventStore ===

#[derive(Debug)]
pub struct EventStore {
    entries: HashMap<String, Entry>,
    ttl: Duration,
}

impl EventStore {
    pub fn new(ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            ttl,
        }
    }

    pub fn on_event(&mut self, event: UdevEvent) {
        let now = Instant::now();
        let e = self
            .entries
            .entry(event.syspath.clone())
            .or_insert_with(|| Entry {
                syspath: event.syspath.clone(),
                seqnum: event.seqnum,
                add_data: None,
                remove_data: None,
                add_processed: false,
                tombstone: false,
                last_update: now,
            });

        e.seqnum = event.seqnum;
        e.last_update = now;
        e.tombstone = false;

        match event.kind {
            EventKind::Add => {
                e.add_data = Some(event.payload);
                e.add_processed = false;
                e.remove_data = None;
            }
            EventKind::Remove => {
                e.remove_data = Some(event.payload);
            }
        }
    }

    pub fn take(&mut self, syspath: &str) -> Option<Entry> {
        let e = self.entries.get_mut(syspath)?;

        let result = e.clone();

        if e.tombstone {
            return Some(result);
        }

        if !e.add_processed {
            e.add_processed = true;
        }
        if e.remove_data.is_some() {
            e.tombstone = true;
        }

        Some(result)
    }

    pub fn cleanup(&mut self) {
        let now = Instant::now();
        self.entries.retain(|_, e| {
            if e.tombstone {
                return false;
            }
            now.duration_since(e.last_update) < self.ttl
        });
    }
}

// === Global store ===

pub static EVENT_STORE: OnceLock<Arc<Mutex<EventStore>>> = OnceLock::new();

pub struct MonitorBackgroundLoop {}
impl MonitorBackgroundLoop {
    pub fn new() -> Self {
        Self {}
    }
}

impl Job for MonitorBackgroundLoop {
    fn desc(&self) -> &str {
        "Monitor udev events"
    }

    fn execute_after_cancellation(&self) -> bool {
        false
    }
    fn create_task(self: &MonitorBackgroundLoop) -> Pin<Box<dyn Future<Output = ()>>> {
        let cancel_token: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
        Box::pin(udev_monitor_loop(cancel_token))
    }

    fn job_target(&self) -> JobTarget {
        JobTarget::BackgroundLoop
    }
}

pub async fn udev_monitor_loop(cancel_token: Arc<AtomicBool>) {
    // Clone a reference to the shared store which should already be initialized in main.

    // Initialize shared store
    let store = Arc::new(Mutex::new(EventStore::new(Duration::from_secs(60))));
    EVENT_STORE.set(store.clone()).unwrap();

    // Create monitor that listens for kernel events.
    // Use match_subsystem to filter for "input" subsystem as requested.
    debug!("Monitor started");
    let mut next_cleanup = Instant::now() + Duration::from_secs(60);

    let context = libudev::Context::new().unwrap();
    let mut monitor = Monitor::new(&context).unwrap();
    monitor.match_subsystem("input").unwrap();
    let mut monitor_socket = monitor.listen().expect("Failed to create udev monitor");

    // Wrap the monitor in a small AsFd adapter
    struct FdWrap(RawFd);
    impl AsRawFd for FdWrap {
        fn as_raw_fd(&self) -> RawFd {
            self.0
        }
    }
    impl AsFd for FdWrap {
        fn as_fd(&self) -> BorrowedFd<'_> {
            // SAFETY: FdWrap owns the fd and lives long enough
            unsafe { BorrowedFd::borrow_raw(self.0) }
        }
    }

    let async_monitor = Async::new(FdWrap(monitor_socket.as_raw_fd())).unwrap();

    let re = Regex::new(r"^/devices/virtual/input/input(\d+)/event(\d+)$").unwrap();

    loop {
        // check cancel token first
        if cancel_token.load(Ordering::Relaxed) {
            debug!("Cancellation requested, shutting down udev monitor thread.");
            break;
        }
        debug!("Waiting for event");
        async_monitor.readable().await.unwrap();
        debug!("Event registered");

        if let Some(event) = monitor_socket.receive_event() {
            let mut properties: HashMap<_, _> = HashMap::new();
            for property in event.properties() {
                let key: String = property.name().to_str().unwrap().to_string();
                let key = match key.as_str() {
                    "ID_VUINPUT_KEYBOARD" => "ID_INPUT_KEYBOARD".to_string(),
                    "ID_VUINPUT_MOUSE" => "ID_INPUT_MOUSE".to_string(),
                    _ => key,
                };

                let value: String = property.value().to_str().unwrap().to_string();
                if key != "ID_SEAT" {
                    properties.insert(key, value);
                }
            }

            let value_of_devpath = properties.get("DEVPATH").unwrap();

            if let Some(caps) = re.captures(value_of_devpath) {
                // result is something like /devices/virtual/input/input126/event9
                // println!("devpath {}",value_of_devpath);
                let input_number: u32 = caps[1].parse().unwrap();
                // let event_number: u32 = caps[2].parse().unwrap();
                let syspath = format!("/sys/devices/virtual/input/input{}", input_number);
                let seqnum: u64 = properties.get("SEQNUM").unwrap().parse().unwrap();
                let kind = match properties.get("ACTION").unwrap().as_str() {
                    "ADD" => EventKind::Add,
                    "REMOVE" => EventKind::Remove,
                    _ => EventKind::Add,
                };

                let mut event_store = EVENT_STORE.get().unwrap().lock().unwrap();
                let udev_event = UdevEvent {
                    syspath: syspath,
                    seqnum: seqnum,
                    kind: kind,
                    payload: properties,
                };
                event_store.on_event(udev_event);
            }
        }

        if Instant::now() > next_cleanup {
            next_cleanup = Instant::now() + Duration::from_secs(60);
            EVENT_STORE.get().unwrap().lock().unwrap().cleanup();
        }
    } // loop

    debug!("udev monitor thread exiting.");
}

// === Example threads ===
/*
fn producer_thread(stop: Arc<AtomicBool>) {
    let store = EVENT_STORE.get().unwrap().clone();
    let mut seq = 0u64;

    while !stop.load(Ordering::Relaxed) {
        // Simulate some udev events
        let syspath = if seq % 2 == 0 {
            "/devices/input/event9"
        } else {
            "/devices/input/event10"
        };

        let kind = if seq % 3 == 0 {
            EventKind::Remove
        } else {
            EventKind::Add
        };

        let event = UdevEvent {
            syspath: syspath.to_string(),
            seqnum: seq,
            kind,
            payload: format!("payload for seq {seq}"),
        };

        {
            let mut guard = store.lock().unwrap();
            guard.on_event(event);
        }

        seq += 1;
        thread::sleep(Duration::from_millis(400));
    }

    println!("[producer] exiting");
}

fn consumer_thread(stop: Arc<AtomicBool>) {
    let store = EVENT_STORE.get().unwrap().clone();

    while !stop.load(Ordering::Relaxed) {
        {
            let mut guard = store.lock().unwrap();

            // Example: iterate over all known syspaths
            let syspaths: Vec<String> = guard.entries.keys().cloned().collect();
            for syspath in syspaths {
                if let Some(entry) = guard.take(&syspath) {
                    println!("[consumer] Got actionable event: {:?}", entry);
                }
            }

            guard.cleanup();
        }

        thread::sleep(Duration::from_millis(200));
    }

    println!("[consumer] exiting");
}

fn main() {
    // Initialize shared store
    let store = Arc::new(Mutex::new(EventStore::new(Duration::from_secs(60))));
    EVENT_STORE.set(store.clone()).unwrap();

    // Shared stop flag
    let stop_flag = Arc::new(AtomicBool::new(false));

    let p_stop = stop_flag.clone();
    let producer = thread::spawn(move || producer_thread(p_stop));

    let c_stop = stop_flag.clone();
    let consumer = thread::spawn(move || consumer_thread(c_stop));

    // Let it run for 5 seconds
    thread::sleep(Duration::from_secs(5));
    stop_flag.store(true, Ordering::Relaxed);

    producer.join().unwrap();
    consumer.join().unwrap();

    println!("Main exiting");
}
 */
