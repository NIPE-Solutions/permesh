// SPDX-License-Identifier: MIT OR Apache-2.0
//! Cancellation stops waiting; an in-flight native operation cannot be undone.
use crate::error::AppError;
use std::sync::Arc;
const MAX_NATIVE_WORKERS: usize = 4;

#[derive(Clone)]
pub struct BlockingPool {
    workers: Arc<tokio::sync::Semaphore>,
}
impl BlockingPool {
    pub fn new() -> Self {
        Self {
            workers: Arc::new(tokio::sync::Semaphore::new(MAX_NATIVE_WORKERS)),
        }
    }
    pub async fn run<T: Send + 'static>(
        &self,
        operation: impl FnOnce() -> T + Send + 'static,
    ) -> Result<T, AppError> {
        let permit = self
            .workers
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| AppError::new(5, "Native I/O workers unavailable"))?;
        let (send, receive) = tokio::sync::oneshot::channel();
        std::thread::Builder::new()
            .name("permesh-native-io".into())
            .spawn(move || {
                // Keep the bound even when the receiver times out or cancels.
                let _permit = permit;
                if let Err(undelivered) = send.send(operation()) {
                    // A cancelled receiver no longer needs the value; drop it here
                    // so any secret-bearing result is cleared immediately.
                    drop(undelivered);
                }
            })
            .map_err(|_| AppError::new(5, "Cannot start native I/O worker"))?;
        receive
            .await
            .map_err(|_| AppError::new(5, "Native I/O worker stopped unexpectedly"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn cancelled_waiters_do_not_release_active_worker_capacity() {
        let pool = BlockingPool::new();
        let mut releases = Vec::new();
        let mut jobs = Vec::new();
        for _ in 0..4 {
            let (release, wait) = std::sync::mpsc::channel::<()>();
            let (started, ready) = tokio::sync::oneshot::channel();
            releases.push(release);
            let worker = pool.clone();
            jobs.push(tokio::spawn(async move {
                worker
                    .run(move || {
                        let _ = started.send(());
                        let _ = wait.recv();
                    })
                    .await
            }));
            assert!(ready.await.is_ok());
        }
        for job in jobs {
            job.abort();
            let _ = job.await;
        }
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(25), pool.run(|| ()))
                .await
                .is_err()
        );
        for release in releases {
            assert!(release.send(()).is_ok());
        }
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(2), pool.run(|| 42))
                .await
                .is_ok_and(|result| result.is_ok_and(|value| value == 42))
        );
    }
}
