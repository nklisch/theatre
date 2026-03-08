use std::path::Path;

use tokio::sync::Mutex;

use crate::daemon::{DaemonHandle, resolve_daemon_port};
use crate::oneshot::{self, OperationError, OperationResult};

/// Backend selection: daemon → one-shot fallback.
///
/// Tries the persistent headless daemon first; falls back to one-shot
/// subprocess if the daemon is unavailable or fails.
pub struct Backend {
    daemon: Mutex<Option<DaemonHandle>>,
}

impl Backend {
    pub fn new() -> Self {
        Self {
            daemon: Mutex::new(None),
        }
    }

    /// Run an operation via the best available backend.
    ///
    /// Tries daemon first; if daemon fails or is for a different project,
    /// falls back to one-shot. On daemon connection failure, attempts one
    /// respawn before falling back.
    pub async fn run_operation(
        &self,
        godot_bin: &Path,
        project_path: &Path,
        operation: &str,
        params: &serde_json::Value,
    ) -> Result<OperationResult, OperationError> {
        let port = resolve_daemon_port();
        let mut guard = self.daemon.lock().await;

        // Check whether the current daemon can serve this request.
        if let Some(ref mut handle) = *guard {
            if handle.project_path() == project_path && handle.is_alive() {
                match handle.send_operation(operation, params).await {
                    Ok(result) => return Ok(result),
                    Err(e) => {
                        tracing::warn!("daemon send_operation failed ({e}), attempting respawn");
                        // Kill the stale daemon.
                        *guard = None;
                        // Attempt a single respawn.
                        match DaemonHandle::spawn(godot_bin, project_path, port).await {
                            Ok(mut new_handle) => {
                                let result =
                                    new_handle.send_operation(operation, params).await;
                                *guard = Some(new_handle);
                                match result {
                                    Ok(r) => return Ok(r),
                                    Err(e2) => {
                                        tracing::warn!(
                                            "respawned daemon also failed ({e2}), \
                                             falling back to one-shot"
                                        );
                                        *guard = None;
                                    }
                                }
                            }
                            Err(e2) => {
                                tracing::warn!(
                                    "daemon respawn failed ({e2}), falling back to one-shot"
                                );
                            }
                        }
                        // Fall through to one-shot.
                        drop(guard);
                        return oneshot::run_oneshot(godot_bin, project_path, operation, params)
                            .await;
                    }
                }
            } else {
                // Project switched or daemon dead — shut down the old one.
                if let Some(old) = guard.take() {
                    let _ = old.shutdown().await;
                }
            }
        }

        // No daemon running. Try to spawn one.
        match DaemonHandle::spawn(godot_bin, project_path, port).await {
            Ok(mut handle) => {
                let result = handle.send_operation(operation, params).await;
                *guard = Some(handle);
                match result {
                    Ok(r) => return Ok(r),
                    Err(e) => {
                        tracing::warn!(
                            "newly spawned daemon operation failed ({e}), \
                             falling back to one-shot"
                        );
                        *guard = None;
                    }
                }
            }
            Err(e) => {
                tracing::info!("daemon spawn failed ({e}), using one-shot");
            }
        }

        // One-shot fallback — always available.
        oneshot::run_oneshot(godot_bin, project_path, operation, params).await
    }

    /// Shut down any running daemon.
    pub async fn shutdown(&self) {
        let mut guard = self.daemon.lock().await;
        if let Some(handle) = guard.take()
            && let Err(e) = handle.shutdown().await
        {
            tracing::warn!("daemon shutdown error: {e}");
        }
    }
}

impl Default for Backend {
    fn default() -> Self {
        Self::new()
    }
}
