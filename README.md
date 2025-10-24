# vuinputd

A minimal **CUSE-based proxy for `/dev/uinput`** that lets unmodified applications (like [Sunshine](https://github.com/LizardByte/Sunshine)) run inside containers while creating virtual input devices safely on the host.

> **Run Sunshine and other uinput-based apps inside containers — with full input isolation and zero kernel patches.**

---

## Overview

Containerizing input-producing software (e.g. Sunshine, Moonlight host replacements, remote desktop servers) improves separation and simplifies deployment.  
However, exposing the host’s `/dev/uinput` directly into a container breaks isolation:

* Containers can create devices visible system-wide or to other containers.  
* Keyboards and mice may attach to host seats or inject input into active host sessions.  

`vuinputd` solves this by introducing a **mediated input stack**:

* A **fake `/dev/uinput`** inside each container.  
* A **host proxy daemon** that safely creates the actual devices via `/dev/uinput`.  
* The proxy **forwards add/remove udev events** into the container so SDL2, Wayland, and libinput see devices natively.  
* **udev rules** tag and isolate devices per container, preventing the host from consuming them.

Applications use `/dev/uinput` unmodified, and the mediation adds **negligible overhead**.

---

## Architecture

* **Container:** The app writes to the fake `/dev/uinput`.  
* **Host Proxy:** Creates real devices on the host (labeled with container identity); forwards add/remove events back into the container.  
* **udev:** Matches devices by identity and prevents the host input stack from attaching.  

This design works with any container runtime — **systemd-nspawn, Docker, LXC, Podman**, and others.

---

## Benefits

* 🎮 **SDL2 & Wayland compatibility:** `vuinputd` ensures compositors and games recognize input devices correctly.  
* 🔒 **Strong isolation:** Containers see only their own devices; the host sees them but ignores them completely.  
* ♻️ **Safe lifecycle:** Devices are removed cleanly when the containerized app stops.  
* 🛠️ **Simple integration:** No kernel patches required — only userspace tools and udev rules.

---

## Documentation

See [docs/BUILD.md](docs/BUILD.md) for a short build and installation guide.  
See [docs/DESIGN.md](docs/DESIGN.md) for a detailed overview of the architecture, design trade-offs, and security considerations.

---

## License

MIT
