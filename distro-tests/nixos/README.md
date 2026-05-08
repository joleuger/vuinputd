
> incus image copy images:nixos/25.11 local: --alias nixos/25.11 --vm
> incus launch local:nixos/25.11 nixos-vm --vm
> incus stop local:nixos/25.11 nixos-vm
> incus config set nixos-vm limits.memory 3GiB
> incus config set nixos-vm security.secureboot false
> incus start nixos-vm

> incus exec nixos-vm sed --  -i '/imports = \[/a\    ./vuinputd-test-automation.nix' /etc/nixos/configuration.nix

> incus file push vuinputd-test-automation.nix nixos-vm/etc/nixos/vuinputd-test-automation.nix

> incus exec nixos-vm nixos-rebuild -- switch --max-jobs 1

Execute the test
> incus exec nixos-vm bwrap -- --unshare-net --ro-bind / / --tmpfs /tmp --tmpfs /run/udev --dev-bind /run/vuinputd/vuinput/dev-input /dev/input --dev-bind  /dev/vuinput /dev/uinput /run/current-system/sw/bin/test-scenarios basic-keyboard