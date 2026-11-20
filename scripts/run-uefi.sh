#!/bin/sh
# Boot a UEFI disk image in QEMU using the bundled OVMF firmware.
#
# Usage: ./run-uefi.sh <path-to-uefi.img> [options] [extra qemu args...]
#
# Options:
#   --nvme    Enable NVMe disk controller
#   --ahci    Enable AHCI disk controller
#
# Expects OVMF_CODE.fd and OVMF_VARS.fd to sit next to this script (as shipped
# in the GitHub release), or in the current working directory.

set -eu

if [ $# -lt 1 ]; then
  echo "usage: $0 <path-to-uefi.img> [--nvme] [--ahci] [extra qemu args...]" >&2
  exit 1
fi

NVME=false
AHCI=false
IMAGE=

# Options may come before or after the image; anything after that goes to QEMU.
while [ $# -gt 0 ]; do
  case "$1" in
    --nvme)
      NVME=true
      shift
      ;;
    --ahci)
      AHCI=true
      shift
      ;;
    *)
      if [ -n "$IMAGE" ]; then
        break
      fi
      IMAGE="$1"
      shift
      ;;
  esac
done

if [ -z "$IMAGE" ]; then
  echo "error: missing path to UEFI image." >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

find_firmware() {
  for dir in "$SCRIPT_DIR" "$PWD"; do
    if [ -f "$dir/$1" ]; then
      echo "$dir/$1"
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
VARS_TMP="$(mktemp "${TMPDIR:-/tmp}/ovmf_vars.XXXXXX")"
trap 'rm -f "$VARS_TMP"' EXIT
cp "$OVMF_VARS" "$VARS_TMP"

# Create disk images if they don't exist.
if [ "$AHCI" = true ] && [ ! -f AHCI.img ]; then
  qemu-img create -f raw AHCI.img 1M
fi
if [ "$NVME" = true ] && [ ! -f NVME.img ]; then
  qemu-img create -f raw NVME.img 1M
fi

if [ "$NVME" = true ]; then
  set -- \
    -drive "id=nvme-disk,file=NVME.img,format=raw,if=none" \
    -device "nvme,serial=deadbeef,drive=nvme-disk" \
    "$@"
fi

if [ "$AHCI" = true ]; then
  set -- \
    -drive "id=disk,file=AHCI.img,format=raw,if=none" \
    -device "ich9-ahci,id=ahci" \
    -device "ide-hd,drive=disk,bus=ahci.0" \
    "$@"
fi

# Hardware acceleration if available, falls back to TCG otherwise.
if [ -r /dev/kvm ] && [ -w /dev/kvm ]; then
  set -- -accel kvm -cpu host "$@"
fi

set -- \
  -drive "format=raw,if=pflash,readonly=on,file=$OVMF_CODE" \
  -drive "format=raw,if=pflash,file=$VARS_TMP" \
  -drive "format=raw,file=$IMAGE" \
  -serial stdio \
  -m 512M \
  "$@"

# Not exec'd, so the EXIT trap still cleans up the temp VARS file.
qemu-system-x86_64 "$@"
