use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::{ExitStatus, Stdio};
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, ChildStderr, ChildStdout, Command};

pub const STDERR_TAIL_LIMIT: usize = 64 * 1024;

/// Build a Godot command. On Windows, the hidden supervisor establishes a Job
/// Object before Godot is spawned, so cancellation cannot race descendant
/// assignment.
pub fn godot_command(godot_bin: &Path) -> io::Result<Command> {
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new(std::env::current_exe()?);
        command.arg("__process-supervisor").arg(godot_bin);
        command
    };

    #[cfg(not(windows))]
    let mut command = Command::new(godot_bin);

    command.kill_on_drop(true);
    Ok(command)
}

/// Child ownership that always requests termination and transfers reaping to
/// the Tokio runtime if an awaited cleanup path is skipped.
pub struct OwnedChild {
    child: Option<Child>,
}

impl OwnedChild {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        command.kill_on_drop(true);
        Ok(Self {
            child: Some(command.spawn()?),
        })
    }

    fn child_mut(&mut self) -> &mut Child {
        self.child.as_mut().expect("child already reaped")
    }

    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child_mut().stdout.take()
    }

    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child_mut().stderr.take()
    }

    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child_mut().try_wait()
    }

    pub async fn wait(&mut self) -> io::Result<ExitStatus> {
        let status = self.child_mut().wait().await?;
        self.child.take();
        Ok(status)
    }

    pub async fn terminate_and_wait(&mut self) -> io::Result<ExitStatus> {
        let child = self.child_mut();
        let _ = child.start_kill();
        let status = child.wait().await?;
        self.child.take();
        Ok(status)
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        let _ = child.start_kill();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _ = child.wait().await;
            });
        }
    }
}

pub async fn read_all(mut reader: impl AsyncRead + Unpin) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await?;
    Ok(bytes)
}

pub fn spawn_stderr_tail(reader: ChildStderr) -> StderrTail {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&bytes);
    let task = tokio::spawn(async move {
        let mut reader = reader;
        let mut chunk = [0_u8; 4096];
        loop {
            match reader.read(&mut chunk).await {
                Ok(0) => break,
                Ok(count) => {
                    let mut tail = captured.lock().expect("stderr tail mutex poisoned");
                    append_tail(&mut tail, &chunk[..count], STDERR_TAIL_LIMIT);
                }
                Err(_) => break,
            }
        }
    });
    StderrTail { bytes, task }
}

fn append_tail(tail: &mut Vec<u8>, bytes: &[u8], limit: usize) {
    tail.extend_from_slice(bytes);
    if tail.len() > limit {
        let excess = tail.len() - limit;
        tail.drain(..excess);
    }
}

pub struct StderrTail {
    bytes: Arc<Mutex<Vec<u8>>>,
    task: tokio::task::JoinHandle<()>,
}

impl StderrTail {
    pub fn snapshot(&self) -> String {
        String::from_utf8_lossy(&self.bytes.lock().expect("stderr tail mutex poisoned"))
            .into_owned()
    }

    pub async fn finish(self) -> String {
        let _ = self.task.await;
        String::from_utf8_lossy(&self.bytes.lock().expect("stderr tail mutex poisoned"))
            .into_owned()
    }
}

/// Entry point for the Windows-only hidden process supervisor.
pub fn run_supervisor(args: impl IntoIterator<Item = OsString>) -> io::Result<i32> {
    let mut args = args.into_iter();
    let godot_bin = args.next().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "missing supervised executable")
    })?;

    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject,
        };
        use windows_sys::Win32::System::Threading::GetCurrentProcess;

        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return Err(io::Error::last_os_error());
        }

        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            std::ptr::addr_of!(limits).cast(),
            std::mem::size_of_val(&limits) as u32,
        ) == 0
        {
            let error = io::Error::last_os_error();
            CloseHandle(job);
            return Err(error);
        }
        if AssignProcessToJobObject(job, GetCurrentProcess()) == 0 {
            let error = io::Error::last_os_error();
            CloseHandle(job);
            return Err(error);
        }

        // Keep the handle open for the supervisor's lifetime. The OS closes it
        // on exit, which terminates every process inherited into the job.
        let _ = job;
    }

    let status = std::process::Command::new(godot_bin)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    Ok(status.code().unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stderr_tail_keeps_only_the_newest_bytes() {
        let mut tail = b"older".to_vec();
        append_tail(&mut tail, b"newest", 6);
        assert_eq!(tail, b"newest");
    }
}
