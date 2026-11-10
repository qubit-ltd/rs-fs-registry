// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::io;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;
use std::time::Instant;

use crate::support::process_runner::run_with_timeout;

const HELPER: &str = "process_runner_tests::test_process_runner_helper";
const MODE: &str = "QUBIT_PROCESS_RUNNER_MODE";
const HEARTBEAT: &str = "QUBIT_PROCESS_RUNNER_HEARTBEAT";

fn helper(mode: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().expect("current test executable"));
    command.args(["--exact", HELPER, "--nocapture"]);
    command.env(MODE, mode);
    command
}

#[test]
fn test_process_runner_helper() {
    let Ok(mode) = std::env::var(MODE) else {
        return;
    };
    match mode.as_str() {
        "normal" => {
            println!("fixed stdout marker");
            eprintln!("fixed stderr marker");
        }
        "failure" => std::process::exit(23),
        "volume" => {
            use std::io::Write;
            let chunk = [b'x'; 8192];
            for _ in 0..32 {
                std::io::stdout().write_all(&chunk).expect("write stdout");
                std::io::stderr().write_all(&chunk).expect("write stderr");
            }
        }
        "hang" => {
            println!("partial output marker");
            loop {
                sleep(Duration::from_secs(1));
            }
        }
        "grandparent" => {
            let mut child = helper("heartbeat");
            let mut child = child.spawn().expect("spawn heartbeat child");
            std::thread::spawn(move || child.wait().expect("reap heartbeat child"));
            println!("partial output marker");
            loop {
                sleep(Duration::from_secs(1));
            }
        }
        "parent-exits" => {
            let path = std::env::var_os(HEARTBEAT).expect("heartbeat path");
            let mut child = helper("heartbeat");
            let mut child = child.env(HEARTBEAT, &path).spawn().expect("spawn heartbeat child");
            std::thread::spawn(move || child.wait().expect("reap heartbeat child"));
            let deadline = Instant::now() + Duration::from_secs(2);
            while !std::path::Path::new(&path).exists() && Instant::now() < deadline {
                sleep(Duration::from_millis(10));
            }
            assert!(std::path::Path::new(&path).exists(), "heartbeat child started");
        }
        "heartbeat" => {
            let path = std::env::var_os(HEARTBEAT).expect("heartbeat path");
            let mut count = 0_u64;
            loop {
                count += 1;
                std::fs::write(&path, count.to_string()).expect("write heartbeat");
                sleep(Duration::from_millis(20));
            }
        }
        _ => panic!("unknown helper mode"),
    }
}

#[test]
fn test_process_runner_preserves_success_and_nonzero_output() {
    let output =
        run_with_timeout(&mut helper("normal"), Duration::from_secs(2), "normal helper").expect("successful helper");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("fixed stdout marker"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("fixed stderr marker"));

    let output = run_with_timeout(&mut helper("failure"), Duration::from_secs(2), "failure helper")
        .expect("nonzero helper is still an output");
    assert_eq!(output.status.code(), Some(23));
}

#[test]
fn test_process_runner_handles_large_output_without_pipe_deadlock() {
    let output = run_with_timeout(&mut helper("volume"), Duration::from_secs(3), "large output helper")
        .expect("large output helper");
    assert!(output.status.success());
    assert!(output.stdout.len() >= 8192 * 32);
    assert!(output.stderr.len() >= 8192 * 32);
    assert!(
        output
            .stdout
            .windows(8192)
            .any(|bytes| bytes.iter().all(|byte| *byte == b'x'))
    );
    assert!(
        output
            .stderr
            .windows(8192)
            .any(|bytes| bytes.iter().all(|byte| *byte == b'x'))
    );
}

#[test]
fn test_process_runner_times_out_with_context_and_partial_output() {
    let started = Instant::now();
    let error = run_with_timeout(&mut helper("hang"), Duration::from_millis(300), "unique helper context")
        .expect_err("helper should time out");
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    let message = error.to_string();
    assert!(message.contains("unique helper context"), "{message}");
    assert!(message.contains("partial output marker"), "{message}");
    assert!(started.elapsed() < Duration::from_secs(5));
}

fn assert_process_runner_timeout_stops_grandchild() {
    let directory = tempfile::tempdir().expect("temp directory");
    let heartbeat = directory.path().join("heartbeat");
    let mut command = helper("grandparent");
    command.env(HEARTBEAT, &heartbeat);
    let error = run_with_timeout(&mut command, Duration::from_millis(300), "grandchild context")
        .expect_err("helper should time out");
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    let deadline = Instant::now() + Duration::from_secs(2);
    while !heartbeat.exists() && Instant::now() < deadline {
        sleep(Duration::from_millis(10));
    }
    assert!(heartbeat.exists(), "grandchild started");
    let previous = std::fs::read(&heartbeat).expect("heartbeat contents");
    sleep(Duration::from_millis(100));
    let current = std::fs::read(&heartbeat).expect("heartbeat contents");
    assert_eq!(previous, current, "heartbeat stopped after process group cleanup");
}

#[test]
fn test_process_runner_cleans_grandchild_after_parent_exits() {
    let directory = tempfile::tempdir().expect("temp directory");
    let heartbeat = directory.path().join("heartbeat");
    let mut command = helper("parent-exits");
    command.env(HEARTBEAT, &heartbeat);
    let output = run_with_timeout(&mut command, Duration::from_secs(3), "parent exits first")
        .expect("parent exits normally and owned grandchild is cleaned");
    assert!(output.status.success());
    assert!(heartbeat.exists(), "grandchild ran before parent exit");
    let previous = std::fs::read(&heartbeat).expect("heartbeat contents");
    sleep(Duration::from_millis(100));
    let current = std::fs::read(&heartbeat).expect("heartbeat contents");
    assert_eq!(previous, current, "grandchild stopped with the owned process tree");
}

#[cfg(unix)]
#[test]
fn test_process_runner_timeout_stops_grandchild_unix() {
    assert_process_runner_timeout_stops_grandchild();
}

#[cfg(windows)]
#[test]
fn test_process_runner_timeout_stops_grandchild_windows() {
    assert_process_runner_timeout_stops_grandchild();
}
