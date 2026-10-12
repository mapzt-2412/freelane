# Experimental release packages

FreeLane 0.1.0 packages are development previews. The release profile optimizes compiled Rust code; it does not establish smooth guest performance or everyday reliability. Windows stability and Android software-emulation System UI ANRs remain unresolved. iOS runtime and native Mac guest execution still need verification.

## Mac Apple Silicon

[Verified build and download](https://github.com/puma2k/freelane/actions/runs/38237076778#artifacts): `FreeLane-experimental-macos-arm64`, 6.21 MB, source commit `28e0fd0`. Sign in to GitHub to download the artifact. This run passed build/signature/ARM64/CLI-plan checks and uploaded the ZIP. [Recorded evidence](evidence/macos-release-build.json) distinguishes these checks from native GUI, guest and Simulator execution. Artifacts expire after 30 days.

Use macOS 14 or later on Apple Silicon. The [Experimental release packages workflow](https://github.com/puma2k/freelane/actions/workflows/release.yml) builds the CLI and desktop with the macOS SDK, bundles `FreeLane.app`, ad-hoc signs it, verifies the signatures/ARM64 architecture and executes an offline release CLI plan check. Download its `FreeLane-experimental-macos-arm64` artifact from a **successful** run, then extract the inner ZIP. A workflow definition or push alone is not evidence that its build succeeded. Artifacts expire after 30 days; this workflow does not publish GitHub Releases automatically.

The package contains `FreeLane.app`, CLI, documentation, example configs, source/font notices, dependency metadata, commit identifier and a ZIP SHA-256 file. It does not contain VM disks, Windows, QEMU, swtpm, firmware, Android SDKs, Xcode or guest-agent binaries. Install prerequisites separately following [the README](../README.md). SDK/firmware/config paths must match your Mac; examples contain placeholders.

The app uses local ad-hoc signing, **not Developer ID signing or Apple notarization**. macOS may block a downloaded experimental app. Inspect its origin and signature; use Apple's supported Open Anyway flow if you choose to run it. Do not disable system-wide security controls.

QEMU/SDK tools must be reachable through your shell PATH or configured paths. For a Homebrew Apple Silicon installation, launch from Terminal so the desktop and its child processes receive the tool paths:

```sh
export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"
./FreeLane.app/Contents/MacOS/freelane-desktop /absolute/path/to/vm.toml --lang vi
# Or use a mobile device profile:
./FreeLane.app/Contents/MacOS/freelane-desktop --mobile /absolute/path/to/mobile.toml
# CLI is bundled beside the desktop:
./FreeLane.app/Contents/MacOS/freelane mobile-plan /absolute/path/to/mobile.toml
```

The desktop locates the sibling CLI automatically. Finder's PATH may differ from your Terminal's; a missing QEMU executable is a prerequisite error. MCP remains a source-checkout development workflow using `target/debug/freelane`; this standalone app does not configure MCP clients or replace the source wrapper.

Build the same package yourself on Mac with Xcode, Python3.11+ and the pinned Rust toolchain:

```sh
bash scripts/build-macos-release.sh
```

Outputs go under `target/packages/`, named with version and commit. Existing package directories are refused; move your previous output before rebuilding the same commit. The recorded Mac build passed; native GUI, HVF guest boot and mobile runtime still need testing.

## Linux cloud

An x86_64 Linux release can be built in the retained cloud:

```sh
cargo build --locked --release --workspace
```

This produces Linux executables under `target/release/`; they cannot run on Mac. The experimental cloud archive includes CLI/desktop/host agent, docs, configs and notices, while QEMU and mobile SDKs remain separate. Desktop execution needs graphical libraries/session. Its Linux runtime uses explicit TCG software emulation, without a Mac/HVF performance claim.
# Publishing a preview

Pushing a new `v*-preview.*` tag runs the release workflow. Both native Mac packaging and Windows ARM64 agent packaging must succeed before the publish job verifies their SHA256 files and creates a GitHub prerelease with ZIPs and checksums. Only that job receives `contents: write`. Main-branch builds continue to upload Actions artifacts without creating releases. Release notes are maintained in `docs/release-notes-preview.md`.

