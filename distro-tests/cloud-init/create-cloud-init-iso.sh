#!/bin/sh
# SPDX-License-Identifier: MIT
set -eu

# How to do it is documented on https://cloudinit.readthedocs.io/en/latest/howto/launch_qemu.html


ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
PREPARED_DIR="${ROOT_DIR}/prepared"
IDENTITY_DIR="${PREPARED_DIR}/identity"
TMP_DIR="${ROOT_DIR}/tmp"

KEY_PRIV="${IDENTITY_DIR}/ssh_ed25519"
KEY_PUB="${IDENTITY_DIR}/ssh_ed25519.pub"

USER_DATA_SRC="${ROOT_DIR}/cloud-init/template/user-data.tmpl"
META_DATA_SRC="${ROOT_DIR}/cloud-init/template/meta-data.tmpl"

mkdir -p ${TMP_DIR}

# --------------------------------------------------------------------
# SSH identity (per user, stable)
# --------------------------------------------------------------------
if [ ! -f "${KEY_PRIV}" ]; then
  echo "[*] Generating SSH keypair"
  ssh-keygen -t ed25519 -N "" -f "${KEY_PRIV}" -C "distro-tests"
else
  echo "[*] Reusing existing SSH key"
fi

SSH_KEY="$(cat "${KEY_PUB}")"

# --------------------------------------------------------------------
# Build user-data with injected SSH key
# --------------------------------------------------------------------
USER_DATA_TMP="${TMP_DIR}/user-data"
META_DATA_TMP="${TMP_DIR}/meta-data"

awk -v key="${SSH_KEY}" '
  /^users:/ { print; users=1; next }
  users && /ssh_authorized_keys:/ {
    print
    print "      - " key
    next
  }
  { print }
' "${USER_DATA_SRC}" > "${USER_DATA_TMP}"

cp "${META_DATA_SRC}" "${META_DATA_TMP}"

# --------------------------------------------------------------------
# Create seed ISO
# --------------------------------------------------------------------
echo "[*] Creating cloud-init seed.iso"
cloud-localds \
  "${PREPARED_DIR}/seed.iso" \
  "${USER_DATA_TMP}" \
  "${META_DATA_TMP}"

echo "[✓] cloud-init ISO ready: ${PREPARED_DIR}/seed.iso"
