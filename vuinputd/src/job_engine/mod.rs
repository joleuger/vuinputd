// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>
//! # Design: Async Per-Container Job Executor (Tokio)
//!
//! ## Overview
//! A scalable, structured design for running async jobs per container.
//!
//! - Global dispatcher routes jobs to per-container async loops or to global queue.
//! - Each container has its own unbounded job queue (no backpressure).
//! - Loops are spawned lazily on first job and exit when their sender drops.
//! - Graceful shutdown happens automatically (channel close → loop exit).
//! - Periodic cleanup removes idle container queues.
//!
//! ## Async Jobs
//! - Each `Job` contains an async closure `task: Box<dyn FnOnce(JobTarget) -> Pin<Box<dyn Future<Output = ()>>> + Send>`
//! - This allows full async/await usage inside the job body.
//!
//!
//!         +--------------------------------------+
//!         |            Global dispatcher         |
//!         +----------+---------------------------+
//!                    |                     |
//!                    v                     v
//!         +----------+-----------+  +------------+
//!         | Per-container queues |  | Host queue |
//!         +----+------+----+-----+  +------------+
//!              |           |               |
//!         +----v----+  +---v----+      +---v----+
//!         | Cont A  |  | Cont B |      | Cont C |
//!         | loop()  |  | loop() |      | loop() |
//!         +---------+  +--------+      +--------+

use std::sync::{Mutex, OnceLock};

use crate::job_engine::job::Dispatcher;

pub mod closure_job;
pub mod job;

pub static JOB_DISPATCHER: OnceLock<Mutex<Dispatcher>> = OnceLock::new();

#[cfg(test)]
mod tests;
