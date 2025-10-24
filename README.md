# vuinputd
A minimal **CUSE-based proxy for `/dev/uinput`** that lets unmodified applications (like [Sunshine](https://github.com/LizardByte/Sunshine)) run inside a container while creating virtual input devices on the host.

## Overview

This project makes it possible to run [Sunshine](https://github.com/LizardByte/Sunshine) inside `systemd-nspawn` containers without breaking input isolation.

Normally, Sunshine creates virtual input devices via `/dev/uinput`. If `/dev/uinput` is simply bind-mounted into a container:

* Devices from one container can leak into another.
* Keyboards and mice may attach to host seats that are attached to a running session.

This project solves that by introducing a **mediated input stack**:

* A **fake `/dev/uinput`** inside the container.
* A daemon that **forward** add/remove events into the container, making SDL2 and Wayland/libinput behave correctly.
* A **host proxy** that safely creates the real devices.
* **udev rules** that tag and isolate devices per-container.

---

## Architecture

* **Container**: Sunshine writes to fake `/dev/uinput`.
* **Host Proxy**: Creates real devices on the host, labeled with container identity Forwards add/remove events into the container, so SDL2 and Wayland see devices natively.
* **udev**: Matches by identity, prevents host use.

---

## Benefits

* 🎮 **SDL2 / Wayland compatibility**: fake-udev ensures compositors and games see device events properly.
* 🔒 **Isolation**: containers only see their own devices; host also sees them, but ignores them completely.
* ♻️ **Lifecycle safety**: devices are removed cleanly when Sunshine stops.
* 🛠️ **Simple integration**: no kernel patches, just userspace tools + udev rules.

---

## Documentation

See [docs/DESIGN.md](docs/DESIGN.md) for detailed architecture, design tradeoffs, and security considerations.

---

## License

MIT
