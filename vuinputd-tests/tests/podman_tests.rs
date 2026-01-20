// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::time::Duration;
use vuinputd_tests::podman;
use vuinputd_tests::run_vuinputd;

#[cfg(all(feature = "requires-privileges", feature = "requires-podman"))]
#[test]
fn test_podman_simple() {
    let out = podman::PodmanBuilder::new()
        .run_cmd()
        .rm()
        //.detach()
        //.name(&format!("vuinputd-podman-tests"))
        .image("localhost/vuinputd-tests:latest")
        .command(&["/test-ok"])
        .run()
        .unwrap();

    println!("Output");
    println!("stdout: {}", str::from_utf8(&out.stdout).unwrap());
    println!("stderr: {}", str::from_utf8(&out.stderr).unwrap());
}

#[cfg(all(feature = "requires-privileges", feature = "requires-podman"))]
#[test]
fn test_podman_ipc() {
    let (builder, ipc) = podman::PodmanBuilder::new()
        .run_cmd()
        .rm()
        .with_ipc()
        .expect("failed to create IPC");
    let builder = builder
        //.detach()
        //.name(&format!("vuinputd-podman-tests"))
        .image("localhost/vuinputd-tests:latest")
        .command(&["/test-ipc"]);

    // Note that builder.run() will block. Thus, the send needs to happen before the child process blocks
    // the host process.
    ipc.send("continue".as_bytes())
        .unwrap_or_else(|e| panic!("failed to send data via ipc: {e}"));

    let out = builder
        .run()
        .unwrap_or_else(|e| panic!("failed to run podman!: {e}"));

    let result = ipc.recv(Some(Duration::from_secs(5)));

    println!("Output");
    println!("stdout: {}", str::from_utf8(&out.stdout).unwrap());
    println!("stderr: {}", str::from_utf8(&out.stderr).unwrap());

    let result = result.expect("error receiving input from ipc as host within 5 seconds");
    let result_str =
        str::from_utf8(&result).expect("message received from ipc is not encoded as utf8");
    println!("host received {}", result_str);
}

#[cfg(all(
    feature = "requires-privileges",
    feature = "requires-uinput",
    feature = "requires-podman"
))]
#[test]
fn test_keyboard_in_container_with_vuinput() {
    let _guard=run_vuinputd::ensure_vuinputd_running(&[]);

    let (builder, _ipc) = podman::PodmanBuilder::new()
        .run_cmd()
        .rm()
        .with_ipc()
        .expect("failed to create IPC");
    let builder = builder
        //.detach()
        //.name(&format!("vuinputd-podman-tests"))
        .device("/dev/vuinput-test:/dev/uinput")
        .allow_input_devices()
        .image("localhost/vuinputd-tests:latest")
        .command(&["/test-keyboard"]);

    let out = builder
        .run()
        .unwrap_or_else(|e| panic!("failed to run podman!: {e}"));

    println!("Output");
    println!("stdout: {}", str::from_utf8(&out.stdout).unwrap());
    println!("stderr: {}", str::from_utf8(&out.stderr).unwrap());

    assert!(out.status.success());
}
