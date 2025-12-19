// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::{process::Command, time::Duration};
use vuinputd_tests::bwrap;
use vuinputd_tests::run_vuinputd;

#[cfg(all(feature = "requires-privileges", feature = "requires-bwrap"))]
#[test]
fn test_bwrap_simple() {
    let out = bwrap::BwrapBuilder::new()
        .unshare_all()
        .ro_bind("/", "/")
        .tmpfs("/tmp")
        .die_with_parent()
        .command("/usr/bin/echo", &["test", "test", "test"])
        .run()
        .unwrap_or_else(|e| panic!("failed to run bwrap!: {e}"));

    println!("Output");
    println!("stdout: {}", str::from_utf8(&out.stdout).unwrap());
    println!("stderr: {}", str::from_utf8(&out.stderr).unwrap());
}

#[cfg(all(feature = "requires-privileges", feature = "requires-bwrap"))]
#[test]
fn test_bwrap_ipc() {
    let bwrap_ipc = env!("CARGO_BIN_EXE_bwrap-ipc");

    let (builder, ipc) = bwrap::BwrapBuilder::new()
        .unshare_all()
        .ro_bind("/", "/")
        .tmpfs("/tmp")
        .die_with_parent()
        .with_ipc()
        .expect("failed to create IPC");

    // Note that builder.run() will block. Thus, the send needs to happen before the child process blocks
    // the host process.
    ipc.send("continue".as_bytes())
        .unwrap_or_else(|e| panic!("failed to send data via ipc: {e}"));

    let out = builder
        .command(bwrap_ipc, &[])
        .run()
        .unwrap_or_else(|e| panic!("failed to run bwrap!: {e}"));

    let result = ipc.recv(Some(Duration::from_secs(5)));

    println!("Output");
    println!("stdout: {}", str::from_utf8(&out.stdout).unwrap());
    println!("stderr: {}", str::from_utf8(&out.stderr).unwrap());

    let result = result.expect("error receiving input from ipc as host within 5 seconds");
    let result_str =
        str::from_utf8(&result).expect("message received from ipc is not encoded as utf8");
    println!("host received {}", result_str);
}

#[cfg(all(feature = "requires-privileges", feature = "requires-bwrap"))]
#[test]
fn test_list_sys_in_container() {
    let out = bwrap::BwrapBuilder::new()
        .unshare_all()
        .ro_bind("/", "/")
        .tmpfs("/tmp")
        .die_with_parent()
        .command(
            "/usr/bin/ls",
            &["-lh", "/sys/devices/virtual/input/input235"],
        )
        .run()
        .unwrap_or_else(|e| panic!("failed to run bwrap!: {e}"));

    println!("Output");
    println!("stdout: {}", str::from_utf8(&out.stdout).unwrap());
    println!("stderr: {}", str::from_utf8(&out.stderr).unwrap());
}

#[cfg(all(feature = "requires-privileges", feature = "requires-uinput"))]
#[test]
fn test_keyboard_on_host() {
    let test_keyboard = env!("CARGO_BIN_EXE_test-keyboard");

    let status = Command::new(test_keyboard)
        .status()
        .expect("failed to launch keyboard-in-container");

    assert!(status.success());
}

#[cfg(all(
    feature = "requires-privileges",
    feature = "requires-uinput",
    feature = "requires-bwrap"
))]
#[test]
fn test_keyboard_in_container_with_uinput() {
    let test_keyboard = env!("CARGO_BIN_EXE_test-keyboard");

    let (builder, _ipc) = bwrap::BwrapBuilder::new()
        .unshare_net()
        .ro_bind("/", "/")
        .tmpfs("/tmp")
        .dev_bind("/dev/uinput", "/dev/uinput")
        .dev_bind("/dev/input", "/dev/input")
        .die_with_parent()
        .with_ipc()
        .expect("failed to create IPC");

    let out = builder
        .command(test_keyboard, &["--ipc"])
        .run()
        .unwrap_or_else(|e| panic!("failed to run bwrap!: {e}"));

    println!("Output");
    println!("stdout: {}", str::from_utf8(&out.stdout).unwrap());
    println!("stderr: {}", str::from_utf8(&out.stderr).unwrap());

    assert!(out.status.success());
}

#[cfg(all(
    feature = "requires-privileges",
    feature = "requires-uinput",
    feature = "requires-bwrap"
))]
#[test]
fn test_keyboard_in_container_with_vuinput() {
    run_vuinputd::ensure_vuinputd_running();

    let test_keyboard = env!("CARGO_BIN_EXE_test-keyboard");

    let (builder, _ipc) = bwrap::BwrapBuilder::new()
        .unshare_net()
        .ro_bind("/", "/")
        .tmpfs("/tmp")
        // dev needs to be writable for the new devices
        .dev()
        // run needs to be writable for the udev devices
        .tmpfs("/run")
        .dev_bind("/dev/vuinput-test", "/dev/uinput")
        .die_with_parent()
        .with_ipc()
        .expect("failed to create IPC");

    let out = builder
        .command(test_keyboard, &["--ipc"])
        .run()
        .unwrap_or_else(|e| panic!("failed to run bwrap!: {e}"));

    println!("Output");
    println!("stdout: {}", str::from_utf8(&out.stdout).unwrap());
    println!("stderr: {}", str::from_utf8(&out.stderr).unwrap());

    assert!(out.status.success());
}
