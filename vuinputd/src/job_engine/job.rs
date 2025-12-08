// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use async_channel::{Receiver, Sender};
use futures::executor::{LocalPool, LocalSpawner};
use futures::future::RemoteHandle;
use futures::task::LocalSpawnExt;
use log::debug;
use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

use crate::process_tools::RequestingProcess;

// To discuss:
// what we handle here, could also be named Task. The decision for job was more or less
// because the main goal was to run some short "scripts" that create files etc.
// see e.g., https://blog.yoshuawuyts.com/async-cancellation-1/

/// Represents where a job should run.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum JobTarget {
    /// A global or host-wide task.
    Host,
    BackgroundLoop,
    /// A specific container or namespace target.
    Container(RequestingProcess),
}

pub trait Job: Send + 'static {
    /// Free-form description, used for logging or debugging
    fn desc(&self) -> &str;

    /// Job Target
    fn job_target(&self) -> JobTarget;

    /// Whether the job should still execute after cancellation
    fn execute_after_cancellation(&self) -> bool {
        false
    }

    /// Main entry point — creates the future that executes this job
    fn create_task(self: &Self) -> Pin<Box<dyn Future<Output = ()>>>;
}

impl std::fmt::Debug for dyn Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Job")
            .field("target", &self.job_target())
            .field("desc", &self.desc())
            .finish()
    }
}

/// Central dispatcher that manages per-target async loops.
#[derive(Debug)]
pub struct Dispatcher {
    thread_handle: Option<JoinHandle<()>>,
    tx: Option<Sender<Box<dyn Job>>>,
    future_handles: Arc<Mutex<Vec<RemoteHandle<()>>>>,
}

impl Dispatcher {
    /// Create a new dispatcher and return its sender handle.
    pub fn new() -> Self {
        let (tx, rx) = async_channel::unbounded();

        // Map of active per-target senders.
        let targets: Arc<Mutex<HashMap<JobTarget, Sender<Box<dyn Job>>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let rx_in_thread: Receiver<Box<dyn Job>> = rx.clone();
        let future_handles: Arc<Mutex<Vec<RemoteHandle<()>>>> = Arc::new(Mutex::new(Vec::new()));
        let future_handles_for_thread = future_handles.clone();
        // run dispatcher in a dedicated thread
        let thread_handle = thread::spawn(move || {
            let mut pool = LocalPool::new();
            let spawner = pool.spawner();

            let dispatcher_loop_handle = spawner
                .spawn_local_with_handle(spawn_dispatcher_loop(
                    spawner.clone(),
                    targets,
                    rx_in_thread,
                    future_handles_for_thread.clone(),
                ))
                .unwrap();
            future_handles_for_thread
                .lock()
                .unwrap()
                .push(dispatcher_loop_handle);
            pool.run(); // blocks until all tasks complete
        });

        Self {
            thread_handle: Some(thread_handle),
            tx: Some(tx),
            future_handles: future_handles,
        }
    }

    pub fn dispatch(&mut self, job: Box<dyn Job>) {
        self.tx
            .as_ref()
            .expect("Dispatcher already closed")
            .send_blocking(job)
            .unwrap();
    }

    pub fn close(&mut self) {
        self.tx = None;
        debug!("Checking for running jobs before shutdown");
        self.future_handles.lock().unwrap().clear();
        debug!("Pending jobs canceled");
    }

    pub fn wait_until_finished(&mut self) {
        self.tx = None;
        self.future_handles.lock().unwrap().clear();
        let handle = self.thread_handle.take();
        handle.unwrap().join().unwrap();
    }
}

/// Run the dispatcher: listen for incoming jobs and route them to the right loop.
async fn spawn_dispatcher_loop(
    spawner: LocalSpawner,
    targets: Arc<Mutex<HashMap<JobTarget, Sender<Box<dyn Job>>>>>,
    rx: Receiver<Box<dyn Job>>,
    future_handles: Arc<Mutex<Vec<RemoteHandle<()>>>>,
) {
    loop {
        let received_job = rx.recv().await;
        match received_job {
            Ok(job) => {
                if job.job_target() == JobTarget::BackgroundLoop {
                    // this is a separate loop that just runs in parallel and does not need a queue to be ordered.
                    let background_loop_handle =
                        spawner.spawn_local_with_handle(job.create_task()).unwrap();
                    future_handles.lock().unwrap().push(background_loop_handle);
                    log::info!("Spawned new background loop for {:?}", job.desc());
                } else {
                    let target = job.job_target();
                    let (tx, newly_created) = get_or_spawn_target_loop(
                        spawner.clone(),
                        targets.clone(),
                        target.clone(),
                        future_handles.clone(),
                    )
                    .await;
                    if newly_created {
                        log::info!("Spawned new loop for {:?}", target);
                    }
                    if let Err(e) = tx.send(job).await {
                        log::warn!("Failed to enqueue job: {e}");
                    }
                }
            }
            Err(_err) => {
                // channel has been closed
                log::info!("Channel has been closed {:?}", _err);
                break;
            }
        }
    }
    log::info!("Global dispatcher shutting down gracefully");
}

/// Get or lazily create a target-specific queue and loop.
async fn get_or_spawn_target_loop(
    spawner: LocalSpawner,
    targets: Arc<Mutex<HashMap<JobTarget, Sender<Box<dyn Job>>>>>,
    target: JobTarget,
    future_handles: Arc<Mutex<Vec<RemoteHandle<()>>>>,
) -> (Sender<Box<dyn Job>>, bool) {
    let mut map = targets.lock().unwrap();
    if let Some(tx) = map.get(&target) {
        return (tx.clone(), false);
    }

    let (tx, rx) = async_channel::unbounded();
    map.insert(target.clone(), tx.clone());
    drop(map); // release lock before spawning

    let job_target_loop_handle = spawner
        .spawn_local_with_handle(job_target_loop(target.clone(), rx))
        .unwrap();
    future_handles.lock().unwrap().push(job_target_loop_handle);

    (tx, true)
}

/// The main loop for a single job target (container or host).
async fn job_target_loop(target: JobTarget, rx: Receiver<Box<dyn Job>>) {
    log::info!("Starting loop for {:?}", target);
    while let Ok(job) = rx.recv().await {
        log::debug!("Executing job: {}", job.desc());
        job.create_task().await;
    }
    log::info!("Loop for {:?} ended — channel closed", target);
}

/*
macro_rules! job {
    ($desc:expr, async move { $($body:tt)* }) => {
        Box::new(ClosureJob::new($desc, |_: JobTarget| async move { $($body)* }))
    };
}
     */
