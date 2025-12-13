use core::panic;
use std::{str::from_utf8_unchecked, time::Duration};

use vuinputd_tests::bwrap::SandboxChildIpc;

fn main() {
    println!("starting bwrap-ipc");
    let ipc = unsafe { SandboxChildIpc::from_fd() };

    let incoming = ipc
        .recv(Some(Duration::from_secs(5)))
        .expect("error receiving input from ipc as child within 5 seconds");
    let incoming_str =
        str::from_utf8(&incoming).expect("message received from ipc is not encoded as utf8");
    if incoming_str == "continue" {
        ipc.send(b"ok").unwrap();
    } else {
        ipc.send(b"nok").unwrap();
        panic!("expected ipc message to be 'continue'");
    }
}
