#!/usr/bin/env bash
set -euo pipefail
repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_dir"
rustup target add aarch64-unknown-linux-musl
sysroot="$(rustc --print sysroot)"
rust_host="$(rustc -vV | sed -n 's/^host: //p')"
linker="$sysroot/lib/rustlib/$rust_host/bin/rust-lld"
test -x "$linker"
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER="$linker"
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_RUSTFLAGS="${CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_RUSTFLAGS:-} -C linker-flavor=ld.lld -C target-feature=+crt-static"
cargo build --locked -p freelane-guest-agent --release --target aarch64-unknown-linux-musl
package_dir="$repo_dir/target/guest-agent-linux-arm64"
mkdir -p "$package_dir/notices"
cp target/aarch64-unknown-linux-musl/release/freelane-guest-agent "$package_dir/"
cp LICENSE "$package_dir/notices/FreeLane-MIT.txt"
cp "$sysroot/share/doc/rust/COPYRIGHT.html" "$package_dir/notices/Rust-COPYRIGHT.html"
cp "$sysroot/share/doc/rust/COPYRIGHT-library.html" "$package_dir/notices/Rust-library-COPYRIGHT.html"
echo "Built static Linux ARM64 agent: $package_dir/freelane-guest-agent"
