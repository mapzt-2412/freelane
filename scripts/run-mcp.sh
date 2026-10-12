#!/usr/bin/env bash
set -euo pipefail
repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
# Activate retained cloud tools when present. Native Macs use their normal PATH.
if [ -x /workspace/.freelane-tools/cargo/bin/cargo ]; then
  export CARGO_HOME=/workspace/.freelane-tools/cargo
  export RUSTUP_HOME=/workspace/.freelane-tools/rustup
  export PATH="/workspace/.freelane-tools/bin:$CARGO_HOME/bin:$PATH"
  export FREELANE_TEST_FIRMWARE=/workspace/.freelane-tools/qemu-root/usr/share/AAVMF/AAVMF_CODE.fd
  export FREELANE_TEST_NVRAM=/workspace/.freelane-tools/qemu-root/usr/share/AAVMF/AAVMF_VARS.fd
  export FREELANE_LLVM_MINGW_ROOT=/workspace/.freelane-tools/llvm-mingw
fi
exec python3 -u "$repo_dir/scripts/freelane_mcp.py" "$@"
