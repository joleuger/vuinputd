# Usage Guide

This guide explains how to run applications that use `/dev/uinput` (like [Sunshine](https://github.com/LizardByte/Sunshine)) inside containers using **`vuinputd`**.
You’ll learn how to connect your container to the host’s input proxy, configure permissions, and verify that input devices are visible and functional inside the container.

---

## 1. Overview

`vuinputd` allows unmodified apps that use `/dev/uinput` to run safely inside containers.
It provides each container with a **virtual `/dev/uinput`**, while a **host-side daemon** mediates all access to the real uinput subsystem.

This guide shows how to:

1. Run a container (Docker, systemd-nspawn, or LXC/LXD)
2. Connect it to the host’s virtual `/dev/uinput`
3. Verify that device creation and input forwarding work correctly

---

## 2. Prerequisites

Before continuing, ensure the following:

* `vuinputd` is **installed and running** on the host
  → see [docs/BUILD.md](BUILD.md)
* You have **root access** on the host (required for mounting and device permissions)
* The host kernel supports:

  * `/dev/uinput`
  * FUSE/`CUSE`
* Optional tools for debugging and validation inside the container:

  ```bash
  apt-get install libinput-tools evtest udev tmux
  ```

---

## 3. Quick Start (Docker Example)

This is the simplest way to verify that `vuinputd` works.

### 🖥️ On the Host

1. Install Docker:

   ```bash
   sudo apt-get install docker.io
   ```

2. Start a test container with `vuinputd`’s virtual device mapped in:

   ```bash
   sudo docker run -it \
       --name vuinput-test \
       --device=/dev/vuinput:/dev/uinput \
       --device-cgroup-rule='c 13:* rw' \
       --mount type=bind,src=<path-to-vuinputd-build>,dst=/build \
       ubuntu:noble
   ```

   *(Replace `<path-to-vuinputd-build>` with your actual build directory)*

3. Test the application

Just run those lines in the container.
```bash
# Allow access for any application
chmod 666 /dev/uinput
# Prepare udev stubs so applications relying on libudev work
mkdir -p /run/udev/data/
touch /run/udev/control
# Run the demo application
/build/release/mouse-advanced
```
The `vuinputd` daemon on the host should provide some logs. The following section "Verifying Operation" describes a more elaborate check and also some screenshots.

4. Optional: To reuse the container later:

   ```bash
   sudo docker start -ia vuinput-test
   ```

5. To clean up:

   ```bash
   sudo docker rm vuinput-test
   ```

---

## 4. Runtime-Specific Setup

### 🐳 Docker

(As shown above in Quick Start.)

**Key flags:**

* `--device=/dev/vuinput:/dev/uinput` — mounts the fake uinput device
* `--device-cgroup-rule='c 13:* rw'` — allows access to input devices
* Optional: bind your build directory to `/build` for testing binaries

---

### 🧱 systemd-nspawn

1. Install [mkosi](https://github.com/systemd/mkosi):

   ```bash
   sudo apt-get install mkosi
   ```

2. Create an Ubuntu 24.04 image:

   ```bash
   mkosi -d ubuntu -r noble -t directory ubuntu-dir
   ```

3. Launch a container with `vuinputd` bound:

   ```bash
   /usr/bin/systemd-nspawn \
       -M vuinputtest \
       -D ubuntu-dir \
       --network-veth \
       --system-call-filter="@keyring bpf" \
       --bind=/proc:/run/proc \
       --bind=/sys:/run/sys \
       --bind=/dev/vuinput:/dev/uinput \
       --bind=/dev/dri \
       --property="DeviceAllow=char-drm rw" \
       --property="DeviceAllow=char-input rw" \
       --property="DeviceAllow=/dev/vuinput rw" \
       -b
   ```

---

### 🪶 LXC / LXD

Add the following to your container configuration:

```ini
lxc.cgroup2.devices.allow: c 120:414795 rwm
lxc.mount.entry: /dev/vuinput dev/uinput none bind,optional,create=file
```

Then restart the container.

*(Adjust the major/minor numbers to match `/dev/vuinput` on your host — check with `ls -l /dev/vuinput`. In the current release, 120:414795 is hardcoded. This may change in the future.)*

---

## 5. Inside the Container

Once inside the container shell:

```bash
chmod 666 /dev/uinput
apt-get update

# Optional: install test tools
apt-get install libinput-tools udev evtest tmux

# Prepare udev stubs
mkdir -p /run/udev/data/
touch /run/udev/control
```

---

## 6. Verifying Operation

To test everything, use multiple `tmux` windows for parallel monitoring.

1. Start `libinput` event monitor:

   ```bash
   libinput debug-events
   ```

2. In another window, observe udev events:

   ```bash
   udevadm monitor -p
   ```

3. In a third, run:

   ```bash
   evtest /dev/input/event*
   ```

4. Finally, run the demo binary:

   ```bash
   /build/release/mouse-advanced
   ```

### Expected Results

You should see:

* `libinput` reporting device creation and input events
* `udevadm` announcing a new `/dev/input/event*`
* `evtest` showing input data (e.g. mouse movement)
* `journalctl` on the host showing `vuinputd` logs about device creation and event forwarding

Sample output from `libinput debug-events`:  
<img src="libinput.png" width="640"/>

Sample output from `udevadm monitor -p`:  
<img src="udevadm.png" width="378"/>

Sample output from `mouse-advanced`:  
<img src="mouse.png" width="187"/>

Sample output from `evtest`:  
<img src="evtest.png" width="367"/> 

Sample output from `journalctl` showing vuinputd output:  
<img src="vuinputd.png" width="668"/>  

---

## 7. Troubleshooting

| Symptom                     | Possible Cause                       | Fix                                               |
| --------------------------- | ------------------------------------ | ------------------------------------------------- |
| `/dev/uinput` not found     | `vuinputd` not running               | Start `vuinputd` on host                          |
| Permission denied           | Missing `chmod` or wrong cgroup rule | Run `chmod 666 /dev/uinput` or adjust device rule |
| No events in container      | Missing `/run/udev/control`          | Create dummy udev files (see section 5)           |
| Device appears on host seat | udev rules not isolating             | Add udev rules from vuinputd/udev-folder     |
| Input delayed or missing    | CUSE layer error                     | Check host logs via `journalctl -u vuinputd`      |

---

## 8. Notes and Advanced Topics

* You can safely run **multiple containers**.
* Devices are automatically cleaned up when the container stops.
* Works with **Wayland** and **X11** compositors using libinput.
* For deeper details, see:

  * [docs/DESIGN.md](DESIGN.md)
  * [docs/BUILD.md](BUILD.md)

---

## 9. References

* [mkosi manual](https://github.com/systemd/mkosi/blob/main/mkosi/resources/man/mkosi.1.md)
* [Docker device rules documentation](https://docs.docker.com/engine/reference/run/#device-cgroup-rule)
* [libinput tools](https://wayland.freedesktop.org/libinput/doc/latest/tools.html)