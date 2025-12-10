// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::future::Future;
use std::pin::Pin;

use crate::job_engine::job::{Dispatcher, Job, JobTarget};

pub struct ClosureJob {
    desc: String,
    execute_after_cancellation: bool,
    target: JobTarget,
    task_creator: Box<dyn Fn(&ClosureJob) -> Pin<Box<dyn Future<Output = ()>>> + Send + 'static>,
}

impl ClosureJob {
    pub fn new(
        desc: impl Into<String>,
        target: JobTarget,
        execute_after_cancellation: bool,
        f: Box<
            dyn Fn(&ClosureJob) -> Pin<Box<dyn Future<Output = ()>>> // closure returns any future
                + Send // the closure itself can be sent across threads
                + 'static,
        >,
    ) -> Self
where {
        Self {
            desc: desc.into(),
            execute_after_cancellation,
            target,
            task_creator: f,
        }
    }
}

impl Job for ClosureJob {
    fn desc(&self) -> &str {
        &self.desc
    }

    fn execute_after_cancellation(&self) -> bool {
        self.execute_after_cancellation
    }

    fn create_task(self: &ClosureJob) -> Pin<Box<dyn Future<Output = ()>>> {
        let creator = &self.task_creator;
        let task = creator(self);
        task
    }

    fn job_target(&self) -> JobTarget {
        self.target.clone()
    }
}

/// Example usage
#[test]
pub fn example() {
    let mut dispatcher = Dispatcher::new();

    // Send a Host job
    dispatcher.dispatch(Box::new(ClosureJob::new(
        "Host maintenance",
        JobTarget::Host,
        false,
        Box::new(|job: &ClosureJob| {
            let target = job.target.clone();
            Box::pin(async move {
                println!("Running host job on {:?}", target);
            })
        }),
    )));

    // Sending a Container job works the same
    // dispatcher.dispatch(Job::new(JobTarget::Container(ns.clone()), "Container task", false, |target| async move {
    //     println!("Running container job for {:?}", target);
    // }));

    //
    // JOB_DISPATCHER.get().unwrap().lock().unwrap().dispatch(Box::new(ClosureJob::new("Monitor udev events", JobTarget::BackgroundLoop,false,
    //     Box::new(move |_target| Box::pin(monitor_udev::udev_monitor_loop(cancel_token.clone()))))));

    // Allow loops to run briefly before dropping all senders -> graceful shutdown
    dispatcher.close();
    dispatcher.wait_until_finished();
}
