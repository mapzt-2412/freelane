#!/usr/bin/env bash
set -euo pipefail
# Linux x86_64 cross-build. Pin the compiler archive and its published release digest.
if [ "$(uname -s)" != Linux ] || [ "$(uname -m)" != x86_64 ]; then
  echo 'This script requires Linux x86_64; use the documented MSVC build on Windows.' >&2
  exit 1
fi
repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
compiler_root="${FREELANE_LLVM_MINGW_ROOT:-$repo_dir/target/tools/llvm-mingw}"
if [ ! -x "$compiler_root/bin/aarch64-w64-mingw32-clang" ]; then
  mkdir -p "$compiler_root"
  archive="$(mktemp)"
  trap 'rm -f "$archive"' EXIT
  curl --fail --show-error --silent --location --proto '=https' --proto-redir '=https' \
    https://github.com/mstorsjo/llvm-mingw/releases/download/20261006/llvm-mingw-20261006-ucrt-ubuntu-22.04-x86_64.tar.xz -o "$archive"
  printf '%s  %s\n' 5f9c6ed95b2d4bdb2869a488c5fd5857fbdabcf288a0aa3eb1da43f6a08d8ab4 "$archive" | sha256sum --check
  tar -xJf "$archive" -C "$compiler_root" --strip-components=1
fi
export CARGO_TARGET_AARCH64_PC_WINDOWS_GNULLVM_LINKER="$compiler_root/bin/aarch64-w64-mingw32-clang"
export CARGO_TARGET_AARCH64_PC_WINDOWS_GNULLVM_RUSTFLAGS="${CARGO_TARGET_AARCH64_PC_WINDOWS_GNULLVM_RUSTFLAGS:-} -C target-feature=+crt-static"
cd "$repo_dir"
rustup target add aarch64-pc-windows-gnullvm
cargo build --locked -p freelane-guest-agent --release --target aarch64-pc-windows-gnullvm
"$compiler_root/bin/llvm-readobj" --file-headers --coff-imports target/aarch64-pc-windows-gnullvm/release/freelane-guest-agent.exe
package_dir="$repo_dir/target/guest-agent-arm64"
mkdir -p "$package_dir/notices"
cp target/aarch64-pc-windows-gnullvm/release/freelane-guest-agent.exe "$package_dir/"
cp LICENSE "$package_dir/notices/FreeLane-MIT.txt"
cp "$compiler_root/LICENSE.TXT" "$package_dir/notices/LLVM-MinGW.txt"
cp "$compiler_root"/aarch64-w64-mingw32/share/mingw32/COPYING* "$package_dir/notices/"
sha256sum "$package_dir/freelane-guest-agent.exe"
