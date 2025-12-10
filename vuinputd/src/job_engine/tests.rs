use crate::job_engine::closure_job::ClosureJob;
use crate::job_engine::job::{Dispatcher, JobTarget};

use super::*;
use futures::executor::LocalPool;
use futures::task::LocalSpawnExt;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// Simple shared integer counter
fn shared_counter() -> Arc<Mutex<i32>> {
    Arc::new(Mutex::new(0))
}

//
// 1. Ordering test
//
#[test]
fn test_job_ordering() {
    let mut dispatcher = Dispatcher::new();
    let c = shared_counter();

    let c1 = c.clone();
    dispatcher.dispatch(Box::new(ClosureJob::new(
        "set to 5",
        JobTarget::Host,
        false,
        Box::new(move |_job| {
            let c1 = c1.clone();
            Box::pin(async move {
                *c1.lock().unwrap() = 5;
            })
        }),
    )));

    // job 2: increment to 6
    let c2 = c.clone();
    dispatcher.dispatch(Box::new(ClosureJob::new(
        "increment to 6",
        JobTarget::Host,
        false,
        Box::new(move |_job| {
            let c2 = c2.clone();
            Box::pin(async move {
                *c2.lock().unwrap() += 1;
            })
        }),
    )));

    dispatcher.close();
    dispatcher.wait_until_finished();

    assert_eq!(*c.lock().unwrap(), 6);
}

