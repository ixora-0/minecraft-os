#!/bin/sh
# Boot a UEFI disk image in QEMU using the bundled OVMF firmware.
#
# Usage: ./run-uefi.sh <path-to-uefi.img> [extra qemu args...]
#
# Expects OVMF_CODE.fd and OVMF_VARS.fd to sit next to this script (as shipped
# in the GitHub release), or in the current working directory.

set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "usage: $0 <path-to-uefi.img> [extra qemu args...]" >&2
  exit 1
fi

IMAGE="$1"
shift

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

find_firmware() {
  local name="$1"
  for dir in "$SCRIPT_DIR" "$PWD"; do
    if [[ -f "$dir/$name" ]]; then
      echo "$dir/$name"
      return 0
    fi
  done
  return 1
}

OVMF_CODE="$(find_firmware OVMF_CODE.fd)" || {
  echo "error: OVMF_CODE.fd not found next to this script or in \$PWD." >&2
  echo "       Download it from the same GitHub release as the .img file." >&2
  exit 1
}
OVMF_VARS="$(find_firmware OVMF_VARS.fd)" || {
  echo "error: OVMF_VARS.fd not found next to this script or in \$PWD." >&2
  exit 1
}

# Copy VARS to a writable temp file (QEMU writes back to it).
VARS_TMP="$(mktemp --suffix=.fd)"
trap 'rm -f "$VARS_TMP"' EXIT
cp "$OVMF_VARS" "$VARS_TMP"

exec qemu-system-x86_64 \
  -drive "format=raw,if=pflash,readonly=on,file=$OVMF_CODE" \
  -drive "format=raw,if=pflash,file=$VARS_TMP" \
  -drive "format=raw,file=$IMAGE" \
  -serial stdio \
  -m 512M \
  "$@"
