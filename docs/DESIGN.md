# Design Document

## 1. Introduction

This project provides a safe, general-purpose way to run [Sunshine](https://github.com/LizardByte/Sunshine) and other applications that use `/dev/uinput` **inside containers** — including `systemd-nspawn`, Docker, LXC, Podman, and similar runtimes.

Applications like Sunshine require creating virtual input devices (`/dev/uinput`) for keyboards, mice, and controllers.  
Naively bind-mounting `/dev/uinput` from the host into a container breaks isolation: a container could create devices visible to other containers or even the host, leading to unwanted input injection and security risks.

`vuinputd` introduces a **mediated `/dev/uinput` proxy** that preserves isolation without kernel changes.

---

## 2. Architecture

Normally, applications open `/dev/uinput` directly to create virtual event devices such as `/dev/input/event9`:

```mermaid
sequenceDiagram
uinput apps->>uinput (kernel): open /dev/uinput and setup
create participant eventx
uinput (kernel)->>eventx: create /dev/input/eventx
uinput (kernel)->>libinput/game: announce new device via udev
libinput/game->>eventx: open /dev/input/eventx
```

vuinputd provides a virtual /dev/vuinput implemented via CUSE (Character Device in Userspace).
This device can be bind-mounted into a container as /dev/uinput, so applications operate normally:
```mermaid

sequenceDiagram
box transparent Host
participant uinput (kernel)
participant vuinputd
participant vuinput (host)
end

box transparent Container
participant uinput (container)
participant uinput apps
participant eventx
participant libinput/game
end

vuinputd->>vuinput (host): create /dev/vuinput with cuse
uinput apps->>uinput (container): open /dev/uinput and setup
uinput (container)-->vuinput (host): is equal (bind mount)
vuinput (host)->>vuinputd: forward data
vuinputd->>uinput (kernel): forward data
uinput (kernel)->>eventx: create /dev/input/eventx
uinput (kernel)->>vuinputd: announce new device via udev
vuinputd->>libinput/game: announce new device via udev
libinput/game->>eventx: open /dev/input/eventx
```

---

## 3. Design Decisions

### 3.1 Where `/dev/uinput` lives

* **Decision**: Provide a fake `/dev/vuinput` backed by host proxy. This character device can be bind mounted inside containers to `/dev/uinput`.
* **Why**: Prevents containers from creating devices visible system-wide.

### 3.2 Prevent host from using devices

* **Decision**: udev rules strip `ID_INPUT_KEYBOARD` and `ID_INPUT_MOUSE`, set `ID_SEAT=seat_vuinput`.
* **Why**: Ensures devices are invisible to host input subsystems while still available in containers.

### 3.3 udev events in containers

* **Decision**: Proxy forwards udev events into the container via netlink.
* **Why**: Without this, SDL2 and libinput might not recognize devices correctly; with it, containers behave as if devices were created locally.

### 3.4 Where to run the proxy

* **Decision**: Run proxy on host, one instance per container.
* **Why**: Only host can safely access `/dev/uinput` and enforce mediation.

### 3.5 Security trade-off

* **Decision**: Accept that host always sees devices, but enforce rules to stop it consuming them.
* **Why**: Full input namespaces don’t exist in Linux today; mediation is the practical compromise.

### 3.6 Compatibility
* **Runtimes supported:** Works with systemd-nspawn, Docker, LXC, Podman, and other container engines.  
* **Applications supported:** Any program that writes to `/dev/uinput`, including Sunshine, custom input injectors, and game streaming servers.

