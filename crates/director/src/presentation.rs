use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Presentation {
    /// Best-effort foreground preservation and physical input suppression on Windows.
    Automated,
    /// Best-effort launch-time foreground preservation for later human inspection on Windows.
    Deferred,
    /// Ordinary visible editor launch behavior.
    Interactive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PresentationOutcome {
    /// The requested best-effort Windows operations completed.
    Applied,
    /// Ordinary interactive launch behavior was selected explicitly.
    Interactive,
    /// The game launched, but one or more requested protections could not be applied.
    Degraded,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub struct PresentationResult {
    pub mode: Presentation,
    pub outcome: PresentationOutcome,
    /// Actionable limitation when outcome is degraded.
    pub reason: Option<String>,
}

impl PresentationResult {
    pub fn interactive() -> Self {
        Self {
            mode: Presentation::Interactive,
            outcome: PresentationOutcome::Interactive,
            reason: None,
        }
    }

    #[cfg(windows)]
    fn applied(mode: Presentation) -> Self {
        Self {
            mode,
            outcome: PresentationOutcome::Applied,
            reason: None,
        }
    }

    #[cfg(windows)]
    fn degraded(mode: Presentation, reason: impl Into<String>) -> Self {
        Self {
            mode,
            outcome: PresentationOutcome::Degraded,
            reason: Some(reason.into()),
        }
    }
}

#[cfg(any(windows, test))]
fn should_restore_foreground(
    current_is_previous: bool,
    physical_input_changed: bool,
    current_is_identified_game_surface: bool,
) -> bool {
    !current_is_previous && !physical_input_changed && current_is_identified_game_surface
}

#[cfg(windows)]
pub use windows::PreparedPresentation;

#[cfg(not(windows))]
pub struct PreparedPresentation;

#[cfg(not(windows))]
impl PreparedPresentation {
    pub async fn finish(self) -> PresentationResult {
        PresentationResult {
            mode: Presentation::Automated,
            outcome: PresentationOutcome::Degraded,
            reason: Some("presentation preparation was unavailable".into()),
        }
    }
}

#[cfg(not(windows))]
pub fn prepare(
    _mode: Presentation,
    _editor_process_id: u32,
) -> Result<PreparedPresentation, String> {
    Err("automated and deferred presentation are supported only on Windows; omit presentation or use interactive to preserve this platform's existing launch behavior".into())
}

#[cfg(windows)]
pub use windows::prepare;

#[cfg(windows)]
mod windows {
    use std::collections::{HashMap, HashSet};
    use std::mem::size_of;
    use std::time::{Duration, Instant};

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, HWND, INVALID_HANDLE_VALUE, RECT};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        EnableWindow, GetLastInputInfo, IsWindowEnabled, LASTINPUTINFO,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, EnumWindows, GA_ROOT, GetAncestor, GetForegroundWindow, GetWindowRect,
        GetWindowThreadProcessId, IsWindow, IsWindowVisible, SetForegroundWindow,
    };

    use super::{Presentation, PresentationResult};

    const WINDOW_WAIT: Duration = Duration::from_secs(3);
    const WINDOW_POLL: Duration = Duration::from_millis(50);

    pub struct PreparedPresentation {
        mode: Presentation,
        editor_process_id: u32,
        previous_foreground: HWND,
        previous_input_tick: u32,
        existing_windows: HashSet<usize>,
    }

    // SAFETY: HWND values are opaque identifiers. The prepared value is consumed
    // by the same request and no pointer is dereferenced on another thread.
    unsafe impl Send for PreparedPresentation {}

    pub fn prepare(
        mode: Presentation,
        editor_process_id: u32,
    ) -> Result<PreparedPresentation, String> {
        if !matches!(mode, Presentation::Automated | Presentation::Deferred) {
            return Err("only automated and deferred presentation require preparation".into());
        }
        if editor_process_id == 0 {
            return Err("the verified editor returned an invalid process identity".into());
        }
        let previous_foreground = unsafe { GetForegroundWindow() };
        if previous_foreground.is_null() || unsafe { IsWindow(previous_foreground) } == 0 {
            return Err("Windows did not expose a foreground window to preserve".into());
        }
        let previous_input_tick = last_input_tick()?;
        let existing_windows = enumerate_windows()?;
        Ok(PreparedPresentation {
            mode,
            editor_process_id,
            previous_foreground,
            previous_input_tick,
            existing_windows,
        })
    }

    impl PreparedPresentation {
        pub async fn finish(self) -> PresentationResult {
            let deadline = Instant::now() + WINDOW_WAIT;
            let mut last_process_error = None;
            while Instant::now() < deadline {
                match descendant_processes(self.editor_process_id) {
                    Ok(descendants) => match find_new_game_window(
                        &self.existing_windows,
                        self.editor_process_id,
                        &descendants,
                    ) {
                        Ok(Some(game_window)) => {
                            return self.apply(game_window, &descendants);
                        }
                        Ok(None) => {}
                        Err(reason) => {
                            return self.degraded_after_launch(reason, &descendants, None);
                        }
                    },
                    Err(reason) => last_process_error = Some(reason),
                }
                tokio::time::sleep(WINDOW_POLL).await;
            }

            let descendants = descendant_processes(self.editor_process_id).unwrap_or_default();
            self.degraded_after_launch(
                last_process_error.unwrap_or_else(|| {
                    "no new visible game window was identified within 3 seconds; the run remains available and should be stopped before choosing a fallback".into()
                }),
                &descendants,
                None,
            )
        }

        fn apply(self, game_window: HWND, descendants: &HashSet<u32>) -> PresentationResult {
            if self.mode == Presentation::Automated {
                unsafe { EnableWindow(game_window, 0) };
                if unsafe { IsWindowEnabled(game_window) } != 0 {
                    return self.degraded_after_launch(
                        "Windows did not disable physical input for the game window",
                        descendants,
                        Some(game_window),
                    );
                }
            }

            match self.restore_foreground_if_launch_surface(descendants, Some(game_window)) {
                Ok(()) => PresentationResult::applied(self.mode),
                Err(reason) => PresentationResult::degraded(self.mode, reason),
            }
        }

        fn degraded_after_launch(
            self,
            reason: impl Into<String>,
            descendants: &HashSet<u32>,
            game_window: Option<HWND>,
        ) -> PresentationResult {
            let mut reason = reason.into();
            if let Err(restore_reason) =
                self.restore_foreground_if_launch_surface(descendants, game_window)
            {
                reason.push_str("; ");
                reason.push_str(&restore_reason);
            }
            PresentationResult::degraded(self.mode, reason)
        }

        fn restore_foreground_if_launch_surface(
            &self,
            descendants: &HashSet<u32>,
            game_window: Option<HWND>,
        ) -> Result<(), String> {
            let current = unsafe { GetForegroundWindow() };
            if current.is_null() {
                return Ok(());
            }
            let physical_input_changed =
                last_input_tick().is_ok_and(|tick| tick != self.previous_input_tick);
            let launch_root = game_window.map(|window| unsafe { GetAncestor(window, GA_ROOT) });
            let current_process = window_process_id(current);
            let launch_surface = Some(current) == game_window
                || launch_root == Some(current)
                || current_process.is_some_and(|pid| descendants.contains(&pid));
            if !super::should_restore_foreground(
                current == self.previous_foreground,
                physical_input_changed,
                launch_surface,
            ) {
                // The user selected another surface (including the editor after
                // fresh physical input), or the previous surface is still active.
                return Ok(());
            }
            if unsafe { IsWindow(self.previous_foreground) } == 0 {
                return Err("the previous foreground window no longer exists".into());
            }
            if unsafe { SetForegroundWindow(self.previous_foreground) } == 0 {
                return Err(format!(
                    "Windows refused to restore the previous foreground: {}",
                    std::io::Error::last_os_error()
                ));
            }
            Ok(())
        }
    }

    fn find_new_game_window(
        existing: &HashSet<usize>,
        editor_process_id: u32,
        descendants: &HashSet<u32>,
    ) -> Result<Option<HWND>, String> {
        let windows = enumerate_windows()?;
        let mut candidates = Vec::new();
        for raw in windows.difference(existing) {
            let window = *raw as HWND;
            if unsafe { IsWindowVisible(window) } == 0 {
                continue;
            }
            if window_process_id(window)
                .is_some_and(|pid| pid != editor_process_id && descendants.contains(&pid))
            {
                candidates.push((window_area(window), window));
            }
        }
        candidates.sort_by_key(|(area, _)| *area);
        Ok(candidates.pop().map(|(_, window)| window))
    }

    fn window_area(window: HWND) -> u64 {
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        if unsafe { GetWindowRect(window, &mut rect) } == 0 {
            return 0;
        }
        let width = i64::from(rect.right).saturating_sub(i64::from(rect.left));
        let height = i64::from(rect.bottom).saturating_sub(i64::from(rect.top));
        width.max(0) as u64 * height.max(0) as u64
    }

    fn last_input_tick() -> Result<u32, String> {
        let mut info = LASTINPUTINFO {
            cbSize: size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if unsafe { GetLastInputInfo(&mut info) } == 0 {
            return Err(format!(
                "Windows could not read the physical-input timestamp: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(info.dwTime)
    }

    fn window_process_id(window: HWND) -> Option<u32> {
        let mut process_id = 0;
        let thread_id = unsafe { GetWindowThreadProcessId(window, &mut process_id) };
        (thread_id != 0 && process_id != 0).then_some(process_id)
    }

    fn enumerate_windows() -> Result<HashSet<usize>, String> {
        struct WindowSet {
            handles: HashSet<usize>,
        }
        unsafe extern "system" fn collect_top(
            window: HWND,
            parameter: isize,
        ) -> windows_sys::core::BOOL {
            let set = unsafe { &mut *(parameter as *mut WindowSet) };
            set.handles.insert(window as usize);
            unsafe { EnumChildWindows(window, Some(collect_child), parameter) };
            1
        }
        unsafe extern "system" fn collect_child(
            window: HWND,
            parameter: isize,
        ) -> windows_sys::core::BOOL {
            let set = unsafe { &mut *(parameter as *mut WindowSet) };
            set.handles.insert(window as usize);
            1
        }

        let mut set = WindowSet {
            handles: HashSet::new(),
        };
        let result =
            unsafe { EnumWindows(Some(collect_top), std::ptr::addr_of_mut!(set) as isize) };
        if result == 0 {
            return Err(format!(
                "Windows could not enumerate desktop windows: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(set.handles)
    }

    fn descendant_processes(root: u32) -> Result<HashSet<u32>, String> {
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(format!(
                "Windows could not enumerate editor child processes: {}",
                std::io::Error::last_os_error()
            ));
        }
        let owned = OwnedHandle(snapshot);
        let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        let mut parents = HashMap::new();
        if unsafe { Process32FirstW(owned.0, &mut entry) } != 0 {
            loop {
                parents.insert(entry.th32ProcessID, entry.th32ParentProcessID);
                if unsafe { Process32NextW(owned.0, &mut entry) } == 0 {
                    break;
                }
            }
        }

        let mut descendants = HashSet::new();
        let mut changed = true;
        while changed {
            changed = false;
            for (&process, &parent) in &parents {
                if process != root
                    && (parent == root || descendants.contains(&parent))
                    && descendants.insert(process)
                {
                    changed = true;
                }
            }
        }
        Ok(descendants)
    }

    struct OwnedHandle(HANDLE);

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presentation_values_are_explicit_and_stable() {
        for (wire, expected) in [
            ("automated", Presentation::Automated),
            ("deferred", Presentation::Deferred),
            ("interactive", Presentation::Interactive),
        ] {
            assert_eq!(
                serde_json::from_str::<Presentation>(&format!("\"{wire}\"")).unwrap(),
                expected
            );
        }
        assert!(serde_json::from_str::<Presentation>("\"default\"").is_err());
    }

    #[test]
    fn degraded_result_keeps_an_actionable_reason_shape() {
        let result = PresentationResult {
            mode: Presentation::Automated,
            outcome: PresentationOutcome::Degraded,
            reason: Some("stop before fallback".into()),
        };
        let value = serde_json::to_value(result).unwrap();
        assert_eq!(value["mode"], "automated");
        assert_eq!(value["outcome"], "degraded");
        assert_eq!(value["reason"], "stop before fallback");
    }

    #[test]
    fn foreground_restore_never_overwrites_new_user_input_or_another_application() {
        assert!(!should_restore_foreground(false, true, true));
        assert!(!should_restore_foreground(false, false, false));
        assert!(!should_restore_foreground(true, false, true));
        assert!(should_restore_foreground(false, false, true));
    }
}
