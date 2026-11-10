// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::fs::File;
use std::io;
use std::io::Read;
use std::process::Child;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;
use std::thread::sleep;
use std::time::Duration;
use std::time::Instant;

use tempfile::NamedTempFile;

const POLL_INTERVAL: Duration = Duration::from_millis(10);
#[cfg(unix)]
const CLEANUP_GRACE: Duration = Duration::from_millis(100);
const CLEANUP_WAIT: Duration = Duration::from_secs(5);
const FINAL_REAP_WAIT: Duration = Duration::from_secs(1);

enum ChildWait {
    Reaped,
    TimedOut,
    Failed(io::Error),
}

fn wait_for_exit_until(deadline: Instant, mut try_wait: impl FnMut() -> io::Result<bool>) -> ChildWait {
    loop {
        match try_wait() {
            Ok(true) => return ChildWait::Reaped,
            Ok(false) => {
                let now = Instant::now();
                if now >= deadline {
                    return ChildWait::TimedOut;
                }
                sleep(POLL_INTERVAL.min(deadline.saturating_duration_since(now)));
            }
            Err(error) => return ChildWait::Failed(error),
        }
    }
}

fn bounded_child_cleanup_with(
    deadline: Instant,
    final_wait: Duration,
    mut try_wait: impl FnMut() -> io::Result<bool>,
    mut kill: impl FnMut() -> io::Result<()>,
) -> String {
    let first_wait = wait_for_exit_until(deadline, &mut try_wait);
    if matches!(first_wait, ChildWait::Reaped) {
        return "child reap: reaped".into();
    }
    let kill_result = kill();
    let final_wait = wait_for_exit_until(Instant::now() + final_wait, try_wait);
    let kill_result = kill_result
        .map(|()| "kill: ok".to_owned())
        .unwrap_or_else(|error| format!("kill failed: {error}"));
    format!(
        "child reap before kill: {}; {kill_result}; final child reap: {}",
        describe_child_wait(first_wait),
        describe_child_wait(final_wait),
    )
}

fn bounded_child_cleanup(child: &mut Child, deadline: Instant, final_wait: Duration) -> String {
    let child = std::cell::RefCell::new(child);
    bounded_child_cleanup_with(
        deadline,
        final_wait,
        || child.borrow_mut().try_wait().map(|status| status.is_some()),
        || child.borrow_mut().kill(),
    )
}

fn describe_child_wait(wait: ChildWait) -> String {
    match wait {
        ChildWait::Reaped => "reaped".into(),
        ChildWait::TimedOut => "not reaped before deadline (status unconfirmed)".into(),
        ChildWait::Failed(error) => format!("try_wait failed: {error}; status unconfirmed"),
    }
}

/// Runs a test-support command with a deadline and retains output on failure.
pub fn run_with_timeout(command: &mut Command, timeout: Duration, context: &str) -> io::Result<Output> {
    let stdout = NamedTempFile::new()?;
    let stderr = NamedTempFile::new()?;
    let stdout_file = stdout.reopen()?;
    let stderr_file = stderr.reopen()?;
    command
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file));
    let description = format!("{command:?}");
    let (mut child, mut process_tree) = spawn_contained(command).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("{context}: failed to start {description}: {error}"),
        )
    })?;
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => sleep(POLL_INTERVAL),
            Ok(None) => {
                let cleanup = terminate_tree(&mut child, &mut process_tree);
                let mut stdout_reader = stdout.reopen()?;
                let mut stderr_reader = stderr.reopen()?;
                let (out, err) = read_output(&mut stdout_reader, &mut stderr_reader)?;
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!(
                        "{context}: timed out after {timeout:?} running {description}; cleanup: {cleanup}; stdout: {}; stderr: {}",
                        String::from_utf8_lossy(&out),
                        String::from_utf8_lossy(&err),
                    ),
                ));
            }
            Err(error) => {
                let cleanup = terminate_tree(&mut child, &mut process_tree);
                return Err(io::Error::new(
                    error.kind(),
                    format!("{context}: wait failed for {description}: {error}; cleanup: {cleanup}"),
                ));
            }
        }
    };
    let cleanup = cleanup_descendants(&mut child, &mut process_tree);
    let mut stdout_reader = stdout.reopen()?;
    let mut stderr_reader = stderr.reopen()?;
    let (out, err) = read_output(&mut stdout_reader, &mut stderr_reader)?;
    if let Err(error) = cleanup {
        return Err(io::Error::other(format!(
            "{context}: process exited with {status}; descendant cleanup failed: {error}; stdout: {}; stderr: {}",
            String::from_utf8_lossy(&out),
            String::from_utf8_lossy(&err),
        )));
    }
    Ok(Output {
        status,
        stdout: out,
        stderr: err,
    })
}

