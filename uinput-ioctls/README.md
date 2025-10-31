# uinput-ioctls

**uinput-ioctls** provides Rust bindings and constants for the Linux [`uinput`](https://www.kernel.org/doc/html/latest/input/uinput.html) subsystem's ioctl interface.

It exposes `ioctl_*` helper functions and constants based on the Linux kernel's `uinput.h`, allowing you to interact with virtual input devices (keyboards, mice, gamepads, etc.) through Rust in a low-level but type-safe way.

This crate does not attempt to abstract or validate ioctl usage; it only provides constants and wrappers. Higher-level logic (such as event management or device configuration) should be built on top.

---

## ✨ Features

- Idiomatic Rust wrappers around `uinput` ioctl definitions.
- Uses the [`nix`](https://crates.io/crates/nix) crate for safe `ioctl` macros.
- Includes all `UI_*` constants and corresponding helper functions:
  - `ui_dev_create`, `ui_dev_destroy`
  - `ui_dev_setup`, `ui_abs_setup`
  - `ui_set_evbit`, `ui_set_keybit`, ...
  - `ui_begin_ff_upload`, `ui_end_ff_upload`, etc.

---

## 🧰 Example

[Mouse example](https://github.com/joleuger/vuinputd/blob/main/vuinput-examples/src/bin/mouse-advanced.rs)


[Keyboard example](https://github.com/joleuger/vuinputd/blob/main/vuinput-examples/src/bin/keyboard-advanced.rs)

> ⚠️ Requires Linux and appropriate permissions to access `/dev/uinput`.

---

## 🧩 Related Crates

* [`uinput`](https://crates.io/crates/uinput): High-level abstraction for creating virtual input devices.
* [`nix`](https://crates.io/crates/nix): Provides low-level Unix system call wrappers and `ioctl` macros.

---

## 📜 License

Licensed under the [MIT License](LICENSE).

---

## 👤 Author

**Johannes Leupolz**
[dev@leupolz.eu](mailto:dev@leupolz.eu)
