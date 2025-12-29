# Troubleshooting

This document lists known error codes, their meaning, and how to resolve them.

Error codes are stable identifiers intended to help diagnose problems in
different environments such as bare metal, systemd services, and containers.

---

## How to use this document

1. Locate the error code printed by the application
2. Search for it in this document
3. Follow the diagnostic steps
4. Apply the suggested resolution

Error messages may change over time; error codes do not.

---

## Error Code Index

| Code | Area | Summary |
|------|------|--------|
| VUI-UDEV-001 | udev | udev control socket not reachable |

---

## Error Codes

---

### VUI-UDEV-001 — /run/udev/control/ not available. Keyboard or mouse might be unusable.

**Symptoms**

* No keyboard or mouse usable

**Cause**
This might be a problem when an application that uses libinput has already been started, because libinput only checks the file existance at startup. 

**How to diagnose**

Check in container for file existence:
```sh
ls -l /run/udev/control
```

**Resolution**

* Create /run/udev/data directory and /run/udev/control file during startup. See [USAGE.md](USAGE.md).

---

## Reporting Issues

When reporting an issue, please include:

* The error code(s)
* Full command-line invocation
* Execution environment (host, container, systemd)
* Relevant debug logs (see [DEBUG.md](DEBUG.md))
