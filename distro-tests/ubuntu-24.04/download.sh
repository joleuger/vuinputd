#!/bin/sh
# SPDX-License-Identifier: MIT
set -eu

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
PREPARED_IMG_DIR="${ROOT_DIR}/prepared"

IMG_NAME="ubuntu-24.04-noble-base.qcow2"
IMG_PATH="${PREPARED_IMG_DIR}/${IMG_NAME}"

SRC_URL="https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-amd64.img"

mkdir -p "${PREPARED_IMG_DIR}"

if [ -f "${IMG_PATH}" ]; then
  echo "[*] Ubuntu 24.04 base image already present"
  exit 0
fi

echo "[*] Downloading Ubuntu 24.04 cloud image"
curl -L "${SRC_URL}" -o "${IMG_PATH}.tmp"

echo "[*] Converting to qcow2"
qemu-img convert -c -O qcow2 "${IMG_PATH}.tmp" "${IMG_PATH}"

rm -f "${IMG_PATH}.tmp"

echo "[✓] Ubuntu 24.04 base image ready"
