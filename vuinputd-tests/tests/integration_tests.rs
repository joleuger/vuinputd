// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::{process::Command, time::Duration};
use vuinputd_tests::bwrap;

#[cfg(all(feature = "requires-root", feature = "requires-bwrap"))]
#[test]
fn test_bwrap_simple() {
    use std::vec;

    let out = bwrap::BwrapBuilder::new()
        .unshare_all()
        .ro_bind("/", "/")
        .tmpfs("/tmp")
        .die_with_parent()
        .command("/usr/bin/echo",&["test","test","test"])
        .run()
        .unwrap_or_else(|e| panic!("failed to run bwrap!: {e}"));

    println!("Output");
    println!("stdout: {}", str::from_utf8(&out.stdout).unwrap());
    println!("stderr: {}", str::from_utf8(&out.stderr).unwrap());
}

#[cfg(all(feature = "requires-root", feature = "requires-bwrap"))]
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
        .command(bwrap_ipc,&[])
        .run()
        .unwrap_or_else(|e| panic!("failed to run bwrap!: {e}"));

    let result = ipc.recv(Some(Duration::from_secs(5)));

    println!("Output");
    println!("stdout: {}", str::from_utf8(&out.stdout).unwrap());
    println!("stderr: {}", str::from_utf8(&out.stderr).unwrap());

    let result = result.expect("error receiving input from ipc as host within 5 seconds");
    let result_str =
    str::from_utf8(&result).expect("message received from ipc is not encoded as utf8");
    println!("host received {}",result_str);

}

#[cfg(all(feature = "requires-root", feature = "requires-uinput"))]
#[test]
fn test_keyboard_in_container() {
    let keyboard_in_container = env!("CARGO_BIN_EXE_keyboard-in-container");

    let status = Command::new(keyboard_in_container)
        .status()
        .expect("failed to launch keyboard-in-container");

    assert!(status.success());
}