/*

//
// 2. Cancellation test (default: do NOT run cancelled job)
//
#[test]
fn test_cancellation_stops_jobs() {
    let (mut dispatcher, mut pool) = test_dispatcher();
    let c = shared_counter();

    // First job runs
    let c1 = c.clone();
    dispatcher.queue(Job::new(move |_ctx| {
        *c1.borrow_mut() += 1;
        Poll::Ready(())
    }));

    // Second job should NOT run (will be cancelled)
    let c2 = c.clone();
    dispatcher.queue(Job::with_flags(
        move |_ctx| {
            *c2.borrow_mut() += 1;
            Poll::Ready(())
        },
        JobFlags::default(), // execute_after_cancellation = false
    ));

    // Immediately close before job #2 runs
    dispatcher.close();
    run_pool_to_completion(&mut pool);

    assert_eq!(*c.borrow(), 1);
}

//
// 3. Cancellation *with* execute_after_cancellation
//
#[test]
fn test_cancellation_runs_cleanup_jobs() {
    let (mut dispatcher, mut pool) = test_dispatcher();
    let c = shared_counter();

    // First job runs
    let c1 = c.clone();
    dispatcher.queue(Job::new(move |_ctx| {
        *c1.borrow_mut() += 1;
        Poll::Ready(())
    }));

    // Cleanup job must run even after cancellation
    let c2 = c.clone();
    dispatcher.queue(Job::with_flags(
        move |_ctx| {
            *c2.borrow_mut() += 10;
            Poll::Ready(())
        },
        JobFlags {
            execute_after_cancellation: true,
        },
    ));

    dispatcher.close();
    run_pool_to_completion(&mut pool);

    assert_eq!(*c.borrow(), 11);
}

//
// 4. SIGTERM-like shutdown behaviour
//
#[test]
fn test_shutdown_wait_until_finished() {
    let (mut dispatcher, mut pool) = test_dispatcher();
    let c = shared_counter();

    // Simulate some work
    let c1 = c.clone();
    dispatcher.queue(Job::new(move |_ctx| {
        *c1.borrow_mut() += 1;
        Poll::Ready(())
    }));

    dispatcher.close();
    run_pool_to_completion(&mut pool);
    dispatcher.wait_until_finished();

    assert_eq!(*c.borrow(), 1);
}

//
// 5. Failure propagation
//
#[test]
fn test_job_failure_does_not_crash_dispatcher() {
    let (mut dispatcher, mut pool) = test_dispatcher();
    let c = shared_counter();

    dispatcher.queue(Job::new(|_ctx| {
        panic!("intentional test panic");
        #[allow(unreachable_code)]
        Poll::Ready(())
    }));

    // Following job should still run if the system recovers
    let c2 = c.clone();
    dispatcher.queue(Job::new(move |_ctx| {
        *c2.borrow_mut() += 1;
        Poll::Ready(())
    }));

    dispatcher.close();

    // Some designs need catch_unwind here; yours might not.
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_pool_to_completion(&mut pool);
    }));

    assert_eq!(*c.borrow(), 1);
}

//
// 6. Stress: many short jobs
//
#[test]
fn test_many_small_jobs() {
    let (mut dispatcher, mut pool) = test_dispatcher();
    let count = shared_counter();

    for _ in 0..200 {
        let c = count.clone();
        dispatcher.queue(Job::new(move |_ctx| {
            *c.borrow_mut() += 1;
            Poll::Ready(())
        }));
    }

    dispatcher.close();
    run_pool_to_completion(&mut pool);

    assert_eq!(*count.borrow(), 200);
}

//
// 7. Ensure no jobs run after final close
//
#[test]
fn test_no_jobs_after_close() {
    let (mut dispatcher, mut pool) = test_dispatcher();
    dispatcher.close();

    let executed = shared_counter();
    let e = executed.clone();

    dispatcher.queue(Job::new(move |_ctx| {
        *e.borrow_mut() += 1;
        Poll::Ready(())
    }));

    // Dispatcher should ignore queued jobs after close()
    run_pool_to_completion(&mut pool);

    assert_eq!(*executed.borrow(), 0);
}
#[test]
fn test_stress_light_multi_target() {
    use std::sync::{Arc, Mutex};
    use crate::jobs::{Dispatcher, JobTarget};
    use crate::jobs::closure_job::ClosureJob;

    // Shared result buffer
    let results = Arc::new(Mutex::new(Vec::new()));

    let mut dispatcher = Dispatcher::new();

    // Create a few job targets
    let target_a = JobTarget::Container("A".into());
    let target_b = JobTarget::Container("B".into());
    let host = JobTarget::Host;

    // Create 10 jobs per target
    for i in 0..10 {
        let results_a = results.clone();
        dispatcher.dispatch(Box::new(ClosureJob::new(
            format!("A-{i}"),
            target_a.clone(),
            false,
            Box::new(move |_t| {
                Box::pin(async move {
                    results_a.lock().unwrap().push(format!("A-{i}"));
                })
            }),
        )));

        let results_b = results.clone();
        dispatcher.dispatch(Box::new(ClosureJob::new(
            format!("B-{i}"),
            target_b.clone(),
            false,
            Box::new(move |_t| {
                Box::pin(async move {
                    results_b.lock().unwrap().push(format!("B-{i}"));
                })
            }),
        )));

        let results_h = results.clone();
        dispatcher.dispatch(Box::new(ClosureJob::new(
            format!("H-{i}"),
            host.clone(),
            false,
            Box::new(move |_t| {
                Box::pin(async move {
                    results_h.lock().unwrap().push(format!("H-{i}"));
                })
            }),
        )));
    }

    dispatcher.close();
    dispatcher.wait_until_finished();

    let buf = results.lock().unwrap();

    // Each target must preserve *its* order:
    assert_eq!(
        buf.iter().filter(|s| s.starts_with("A-")).cloned().collect::<Vec<_>>(),
        (0..10).map(|i| format!("A-{i}")).collect::<Vec<_>>()
    );

    assert_eq!(
        buf.iter().filter(|s| s.starts_with("B-")).cloned().collect::<Vec<_>>(),
        (0..10).map(|i| format!("B-{i}")).collect::<Vec<_>>()
    );

    assert_eq!(
        buf.iter().filter(|s| s.starts_with("H-")).cloned().collect::<Vec<_>>(),
        (0..10).map(|i| format!("H-{i}")).collect::<Vec<_>>()
    );

    // But between targets, the order may be interleaved (expected)
    // So we don't assert cross-order.
}

#[test]
fn test_cleanup_when_target_disappears() {
    use std::sync::{Arc, Mutex};
    use crate::jobs::{Dispatcher, JobTarget};
    use crate::jobs::closure_job::ClosureJob;

    let results = Arc::new(Mutex::new(Vec::new()));
    let mut dispatcher = Dispatcher::new();

    let target_dead = JobTarget::Container("dead".into());
    let target_alive = JobTarget::Container("alive".into());

    // Two jobs for dead target: one normal, one allowed after cancel
    {
        let r = results.clone();
        dispatcher.dispatch(Box::new(ClosureJob::new(
            "dead-normal",
            target_dead.clone(),
            false,
            Box::new(move |_| Box::pin(async move {
                r.lock().unwrap().push("dead-normal".into());
            }))
        )));

        let r = results.clone();
        dispatcher.dispatch(Box::new(ClosureJob::new(
            "dead-cleanup",
            target_dead.clone(),
            true,  // allowed after cancellation
            Box::new(move |_| Box::pin(async move {
                r.lock().unwrap().push("dead-cleanup".into());
            }))
        )));
    }

    // One job for a live target
    {
        let r = results.clone();
        dispatcher.dispatch(Box::new(ClosureJob::new(
            "alive-job",
            target_alive.clone(),
            false,
            Box::new(move |_| Box::pin(async move {
                r.lock().unwrap().push("alive-job".into());
            }))
        )));
    }

    // Simulate container removal
    dispatcher.target_gone(&target_dead);

    dispatcher.close();
    dispatcher.wait_until_finished();

    let buf = results.lock().unwrap();

    // Should NOT run:
    assert!(!buf.contains(&"dead-normal".into()));

    // Should run because execute_after_cancellation = true
    assert!(buf.contains(&"dead-cleanup".into()));

    // Should run normally
    assert!(buf.contains(&"alive-job".into()));
}
 */
