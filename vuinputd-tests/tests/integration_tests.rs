use std::process::Command;

#[cfg(all(feature = "requires-root", feature = "requires-uinput"))]
#[test]
fn test_keyboard_in_container() {
    let keyboard_in_container = env!("CARGO_BIN_EXE_keyboard-in-container");

    // Run the ns_child helper which unshares namespaces
    let status = Command::new(keyboard_in_container)
        .status()
        .expect("failed to launch ns helper");

    assert!(status.success());
}
