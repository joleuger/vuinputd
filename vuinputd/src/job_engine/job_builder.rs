/*
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use futures::FutureExt; // for .timeout()

pub struct JobBuilder<J: Job> {
    inner: J,
    timeout: Option<Duration>,
    cancel_token: Option<Arc<AtomicBool>>,
    execute_despite_cancellation: bool,
    log: bool,
}

impl<J: Job> JobBuilder<J> {
    pub fn new(inner: J) -> Self {
        Self {
            inner,
            timeout: None,
            cancel_token: None,
            log: false,
        }
    }

    pub fn with_timeout(mut self, dur: Duration) -> Self {
        self.timeout = Some(dur);
        self
    }

    pub fn with_cancellation(mut self, token: Arc<AtomicBool>) -> Self {
        self.cancel_token = Some(token);
        self
    }

    pub fn execute_despite_cancellation(mut self, execute: bool) -> Self {
        self.execute_despite_cancellation = execute;
        self
    }

    pub fn with_logging(mut self) -> Self {
        self.log = true;
        self
    }

    pub fn build(self) -> WrappedJob<J> {
        WrappedJob {
            inner: self.inner,
            timeout: self.timeout,
            cancel_token: self.cancel_token,
            log: self.log,
        }
    }
}

pub struct WrappedJob<J: Job> {
    inner: J,
    timeout: Option<Duration>,
    cancel_token: Option<Arc<AtomicBool>>,
    log: bool,
}

impl<J: Job> Job for WrappedJob<J> {
    fn desc(&self) -> &str {
        self.inner.desc()
    }

    fn create_task(self: Box<Self>) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async move {
            let desc = self.inner.desc().to_string();
            let mut fut = self.inner.create_task();

            if 

            // Logging
            if self.log {
                println!("[START] {desc}");
            }

            // Cancellation should work cooperatively
            if let Some(token) = self.cancel_token.clone() {
                fut = Box::pin(async move {
                    futures::select! {
                        _ = fut.fuse() => {},
                        _ = async {
                            while !token.load(std::sync::atomic::Ordering::Relaxed) {
                                futures_timer::Delay::new(Duration::from_millis(50)).await;
                            }
                        }.fuse() => {},
                    }
                });
            }

            // Timeout
            if let Some(dur) = self.timeout {
                fut = Box::pin(fut.timeout(dur).map(|_| ()));
            }

            fut.await;

            if self.log {
                println!("[DONE]  {desc}");
            }
        })
    }
}
 */