fn read_output(stdout: &mut File, stderr: &mut File) -> io::Result<(Vec<u8>, Vec<u8>)> {
    let mut out = Vec::new();
    let mut err = Vec::new();
    stdout.read_to_end(&mut out)?;
    stderr.read_to_end(&mut err)?;
    Ok((out, err))
}

#[cfg(unix)]
struct ProcessTreeGuard;

#[cfg(unix)]
fn spawn_contained(command: &mut Command) -> io::Result<(Child, ProcessTreeGuard)> {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
    command.spawn().map(|child| (child, ProcessTreeGuard))
}

#[cfg(windows)]
struct ProcessTreeGuard(Option<std::os::windows::io::OwnedHandle>);

#[cfg(unix)]
fn terminate_tree(child: &mut Child, _: &mut ProcessTreeGuard) -> String {
    let group = -(child.id() as i32);
    let deadline = Instant::now() + CLEANUP_WAIT;
    let gentle = signal_group(group, libc::SIGTERM);
    let remaining = deadline.saturating_duration_since(Instant::now());
    sleep(CLEANUP_GRACE.min(remaining));
    let forced = signal_group(group, libc::SIGKILL);
    let reaped = bounded_child_cleanup(child, deadline, FINAL_REAP_WAIT);
    format!(
        "SIGTERM: {}; SIGKILL: {}; {reaped}",
        display_result(&gentle),
        display_result(&forced),
    )
}

