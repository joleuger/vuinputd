{ config, pkgs, lib, ... }:
let
  vuinputd = pkgs.rustPlatform.buildRustPackage {
  pname = "vuinputd";
  version = "0.3.2-git";

  buildType = "debug";

  nativeBuildInputs = [
    pkgs.pkg-config
    pkgs.rustPlatform.bindgenHook
    # breakpointHook
  ];

  buildInputs = [pkgs.udev pkgs.fuse3];

  src = pkgs.fetchFromGitHub {
    owner = "joleuger";
    repo = "vuinputd";
    rev = "8c40fdc12005319ea16dceb752a8822abfc6039a";
    hash = "sha256-8Q34B04BngZqRLyixeFq8F1t5wFnk6JpaG3EEbgKRcU=";
  };

  cargoHash = "sha256-nJw9bRh6Yn9g1H5SeoT6zxgZLCqV3AtAs9gMfE+P+CU=";

  # Recent versions of fuse3 can also have libfuse_* types
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
    vim
    pkg-config
    udev
    fuse3
    git
    rustc
    cargo
    bubblewrap # for testing

    vuinputd
  ];

  systemd.services.vuinputd = {
    enable = true;
    wantedBy = ["multi-user.target"];
    unitConfig = {
      Description = "Virtual input (/dev/vuinput) daemon";
    };
    serviceConfig = {
      Type = "exec";
      ExecStartPre = pkgs.writeShellScript "mount-tmpfs-dev-input" ''
        mkdir -p /run/vuinputd/vuinput/dev-input
        ${pkgs.util-linux}/bin/mount -t tmpfs -o rw,dev,nosuid tpmfs /run/vuinputd/vuinput/dev-input
      '';
      ExecStart = "${lib.getExe vuinputd} --major 120 --minor 414795 --placement on-host";
      ExecStopPost = pkgs.writeShellScript "umount-dev-input" ''
        ${pkgs.util-linux}/bin/umount /run/vuinputd/vuinput/dev-input
      '';
      Restart = "on-failure";
      DeviceAllow = "char-* rwm";

      Environment = [
        "RUST_LOG=debug"
      ];
    };
  };
  systemd.services.vuinputd-chmod = {
    unitConfig.Description = "Chmod 666 the /dev/vuinput";
    wantedBy = ["vuinputd.service"];
    after = ["vuinputd.service"];

    serviceConfig = {
      ExecStart = pkgs.writeShellScript "chmod-vuinput" ''
        sleep 2 && chmod 666 /dev/vuinput
      '';
    };
  };
}

