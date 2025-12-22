# fallbackdm

> This crate is WIP and has not released any source, yet.

**fallbackdm** is a minimal, headless display manager that exists solely to **own a seat and VT when no graphical session is running**.

It prevents unintended keyboard input from reaching `getty` or the kernel VT layer by registering a proper **greeter session** with `systemd-logind`, activating a VT, and switching it to graphics mode — without starting X11 or Wayland.

This is primarily useful for **kiosk setups, remote desktop systems, or input-virtualization scenarios** where no local user interaction is intended, but correct VT semantics must still be preserved.

---

## Problem Statement

On modern Linux systems:

* Virtual terminals (VTs) still exist and have a kernel keyboard handler
* If **no graphical session is active**, `getty` will attach to a VT
* Input injected via `uinput` or forwarded from remote systems may:

  * Trigger `Ctrl+Alt+Fn`
  * Wake or interfere with `getty`
  * Cause VT switches or text-mode interaction

Graphical compositors avoid this by:

* Registering a session with `systemd-logind`
* Owning a VT
* Switching it to `KD_GRAPHICS`

But when **no compositor or greeter is running**, nothing owns the VT.

**fallbackdm fills exactly this gap.**

---

## What fallbackdm Does

* Registers a **`greeter` session** via PAM + `pam_systemd`
* Acquires a seat using **libseat**
* Activates a VT and switches it to graphics mode
* Keeps the session alive while no real graphical session exists
* Displays nothing and launches no compositor

Once a real display manager or compositor starts, it naturally replaces `fallbackdm`.

---

## What fallbackdm Does *Not* Do

* ❌ No X11
* ❌ No Wayland
* ❌ No greeter UI
* ❌ No input filtering (by design)
* ❌ No Device Ownership Required: Unlike a real compositor, `fallbackdm` does not need to open `/dev/dri/cardX` or `/dev/input/event*` to do its job. It only needs the TTY. This minimizes the attack surface significantly.

It only ensures **correct session, seat, and VT ownership**.

---

## When You Need This

You **do not need fallbackdm** if:

* X11 or Wayland is already running
* A display manager (gdm, sddm, greetd, etc.) is active

You **do need fallbackdm** if:

* The system boots without a graphical stack
* Input devices (especially `uinput`) must not reach `getty`
* You rely on logind-correct VT behavior without a real compositor

---

## Architecture Overview

```
fallbackdm
 ├─ PAM session (class=greeter)
 ├─ pam_systemd
 ├─ libseat
 │   └─ seatd or systemd-logind backend
 └─ VT activation + KD_GRAPHICS
```

This mirrors what real display managers do — just without launching anything graphical.

---

## PAM Configuration

Create `/etc/pam.d/fallbackdm`:

```
session required pam_systemd.so class=greeter
```

This is mandatory. Without it, logind will not track the session.

---

## systemd Service Example

```ini
[Unit]
Description=Fallback Display Manager
After=systemd-user-sessions.service
ConditionPathExists=!/run/graphical-session-active

[Service]
ExecStart=/usr/bin/fallbackdm
PAMName=fallbackdm
Restart=always

[Install]
WantedBy=multi-user.target
```

> The condition is optional and can be replaced with more advanced logic later.

---

## Relationship to Other Projects

* **Display managers (gdm, sddm, greetd)**
  Full login stacks with UI and session spawning.

* **Greeters (gtkgreet, tuigreet)**
  UI components launched *by* a display manager.

* **fallbackdm**
  A *headless*, compatibility-focused DM whose only job is to own the seat.

---

## Future Ideas

* Optional status output on the VT
* Signaling input-forwarding daemons (e.g. `vuinputd`)
* Conditional exit when a real session becomes active

---

## License

MIT