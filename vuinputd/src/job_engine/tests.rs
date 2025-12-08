

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
