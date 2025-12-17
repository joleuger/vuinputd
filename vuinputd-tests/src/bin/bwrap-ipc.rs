// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use core::panic;
use std::{time::Duration};

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
        println!("child received continue");
        ipc.send(b"ok").unwrap();
    } else {
        ipc.send(b"nok").unwrap();
        println!("child received {}",incoming_str);
        panic!("expected ipc message to be 'continue'");
    }
}
