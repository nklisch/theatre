//! One active project connection for a persistent MCP server.
//!
//! Ordinary MCP calls share the operation lock; selection takes it exclusively.
//! This drains whole handlers (including post-query baseline/config writes), not
//! just TCP requests, before replacing project-owned state.
use std::sync::Arc;

use tokio::sync::{Mutex, OwnedRwLockWriteGuard, RwLock};
use tokio::task::{JoinError, JoinHandle};

use crate::tcp::{SessionState, tcp_client_loop};

#[derive(Default)]
pub(crate) struct ProjectConnection {
    pub operations: Arc<RwLock<()>>,
    task: Mutex<Option<JoinHandle<()>>>,
}

impl ProjectConnection {
    /// Finish teardown before replacing state. The returned exclusive guard also
    /// keeps the selecting caller's status/notice response on the selected target.
    pub async fn connect(
        self: &Arc<Self>,
        state: &Arc<Mutex<SessionState>>,
        port: u16,
        replacement: Option<SessionState>,
        exclusive: OwnedRwLockWriteGuard<()>,
    ) -> Result<OwnedRwLockWriteGuard<()>, JoinError> {
        let connection = self.clone();
        let state = state.clone();
        // Once teardown begins, finish it even if the MCP request is cancelled.
        // Merely aborting the old task is insufficient on a multithread runtime:
        // it can still execute until its next yield. Joining proves it can no
        // longer publish a handshake, apply settings or disconnect the new state.
        tokio::spawn(async move {
            let mut task = connection.task.lock().await;
            if let Some(old) = task.take() {
                old.abort();
                if let Err(error) = old.await
                    && !error.is_cancelled()
                {
                    tracing::warn!("Previous Stage connection task failed: {error}");
                }
            }
            let mut current = state.lock().await;
            if let Some(replacement) = replacement {
                *current = replacement;
            }
            current.port = port;
            *task = Some(tokio::spawn(tcp_client_loop(state.clone(), port)));
            exclusive
        })
        .await
    }
}

impl Drop for ProjectConnection {
    fn drop(&mut self) {
        if let Some(task) = self.task.get_mut().take() {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        sync::Notify,
        time::{Duration, timeout},
    };

    #[tokio::test(flavor = "multi_thread", worker_threads = 3)]
    async fn replacement_joins_old_task_even_when_selecting_request_is_cancelled() {
        let connection = Arc::new(ProjectConnection::default());
        let state = Arc::new(Mutex::new(SessionState {
            project_dir: "old".into(),
            ..Default::default()
        }));
        let paused = Arc::new(Notify::new());
        let (release, resume) = std::sync::mpsc::channel();
        let old_state = state.clone();
        let old_paused = paused.clone();
        let old = tokio::spawn(async move {
            old_state.lock().await.config.token_hard_cap = 123;
            old_paused.notify_one();
            // Deliberately hold an executing worker between state accesses. abort
            // cannot interrupt it, and the next uncontended lock need not yield.
            resume.recv().unwrap();
            old_state.lock().await.project_dir = "late-old-write".into();
        });
        *connection.task.lock().await = Some(old);
        paused.notified().await;

        let selecting_connection = connection.clone();
        let selecting_state = state.clone();
        let started = Arc::new(Notify::new());
        let selecting_started = started.clone();
        let mut request = tokio::spawn(async move {
            let exclusive = selecting_connection.operations.clone().write_owned().await;
            selecting_started.notify_one();
            selecting_connection
                .connect(
                    &selecting_state,
                    0,
                    Some(SessionState {
                        project_dir: "new".into(),
                        ..Default::default()
                    }),
                    exclusive,
                )
                .await
                .unwrap()
        });
        started.notified().await;
        // Joining the blocked old worker must keep selection pending. Release
        // it before asserting so a regression failure cannot hang runtime shutdown.
        let completed_early = timeout(Duration::from_millis(100), &mut request)
            .await
            .is_ok();
        if completed_early {
            release.send(()).unwrap();
            panic!("selection published while the old task was still executing");
        }
        request.abort();
        assert!(request.await.unwrap_err().is_cancelled());
        release.send(()).unwrap();
        let _settled = timeout(Duration::from_secs(2), connection.operations.read())
            .await
            .unwrap();
        let selected = state.lock().await;
        assert_eq!(selected.project_dir, std::path::Path::new("new"));
        assert_eq!(selected.config.token_hard_cap, 5000);
    }
}
