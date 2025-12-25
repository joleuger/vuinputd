#!/bin/sh
set -e

RULES_DIR=/usr/lib/udev/rules.d
HWDB_DIR=/usr/lib/udev/hwdb.d

install -D -m 0644 90-vuinputd-protect.rules \
    "$RULES_DIR/90-vuinputd-protect.rules"

install -D -m 0644 90-vuinputd.hwdb \
    "$HWDB_DIR/90-vuinputd.hwdb"

systemd-hwdb update
udevadm control --reload-rules
udevadm trigger
