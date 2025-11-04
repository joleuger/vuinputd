# 📦 vinput Build & Install Guide

## 🔹 Prerequisites

* Rust toolchain (recommended: install via [rustup](https://rustup.rs))
* Linux with `build-essential`, `libc6-dev`, `libfuse3-dev`, `libudev-dev` and `pkg-config` installed (for cuse/udev access)

---

## 🔹 Build Everything (Workspace Build)

Clone the repo:

```bash
git clone https://github.com/joleuger/vuinputd.git
cd vuinputd
```

Build all crates (daemon, forwarder, announce, common):

```bash
apt-get install build-essential libc6-dev libfuse3-dev pkg-config fuse3 libudev-dev 

cargo build --release
```
Note: If the system default compiler for C is clang, then `apt-get install libclang-dev` might be necessary as well.

Binaries will be located under:

```
target/release/vuinputd (the daemon itself)
target/release/mouse-advanced (for testing, fakes a mouse device)
target/release/keyboard-advanced (for testing, fakes a keyboard device)
```


---

## 🔹 Install guide

As root on host:
```
cp target/release/vuinputd /usr/local/bin
cp vuinputd/udev/90-vuinputd-protect.rules /etc/udev/rules.d
cp vuinputd/udev/90-vuinputd-protect.rules /etc/udev/rules.d
cp vuinputd/udev/90-vuinputd.hwdb /etc/udev/rules.d/hwdb.d/
cp vuinputd/systemd/vuinputd.service /etc/systemd/system/
systemd-hwdb update
udevadm control --reload
systemctl daemon-reload
systemctl enable --now vuinputd
```