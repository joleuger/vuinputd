# DEBUG.md

## Debugging vuinputd

vuinputd performs low-level operations such as entering Linux namespaces and creating input device nodes. When something goes wrong, the root cause is often related to container lifetime, namespace visibility, or kernel security mechanisms.

This document describes how to debug common issues.

---

## Enable debug logging

Run vuinputd with debug logging enabled:

```bash
RUST_LOG=debug vuinputd ...
```

This will emit additional diagnostic output, especially around namespace entry and `mknod` execution.

---

## Debugging `mknod` inside a container

Whenever vuinputd is about to execute `mknod` inside a container namespace, it prints a debug message similar to:

```
[2025-12-27T23:45:06Z DEBUG vuinputd::job_engine::job] Executing job: mknod input device in container
[2025-12-27T23:45:06Z DEBUG vuinputd::process_tools] In case you need to debug the system calls, call strace vuinputd --target-namespace /proc/103086/ns --action-base64 eyJhY3Rpb24iOiJta25vZC1kZXZpY2UiLCJwYXRoIjoiL2Rldi9pbnB1dC9ldmVudDEyIiwibWFqb3IiOjEzLCJtaW5vciI6NzZ9
```

The printed command can be used verbatim to trace the exact system calls involved.

---

## Using `strace`

Run the suggested command under `strace`:

```bash
strace vuinputd --target-namespace /proc/103086/ns --action-base64 <BASE64_PAYLOAD>
```

Typical failure output may look like:

```text
strace target/debug/vuinputd --target-namespace /proc/102044/ns --action-base64 eyJhY3Rpb24iOiJta25vZC1kZXZpY2UiLCJwYXRoIjoiL2Rldi9pbnB1dC9ldmVudDEyIiwibWFqb3IiOjEzLCJtaW5vciI6NzZ9
...
statx(AT_FDCWD, "/proc/102044/ns", AT_STATX_SYNC_AS_STAT, STATX_ALL, 0x7ffeb6bea3c0) = -1 ENOENT (No such file or directory)
...
thread 'main' (103610) panicked at vuinputd/src/main.rs:170:84:
called Result::unwrap() on an Err value: the root process of the container whose namespaces we want to enter does not exist anymore
...
+++ exited with 101 +++
```

### Interpretation

In this example, `/proc/102044/ns` no longer exists, which means the container process has already terminated. Entering its namespaces is therefore impossible.

This usually indicates:

* the container exited before vuinputd ran,
* a race between container startup and vuinputd execution,
* or an incorrect PID being passed as `--target-namespace`.

---

## Common causes of permission errors

If `mknod` fails with `EPERM` or similar errors, possible causes include:

* Missing `CAP_MKNOD` in the namespace where vuinputd is running
* seccomp filters blocking `mknod`
* SELinux or AppArmor policies
* eBPF-based LSM policies
* Read-only or improperly mounted `/dev`
* Missing permissions to create devices (systemd actually needs `DeviceAllow=char-* rwm` in service files)

Using `strace` usually makes these issues visible immediately.

---

## `/run/udev` and libinput

libinput expects certain udev runtime files to exist, even if no udev daemon is running inside the container.

In minimal or containerized environments, make sure the following paths exist and are writable:

```bash
mkdir -p /run/udev/data
touch /run/udev/control
```

This is a known libinput behavior in containerized setups and not specific to vuinputd.

---

## When reporting issues

If you open an issue, please include:

* full debug logs (`RUST_LOG=debug`)
* the exact `strace` output, if available
* whether host and container share `/dev/input`
* whether vuinputd runs on the host or inside the container
* relevant security mechanisms (seccomp / SELinux / AppArmor)

This makes it much easier to reproduce and diagnose the problem.