#[cfg(windows)]
fn spawn_contained(command: &mut Command) -> io::Result<(Child, ProcessTreeGuard)> {
    use std::os::windows::io::AsRawHandle;
    use std::os::windows::io::FromRawHandle;
    use std::os::windows::process::CommandExt;

    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::CreateToolhelp32Snapshot;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::TH32CS_SNAPTHREAD;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::THREADENTRY32;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::Thread32First;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::Thread32Next;
    use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
    use windows_sys::Win32::System::JobObjects::CreateJobObjectW;
    use windows_sys::Win32::System::JobObjects::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    use windows_sys::Win32::System::JobObjects::JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
    use windows_sys::Win32::System::JobObjects::JobObjectExtendedLimitInformation;
    use windows_sys::Win32::System::JobObjects::SetInformationJobObject;
    use windows_sys::Win32::System::Threading::CREATE_SUSPENDED;
    use windows_sys::Win32::System::Threading::OpenProcess;
    use windows_sys::Win32::System::Threading::OpenThread;
    use windows_sys::Win32::System::Threading::PROCESS_SET_QUOTA;
    use windows_sys::Win32::System::Threading::PROCESS_TERMINATE;
    use windows_sys::Win32::System::Threading::ResumeThread;
    use windows_sys::Win32::System::Threading::THREAD_SUSPEND_RESUME;

    command.creation_flags(CREATE_SUSPENDED);
    let mut child = command.spawn()?;
    // The child is still suspended, so it cannot create descendants before it
    // is assigned to the kill-on-close job.
    // SAFETY: null attributes and name request a private, unnamed job object.
    let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if job.is_null() {
        let error = io::Error::last_os_error();
        let rollback = bounded_child_cleanup(&mut child, Instant::now(), FINAL_REAP_WAIT);
        return Err(io::Error::new(
            error.kind(),
            format!("{error}; suspended child rollback: {rollback}"),
        ));
    }
    // SAFETY: CreateJobObjectW returned a valid owned kernel handle.
    let job = unsafe { std::os::windows::io::OwnedHandle::from_raw_handle(job.cast()) };
    // SAFETY: this C-compatible structure accepts zero for all unused fields.
    let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    // SAFETY: `limits` has the documented layout and remains alive for the call.
    let configured = unsafe {
        SetInformationJobObject(
            job.as_raw_handle().cast(),
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    };
    if configured == 0 {
        let error = io::Error::last_os_error();
        let rollback = bounded_child_cleanup(&mut child, Instant::now(), FINAL_REAP_WAIT);
        return Err(io::Error::new(
            error.kind(),
            format!("{error}; suspended child rollback: {rollback}"),
        ));
    }
    // SAFETY: `child.id()` names the newly spawned suspended process.
    let process: HANDLE = unsafe { OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, child.id()) };
    if process.is_null() {
        let error = io::Error::last_os_error();
        let rollback = bounded_child_cleanup(&mut child, Instant::now(), FINAL_REAP_WAIT);
        return Err(io::Error::new(
            error.kind(),
            format!("{error}; suspended child rollback: {rollback}"),
        ));
    }
    // SAFETY: both handles are live and the child has not resumed yet.
    let assigned = unsafe { AssignProcessToJobObject(job.as_raw_handle().cast(), process) };
    // SAFETY: `process` is an owned handle returned by OpenProcess.
    if unsafe { CloseHandle(process) } == 0 {
        let error = io::Error::last_os_error();
        let rollback = bounded_child_cleanup(&mut child, Instant::now(), FINAL_REAP_WAIT);
        return Err(io::Error::new(
            error.kind(),
            format!("{error}; suspended child rollback: {rollback}"),
        ));
    }
    if assigned == 0 {
        let error = io::Error::last_os_error();
        let rollback = bounded_child_cleanup(&mut child, Instant::now(), FINAL_REAP_WAIT);
        return Err(io::Error::new(
            error.kind(),
            format!("{error}; suspended child rollback: {rollback}"),
        ));
    }

    // SAFETY: this requests a system-wide thread snapshot without pointers.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if snapshot as isize == -1 {
        let error = io::Error::last_os_error();
        drop(job);
        let rollback = bounded_child_cleanup(&mut child, Instant::now(), FINAL_REAP_WAIT);
        return Err(io::Error::new(
            error.kind(),
            format!("{error}; suspended child rollback: {rollback}"),
        ));
    }
    // SAFETY: a valid snapshot handle was returned above.
    let snapshot = unsafe { std::os::windows::io::OwnedHandle::from_raw_handle(snapshot.cast()) };
    let mut entry = THREADENTRY32 {
        dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    let mut thread = std::ptr::null_mut();
    // SAFETY: `entry` is initialized with its required size and remains valid.
    let mut found = unsafe { Thread32First(snapshot.as_raw_handle().cast(), &mut entry) } != 0;
    while found {
        if entry.th32OwnerProcessID == child.id() {
            // SAFETY: the snapshot's thread ID belongs to the suspended child.
            thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
            if !thread.is_null() {
                break;
            }
        }
        // SAFETY: `entry` remains a valid writable THREADENTRY32.
        found = unsafe { Thread32Next(snapshot.as_raw_handle().cast(), &mut entry) } != 0;
    }
    if thread.is_null() {
        let error = io::Error::last_os_error();
        drop(job);
        let rollback = bounded_child_cleanup(&mut child, Instant::now(), FINAL_REAP_WAIT);
        return Err(io::Error::new(
            error.kind(),
            format!("{error}; suspended child rollback: {rollback}"),
        ));
    }
    // SAFETY: OpenThread returned a valid thread handle.
    let thread = unsafe { std::os::windows::io::OwnedHandle::from_raw_handle(thread.cast()) };
    // SAFETY: the primary thread is suspended by CREATE_SUSPENDED and the job
    // assignment has completed, so resuming it cannot escape containment.
    if unsafe { ResumeThread(thread.as_raw_handle().cast()) } == u32::MAX {
        let error = io::Error::last_os_error();
        drop(job);
        let rollback = bounded_child_cleanup(&mut child, Instant::now(), FINAL_REAP_WAIT);
        return Err(io::Error::new(
            error.kind(),
            format!("{error}; suspended child rollback: {rollback}"),
        ));
    }
    Ok((child, ProcessTreeGuard(Some(job))))
}

