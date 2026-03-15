use std::path::Path;

use tokio::sync::Mutex;

use crate::daemon::{DaemonHandle, resolve_daemon_port};
use crate::editor::{EditorError, EditorHandle, resolve_editor_port};
use crate::oneshot::{self, OperationError, OperationResult};

/// Backend selection: editor plugin → daemon → one-shot fallback.
///
/// Tries the editor plugin first; if not available, falls back to the
/// persistent headless daemon, then one-shot subprocess.
pub struct Backend {
    editor: Mutex<Option<EditorHandle>>,
    daemon: Mutex<Option<DaemonHandle>>,
}

impl Backend {
    pub fn new() -> Self {
        Self {
            editor: Mutex::new(None),
            daemon: Mutex::new(None),
        }
    }

    /// Run an operation via the best available backend.
    ///
    /// Priority: editor plugin → daemon → one-shot.
    pub async fn run_operation(
        &self,
        godot_bin: &Path,
        project_path: &Path,
        operation: &str,
        params: &serde_json::Value,
    ) -> Result<OperationResult, OperationError> {
        // 1. Try editor plugin
        match self.try_editor(project_path, operation, params).await {
            Ok(result) => return Ok(result),
            Err(EditorError::NotReachable(_)) => {
                // Editor not running — fall through to daemon silently.
            }
            Err(e) => {
                tracing::warn!("editor plugin failed ({e}), falling through to daemon");
            }
        }

        // 2. Try daemon → one-shot (existing logic)
        self.try_daemon_then_oneshot(godot_bin, project_path, operation, params)
            .await
    }

    /// Attempt to run an operation via the editor plugin.
    async fn try_editor(
        &self,
        project_path: &Path,
        operation: &str,
        params: &serde_json::Value,
    ) -> Result<OperationResult, EditorError> {
        let port = resolve_editor_port(project_path);
        let mut guard = self.editor.lock().await;

        // Use cached connection if alive.
        if let Some(ref mut handle) = *guard {
            if handle.is_alive() {
                match handle.send_operation(operation, params).await {
                    Ok(result) => return Ok(result),
                    Err(e) => {
                        tracing::warn!("editor send failed ({e}), reconnecting");
                        *guard = None;
                    }
                }
            } else {
                *guard = None;
            }
        }

        // Try fresh connection.
        let mut handle = EditorHandle::connect(port).await?;
        let result = handle.send_operation(operation, params).await?;
        *guard = Some(handle);
        Ok(result)
    }

    /// Daemon → one-shot fallback logic.
    async fn try_daemon_then_oneshot(
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
                                let result = new_handle.send_operation(operation, params).await;
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

    /// Kill the daemon without shutting down the editor connection.
    ///
    /// Used by `project_reload` to force a fresh daemon respawn on the next
    /// operation, ensuring newly-written `.gd` files and their class names are
    /// visible to Godot's resource loader.
    pub async fn kill_daemon(&self) {
        let mut guard = self.daemon.lock().await;
        if let Some(handle) = guard.take()
            && let Err(e) = handle.shutdown().await
        {
            tracing::warn!("daemon kill error: {e}");
        }
    }

    /// Shut down any running editor connection and daemon.
    pub async fn shutdown(&self) {
        // Disconnect editor (drop the handle — no process to kill).
        {
            let mut guard = self.editor.lock().await;
            *guard = None;
        }
        // Shut down daemon.
        {
            let mut guard = self.daemon.lock().await;
            if let Some(handle) = guard.take()
                && let Err(e) = handle.shutdown().await
            {
                tracing::warn!("daemon shutdown error: {e}");
            }
        }
    }
}

impl Default for Backend {
    fn default() -> Self {
        Self::new()
    }
}
