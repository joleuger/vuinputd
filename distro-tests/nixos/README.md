# Testing vuinputd on NixOS with Incus

This guide walks through setting up a reproducible NixOS test environment for `vuinputd`
using [Incus](https://linuxcontainers.org/incus/), an open-source system container and VM manager.

---

## Complete Script

If you are already familiar with Incus and NixOS, here is the full sequence at a glance.
The sections below explain each step in detail.

```bash
# --- Setup ---
# Download NixOS VM image (only needed once)
incus image copy images:nixos/25.11 local: --alias nixos/25.11 --vm

# Create and start the VM
incus launch local:nixos/25.11 nixos-vm --vm

# Adjust resources (requires a stop/start cycle)
incus stop nixos-vm
incus config set nixos-vm limits.memory 3GiB
incus config set nixos-vm security.secureboot false
incus start nixos-vm

# --- Configuration ---
# Extend the NixOS configuration to include the vuinputd test module
incus exec nixos-vm -- sed -i '/imports = \[/a\    ./vuinputd-test-automation.nix' \
    /etc/nixos/configuration.nix

# Push the test module into the VM
incus file push vuinputd-test-automation.nix nixos-vm/etc/nixos/vuinputd-test-automation.nix

# Apply the configuration (takes a few minutes on first run)
incus exec nixos-vm -- nixos-rebuild switch --max-jobs 1

# --- Run the test ---
incus exec nixos-vm -- bwrap \
    --unshare-net \
    --ro-bind / / \
    --tmpfs /tmp \
    --tmpfs /run/udev \
    --dev-bind /run/vuinputd/vuinput/dev-input /dev/input \
    --dev-bind /dev/vuinput /dev/uinput \
    /run/current-system/sw/bin/test-scenarios basic-keyboard
```

---

## Why Incus?

Testing `vuinputd` requires a full Linux system stack: a running kernel, udev, `/dev/uinput`,
CUSE support, and a container runtime. A plain Docker container or unit test harness is not
sufficient because many of these subsystems only exist in a fully booted environment.

[Incus](https://linuxcontainers.org/incus/) is used here for several reasons:

- **Full VM support:** Incus can launch proper virtual machines (not just containers), which
  gives each test environment its own kernel, udev tree, and device namespace — exactly what
  `vuinputd` needs to operate.
- **Clean image lifecycle:** Incus pulls pre-built NixOS images from the
  [Linux Containers image server](https://images.linuxcontainers.org/), so there is no need
  to build a NixOS ISO or maintain a local image manually.
- **Easy configuration injection:** `incus file push` and `incus exec` allow pushing NixOS
  configuration files into the VM and triggering a `nixos-rebuild switch` without requiring
  SSH or manual setup.
- **Reproducibility:** Each test run can start from a fresh image, ensuring that leftover
  state from a previous run does not affect results.
- **Isolation from the host:** The VM is fully isolated from the host system, so test runs
  cannot accidentally interfere with the host's input devices or udev state.

> **Note:** While `vuinputd` is compatible with other container runtimes such as
> `systemd-nspawn`, `Docker`, `LXC`, and `Podman`, Incus VMs are the recommended environment
> for automated NixOS testing because they provide a fully booted NixOS system with minimal
> setup effort.

---

## Prerequisites

- **Incus** installed and initialized on the host (`incus admin init` completed).
- The `vuinputd-test-automation.nix` NixOS module available in your working directory.
  This module configures `vuinputd` and the test tooling inside the VM.
- Sufficient disk space for the NixOS VM image (roughly 3–4 GiB).

---

## Step-by-Step Guide

### 1. Download the NixOS VM Image

```bash
incus image copy images:nixos/25.11 local: --alias nixos/25.11 --vm
```

This pulls the NixOS 25.11 image from the public Linux Containers image server and stores it
locally under the alias `nixos/25.11`. The `--vm` flag ensures the image is treated as a
full virtual machine image rather than a system container rootfs.

---

### 2. Launch the VM

```bash
incus launch local:nixos/25.11 nixos-vm --vm
```

This creates and starts a new VM instance named `nixos-vm` from the downloaded image.
At this point the VM boots NixOS with its default configuration.

---

### 3. Adjust VM Resources

Stop the VM briefly to apply resource limits before running the NixOS rebuild, which is
memory-intensive:

```bash
incus stop nixos-vm
incus config set nixos-vm limits.memory 3GiB
```

NixOS system builds (`nixos-rebuild switch`) evaluate a large Nix expression tree and compile
Rust code. Without sufficient memory, the build may be killed by the OOM reaper.

Secure Boot must also be disabled because the NixOS kernel modules required by `vuinputd`
(CUSE/FUSE) are not signed for Secure Boot by default:

```bash
incus config set nixos-vm security.secureboot false
```

Then restart the VM:

```bash
incus start nixos-vm
```

---

### 4. Inject the Test Configuration

The NixOS configuration inside the VM needs to be extended to include the `vuinputd` test
module. Commands are run inside the VM via `incus exec`. The `--` separator tells Incus where
its own arguments end and where the command to execute inside the VM begins — everything after
`--` is passed verbatim to the VM shell.

First, append the import to the existing `configuration.nix`:

```bash
incus exec nixos-vm -- sed -i '/imports = \[/a\    ./vuinputd-test-automation.nix' \
    /etc/nixos/configuration.nix
```

This uses `sed` to insert `./vuinputd-test-automation.nix` immediately after the `imports = [`
line in the NixOS configuration, so NixOS will pick it up during the next rebuild.

Next, push the test module file into the VM:

```bash
incus file push vuinputd-test-automation.nix nixos-vm/etc/nixos/vuinputd-test-automation.nix
```

The `vuinputd-test-automation.nix` module is responsible for:
- Installing and enabling `vuinputd` as a systemd service.
- Providing any additional packages needed by the test scenarios (e.g., `bwrap`, `test-scenarios`).
- Configuring udev rules for device isolation.

---

### 5. Apply the NixOS Configuration

Trigger a NixOS system rebuild inside the VM:

```bash
incus exec nixos-vm -- nixos-rebuild switch --max-jobs 1
```

The `--max-jobs 1` flag limits parallel build jobs to avoid exhausting VM memory during
compilation. This step may take several minutes on first run because Nix will fetch and
build all required dependencies.

After the rebuild completes, the VM is running a fully configured NixOS system with
`vuinputd` installed and active.

---

### 6. Run the Test Scenarios

Execute a test scenario inside a restricted sandbox using `bwrap` (Bubblewrap).

[Bubblewrap](https://github.com/containers/bubblewrap) was chosen as the sandboxing tool for
several reasons:

- **Minimal and universally available:** `bwrap` is a small, single binary with no daemon and
  no runtime dependencies beyond the kernel. It is packaged in virtually every Linux
  distribution, so there is no need to install a full container runtime just to run tests.
- **Reuses host binaries directly:** Because `bwrap` can bind-mount the host (or VM) filesystem
  read-only into the sandbox, the test binary and all its dependencies are taken straight from
  the running NixOS system — no separate rootfs or image needs to be prepared.
- **Good proxy for heavier runtimes:** The namespace isolation that `bwrap` provides (mount,
  network, udev) is the same fundamental mechanism used by Docker, Podman, and systemd-nspawn.
  If `vuinputd` works correctly inside a `bwrap` sandbox, the same behavior can be expected
  from more heavyweight container runtimes, making `bwrap` a lightweight but representative
  stand-in for integration testing.



```bash
incus exec nixos-vm -- bwrap \
    --unshare-net \
    --ro-bind / / \
    --tmpfs /tmp \
    --tmpfs /run/udev \
    --dev-bind /run/vuinputd/vuinput/dev-input /dev/input \
    --dev-bind /dev/vuinput /dev/uinput \
    /run/current-system/sw/bin/test-scenarios basic-keyboard
```

#### What this command does

| Flag | Purpose |
|---|---|
| `--unshare-net` | Removes network access from the sandbox, ensuring the test is self-contained. |
| `--ro-bind / /` | Mounts the entire VM filesystem read-only as the sandbox root. |
| `--tmpfs /tmp` | Provides a writable temporary directory. |
| `--tmpfs /run/udev` | Provides a writable udev runtime directory, isolating the sandbox from the VM's real udev socket. |
| `--dev-bind /run/vuinputd/vuinput/dev-input /dev/input` | Exposes the input devices managed by `vuinputd` (the container-scoped `/dev/input` subtree) at the expected path inside the sandbox. |
| `--dev-bind /dev/vuinput /dev/uinput` | Exposes the CUSE-backed virtual `/dev/uinput` device provided by `vuinputd` at the standard `/dev/uinput` path inside the sandbox. |

This sandbox mimics what a containerized application would see: it has access to the virtual
`/dev/uinput` provided by `vuinputd` and the corresponding `/dev/input` event devices, but
cannot reach the host's real uinput or other system resources.

The final argument `basic-keyboard` selects which test scenario to run. Additional scenarios
may be available; refer to the `test-scenarios` binary's help output for the full list.

---

## Cleaning Up

To delete the VM and free disk space after testing:

```bash
incus delete --force nixos-vm
```

To also remove the cached image:

```bash
incus image delete nixos/25.11
```

---

## Troubleshooting

If the test fails or `vuinputd` does not appear to be running inside the VM, refer to
[docs/DEBUG.md](https://github.com/joleuger/vuinputd/blob/main/docs/DEBUG.md) for debugging
strategies applicable to container environments.

Common issues:

- **CUSE not available:** Ensure that the `cuse` kernel module is loaded inside the VM
  (`modprobe cuse`). The test automation module should handle this automatically.
- **`nixos-rebuild` runs out of memory:** Increase the memory limit above 3 GiB or reduce
  parallelism further with `--max-jobs 1 --cores 1`.
- **Secure Boot blocking the kernel module:** Verify that `security.secureboot` is set to
  `false` on the VM instance.
- **`/dev/vuinput` not present:** Check that the `vuinputd` systemd service is active
  (`systemctl status vuinputd`) and that CUSE is loaded.