#[cfg(windows)]
fn terminate_tree(child: &mut Child, guard: &mut ProcessTreeGuard) -> String {
    let deadline = Instant::now() + CLEANUP_WAIT;
    let cleanup = guard.close();
    let reaped = bounded_child_cleanup(child, deadline, FINAL_REAP_WAIT);
    match cleanup {
        Ok(()) => format!("owned process job closed; {reaped}"),
        Err(error) => format!("owned process job close failed: {error}; {reaped}"),
    }
}

#[cfg(unix)]
fn signal_group(group: i32, signal: i32) -> io::Result<()> {
    // SAFETY: the caller supplies the ID of a dedicated child process group.
    let result = unsafe { libc::kill(group, signal) };
    if result == 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(error)
    }
}

#[cfg(unix)]
fn group_exists(group: i32) -> io::Result<bool> {
    // SAFETY: signal zero only checks the dedicated process group's existence.
    if unsafe { libc::kill(group, 0) } == 0 {
        return Ok(true);
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(false)
    } else {
        Err(error)
    }
}

#[cfg(unix)]
fn display_result(result: &io::Result<()>) -> String {
    result
        .as_ref()
        .map(|_| "ok".to_owned())
        .unwrap_or_else(|error| error.to_string())
}

#[cfg(unix)]
fn cleanup_descendants(child: &mut Child, _: &mut ProcessTreeGuard) -> io::Result<()> {
    let group = -(child.id() as i32);
    signal_group(group, libc::SIGTERM)?;
    // If a descendant existed, give it a brief graceful exit before enforcing
    // cleanup. A missing group is success because there is nothing to reap.
    if group_exists(group)? {
        sleep(CLEANUP_GRACE);
        signal_group(group, libc::SIGKILL)?;
    }
    Ok(())
}

#[cfg(windows)]
fn cleanup_descendants(_: &mut Child, guard: &mut ProcessTreeGuard) -> io::Result<()> {
    // Closing the owned job terminates all remaining descendants, even when
    // the original process has exited and its PID is no longer valid.
    guard.close()
}

#[cfg(test)]
mod cleanup_tests {
    use std::io;
    use std::time::Instant;

    use super::ChildWait;
    use super::wait_for_exit_until;

    #[test]
    fn test_wait_for_exit_reports_deadline_and_try_wait_failure() {
        assert!(matches!(
            wait_for_exit_until(Instant::now(), || Ok(false)),
            ChildWait::TimedOut
        ));
        assert!(matches!(
            wait_for_exit_until(Instant::now(), || Err(io::Error::other("try marker"))),
            ChildWait::Failed(error) if error.to_string() == "try marker"
        ));
    }

    #[test]
    fn test_child_cleanup_reports_kill_failure_and_unconfirmed_reap() {
        let diagnostic = super::bounded_child_cleanup_with(
            Instant::now(),
            std::time::Duration::ZERO,
            || Ok(false),
            || Err(io::Error::other("kill marker")),
        );
        assert!(diagnostic.contains("kill failed: kill marker"));
        assert!(diagnostic.contains("status unconfirmed"));
    }
}

#[cfg(windows)]
impl ProcessTreeGuard {
    fn close(&mut self) -> io::Result<()> {
        use std::os::windows::io::FromRawHandle;
        use std::os::windows::io::IntoRawHandle;

        use windows_sys::Win32::Foundation::CloseHandle;

        let Some(handle) = self.0.take() else {
            return Ok(());
        };
        let raw = handle.into_raw_handle();
        // SAFETY: `raw` came from the owned job handle and is closed exactly once.
        if unsafe { CloseHandle(raw.cast()) } != 0 {
            Ok(())
        } else {
            let error = io::Error::last_os_error();
            // SAFETY: CloseHandle reported failure, so ownership is restored for
            // a best-effort retry when this guard is dropped.
            self.0 = Some(unsafe { std::os::windows::io::OwnedHandle::from_raw_handle(raw) });
            Err(error)
        }
    }
}
