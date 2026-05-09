# Usage Guide for NixOS

This guide explains how to set up and use `vuinputd` on NixOS.
For general remarks about `vuinputd` and how it works, please refer to [USAGE.md](USAGE.md).

> **Status:** NixOS is one of the primary target platforms for `vuinputd`. Native packaging
> is planned; for now, the configuration below builds `vuinputd` directly from source as part
> of the NixOS system.

---

## Configurations in the Community

The following community members have shared their NixOS configurations including `vuinputd`:

* [ShaneTRS](https://github.com/ShaneTRS/nixos-config/)
* [griffi-gh](https://github.com/girl-pp-ua/nixos-infra/)
* [Markus328](https://github.com/joleuger/vuinputd/issues/14)

Feel free to open a GitHub issue or pull request to add your own configuration or remarks.

---

## NixOS Configuration

The example below is a self-contained NixOS module that:

- Builds `vuinputd` from source using `rustPlatform.buildRustPackage`
- Installs the required udev rules and hwdb entries
- Runs `vuinputd` as a systemd service with a tmpfs for the container-scoped `/dev/input` tree
- Applies the correct permissions to `/dev/vuinput` after the daemon starts

Add the module to your `configuration.nix` imports and run `nixos-rebuild switch`.

```nix
{ config, pkgs, lib, ... }:
let
  vuinputd = pkgs.rustPlatform.buildRustPackage {
    pname = "vuinputd";
    version = "0.3.2-git";

    buildType = "debug";

    nativeBuildInputs = [
      pkgs.pkg-config
      pkgs.rustPlatform.bindgenHook
    ];

    buildInputs = [ pkgs.udev pkgs.fuse3 ];

    src = pkgs.fetchFromGitHub {
      owner = "joleuger";
      repo = "vuinputd";
      rev = "8c40fdc12005319ea16dceb752a8822abfc6039a";
      hash = "sha256-8Q34B04BngZqRLyixeFq8F1t5wFnk6JpaG3EEbgKRcU=";
    };

    cargoHash = "sha256-nJw9bRh6Yn9g1H5SeoT6zxgZLCqV3AtAs9gMfE+P+CU=";

    # Recent versions of fuse3 expose additional libfuse_* types that bindgen
    # needs to allowlist alongside the standard fuse_* types.
    postPatch = ''
      substituteInPlace cuse-lowlevel/build.rs \
        --replace-fail '.allowlist_type("(?i)^fuse.*")' '.allowlist_type("(?i)^(fuse|libfuse).*")'
    '';

    postInstall = ''
      mkdir -p $out/lib/udev/rules.d
      mkdir $out/lib/udev/hwdb.d
      cp vuinputd/udev/*.rules $out/lib/udev/rules.d/
      cp vuinputd/udev/*.hwdb $out/lib/udev/hwdb.d/
    '';
  };
in
{
  environment.systemPackages = with pkgs; [
    vuinputd
    bubblewrap  # required for running containerized applications via bwrap
  ];

  # Main vuinputd daemon.
  # Before starting, a tmpfs is mounted at /run/vuinputd/vuinput/dev-input.
  # This directory serves as the container-scoped /dev/input tree: input devices
  # created by vuinputd are placed here instead of the host's /dev/input, so
  # that containers see only their own devices.
  systemd.services.vuinputd = {
    enable = true;
    wantedBy = [ "multi-user.target" ];
    unitConfig = {
      Description = "Virtual input (/dev/vuinput) daemon";
    };
    serviceConfig = {
      Type = "exec";
      ExecStartPre = pkgs.writeShellScript "mount-tmpfs-dev-input" ''
        mkdir -p /run/vuinputd/vuinput/dev-input
        ${pkgs.util-linux}/bin/mount -t tmpfs -o rw,dev,nosuid tmpfs /run/vuinputd/vuinput/dev-input
      '';
      ExecStart = "${lib.getExe vuinputd} --major 120 --minor 414795 --placement on-host";
      ExecStopPost = pkgs.writeShellScript "umount-dev-input" ''
        ${pkgs.util-linux}/bin/umount /run/vuinputd/vuinput/dev-input
      '';
      Restart = "on-failure";
      # Required to allow vuinputd to access character devices (uinput, CUSE).
      DeviceAllow = "char-* rwm";
      Environment = [
        "RUST_LOG=debug"
      ];
    };
  };

  # vuinputd creates /dev/vuinput via CUSE. The device initially has restrictive
  # permissions, so a one-shot service applies chmod 666 shortly after startup.
  # A proper udev-based solution is planned to replace this workaround.
  systemd.services.vuinputd-chmod = {
    unitConfig.Description = "Chmod 666 /dev/vuinput";
    wantedBy = [ "vuinputd.service" ];
    after = [ "vuinputd.service" ];
    serviceConfig = {
      ExecStart = pkgs.writeShellScript "chmod-vuinput" ''
        sleep 2 && chmod 666 /dev/vuinput
      '';
    };
  };
}
```

### Key Configuration Notes

**`--major` and `--minor`**
These are the device numbers assigned to the virtual `/dev/uinput` character device exposed
inside the container. The values `120` and `414795` are chosen to avoid conflicts with
real devices on the host. Refer to [USAGE.md](USAGE.md) for details on choosing these values.

**`--placement on-host`**
Tells `vuinputd` to place the resulting `/dev/input/event*` devices on the host side (under
the tmpfs at `/run/vuinputd/vuinput/dev-input`) rather than directly in the host's `/dev/input`.
This is what enables per-container input isolation.

**`DeviceAllow = "char-* rwm"`**
`vuinputd` needs access to `/dev/uinput` (to create real input devices on the host) and to
the CUSE subsystem (to expose the virtual `/dev/uinput` inside containers). Both are character
devices, so this broad allowlist is currently required. Reducing the attack surface here is a
[planned hardening step](https://github.com/joleuger/vuinputd/blob/main/docs/DESIGN.md).

**`--device-policy`**
The `ExecStart` line can be extended with a `--device-policy` flag to control which input
capabilities and events the daemon exposes to containerized applications:

| Policy | Effect |
|---|---|
| `none` | All capabilities allowed; no filtering. Useful for debugging. |
| `mute-sys-rq` | Blocks SysRq key handling. All other input passes through. **(default)** |
| `sanitized` | Keyboards and mice only; filters SysRq and VT-switching combos. Recommended for desktop/streaming workloads. |
| `strict-gamepad` | Gamepad-like devices only; blocks keyboards and mice entirely. |

For example, to use the recommended policy for a Sunshine streaming container:

```nix
ExecStart = "${lib.getExe vuinputd} --major 120 --minor 414795 --placement on-host --device-policy sanitized";
```

See [USAGE.md](USAGE.md) for a full description of each policy.

**The `vuinputd-chmod` service**
The CUSE device `/dev/vuinput` is created by the kernel with root-only permissions. Until
a proper udev rule handles this, a small one-shot service applies `chmod 666` two seconds
after the daemon starts. This is a known rough edge and will be improved.

---

## Running a Containerized Application

Once `vuinputd` is running, start a containerized application by binding the virtual devices
into its namespace. The example below uses `bwrap` (Bubblewrap) as a lightweight container:

```bash
bwrap \
    --unshare-net \
    --ro-bind / / \
    --tmpfs /tmp \
    --tmpfs /run/udev \
    --dev-bind /run/vuinputd/vuinput/dev-input /dev/input \
    --dev-bind /dev/vuinput /dev/uinput \
    <your-application>
```

The two `--dev-bind` flags are the core of the integration:

| Bind | Purpose |
|---|---|
| `/run/vuinputd/vuinput/dev-input` → `/dev/input` | Gives the container its own isolated `/dev/input` tree populated by `vuinputd`. |
| `/dev/vuinput` → `/dev/uinput` | Exposes the CUSE-backed virtual `/dev/uinput` at the standard path the application expects. |

The `--tmpfs /run/udev` flag provides a writable but empty udev runtime directory inside the
sandbox. This is sufficient when using `--placement on-host`, because `vuinputd` forwards udev
events into the container directly. If you switch to `--placement in-container`, replace this
flag with a bind-mount of the actual udev runtime directory instead, and create the required
stubs inside the container:

```bash
mkdir -p /run/udev/data/
touch /run/udev/control
```

For instructions on testing this setup in an isolated VM, see
[Testing vuinputd on NixOS with Incus](https://github.com/joleuger/vuinputd/blob/main/distro-tests/nixos/README.md).

---

## Verifying Operation

To confirm that `vuinputd` and the container integration are working correctly, run the
following checks inside the container (install `libinput-tools` and `evtest` if needed):

```bash
# Watch for device creation and input events
libinput debug-events

# Observe udev announcements in a second terminal
udevadm monitor -p

# Read raw events from the input device
evtest /dev/input/event*
```

Then trigger some input from within the container (e.g. run a test binary or move a virtual
mouse). You should see device creation reported by `libinput` and `udevadm`, and raw event
data in `evtest`. On the host, `journalctl -u vuinputd` should show corresponding log lines
about device creation and event forwarding.

For a more detailed walkthrough with example output, see the [Verifying Operation](USAGE.md#7-verifying-operation)
section in the main usage guide.

---

## Phantom Input Events and VT Handling

On headless NixOS systems (no active graphical session), the Linux kernel's virtual terminal
(VT) layer remains active and continues to process keyboard input. This can cause injected
input forwarded by `vuinputd` to reach `getty` login prompts or trigger kernel hotkeys such
as `Ctrl+Alt+Fn`.

The quickest mitigation is to start `vuinputd` with the `--vt-guard` flag:

```nix
ExecStart = "${lib.getExe vuinputd} --major 120 --minor 414795 --placement on-host --vt-guard";
```

`--vt-guard` switches the active VT into graphics mode via a direct ioctl, which disables
the kernel keyboard handler for that VT without requiring a compositor or DRM device.

For a full discussion of all available approaches (including KMSCON and the experimental
`fallbackdm`), see the [Phantom Input Events](USAGE.md#8-handling-phantom-input-events-caused-by-vts)
section in the main usage guide.

---

## Troubleshooting

If `vuinputd` does not behave as expected, refer to
[DEBUG.md](https://github.com/joleuger/vuinputd/blob/main/docs/DEBUG.md) for general
debugging guidance.

Common NixOS-specific issues:

- **CUSE module not loaded:** NixOS should load `cuse` automatically via udev, but if
  `/dev/vuinput` does not appear after the service starts, run `modprobe cuse` and restart
  the service.
- **`/dev/vuinput` is not accessible:** The `vuinputd-chmod` service applies permissions
  2 seconds after startup. If it fails, check `systemctl status vuinputd-chmod` and apply
  `chmod 666 /dev/vuinput` manually for debugging.
- **Input devices not visible inside the container:** Verify that
  `/run/vuinputd/vuinput/dev-input` is mounted as a tmpfs (`mount | grep vuinputd`) and that
  the `bwrap` `--dev-bind` flags point to the correct paths.
- **Read-only filesystem error from `vuinputd`:** If the daemon logs an error like
  `ReadOnlyFilesystem` when creating a device node, the directory where it tries to write
  (typically `/dev/input` or `/run`) is not writable inside the container. Ensure the
  `--dev-bind` and `--tmpfs` flags in your `bwrap` command cover all paths `vuinputd` writes
  to, or switch to `--placement on-host` so writes happen on the host side instead.

  ```
  Error creating input device /dev/input/event12: Read-only file system
  ```
- **Build failures due to bindgen/fuse3 mismatch:** Ensure the `postPatch` block in the
  derivation is present; it is required for recent versions of `fuse3`.