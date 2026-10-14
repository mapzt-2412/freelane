# Automatic setup sources

The desktop embeds these public bootstrap/trust assets:

- `crates/desktop/assets/homebrew-install.sh`: unmodified official Homebrew/install `install.sh`, revision `7a3f48c7e498d7df3237fd41c30f4d492857edf4`; SHA256 `5f333bbe53bc490e51e7ccb1df8779b3dd6ee73a1a7379efda216edb08ccb148`. Source: https://github.com/Homebrew/install/blob/7a3f48c7e498d7df3237fd41c30f4d492857edf4/install.sh . Its BSD-2-Clause notice accompanies the package.
- `crates/desktop/assets/ubuntu-cdimage-keyring.gpg`: official Ubuntu `ubuntu-keyring_2026.08.18_all.deb`, `usr/share/keyrings/ubuntu-cdimage-keyring.gpg`; SHA256 `192b3782ba2e00e05b6521371fbe67847efad3fdd1cfb87621882d833c8703fa`. The package's keyring copyright notice is included. The signing keyring is embedded, rather than fetched from an unauthenticated keyserver at runtime.

The selectable catalog uses normal ARM64 live-server ISOs from `https://cdimage.ubuntu.com/releases/26.04/release/` (26.04.1 LTS) and `/releases/24.04.5/release/` (24.04.5 LTS). The app verifies each release's detached manifest signature using only the bundled keyring, then selects exactly one filename and checks ISO SHA256. TLS and artifact verification remain enabled. Homebrew handles signed/checksummed distribution of the host packages; those packages are not vendored into FreeLane.

macOS administrator/developer-tools/automation prompts occur locally. The app neither collects nor stores administrator passwords. Existing Homebrew background jobs have a deadline and an owned process group. A Terminal bootstrap runs separately; the UI tells the user to close its installation tab after cancellation. VM creation does not claim that guest OS installation has completed.
