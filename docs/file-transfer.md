# File transfer

FreeLane copies explicitly selected files into an opt-in guest inbox through agent protocol v5. This is not a live shared mount or background synchronization. Existing Ubuntu 9p shares remain separate.

Create an inbox in the guest and run the updated agent in your ordinary user account:

```powershell
New-Item -ItemType Directory -Force "$env:USERPROFILE\Documents\FreeLane Inbox"
.\freelane-guest-agent.exe --transfer-root "$env:USERPROFILE\Documents\FreeLane Inbox"
```

```bash
mkdir -p "$HOME/FreeLane Inbox"
./freelane-guest-agent --transfer-root "$HOME/FreeLane Inbox"
```

The destination stays disabled unless explicitly configured. Keep that argument when configuring user login startup. The current Windows installer does not add it automatically.

In **Display**, choose **Send file to guest**, select a file, and use **Cancel transfer** to stop. Optional **Open text document after transfer** opens `.txt`/`.csv` up to 32 MiB through system Notepad on Windows or installed `/usr/bin/gnome-text-editor`/`/usr/bin/gedit` on graphical Linux. Ubuntu Server can receive files without a GUI; document opening requires a graphical editor. PDFs/images and arbitrary associations are not opened in this version.

```bash
freelane file-send vm.toml '/path/Tài liệu mới.txt'
freelane file-send-open vm.toml '/path/notes.txt'
```

Transfers use 16 KiB sequential chunks, a declared size, SHA-256 and publication of the exact verified file handle. A conflicting filename—including an existing symlink—is refused; rename the source or resolve the conflict explicitly. Never delete an original to make a retry succeed. Canceled/incomplete transfers are discarded on session termination; uncertain mutations are not retried automatically. Inactivity is checked on subsequent requests after five minutes. An interrupted host connection may require waiting for expiry or restarting the guest agent before sending again.

One active transfer per agent, maximum 64 GiB per file. Source hashing runs before upload; progress currently starts with upload. The limit bounds this transport, not VM disk capacity. Linux requires anonymous-file/hard-link support (e.g. ext4); Windows requires hard-link support (e.g. NTFS). Unsupported filesystems fail without replacing existing files. Unexpected termination on Windows may retain a private `.freelane-stage-*` file; inspect and clean it only after stopping the agent. Root capabilities keep an inbox rename from redirecting writes.

Portable Linux stdio agent/host CLI acceptance and Windows native CI are separate from real Mac/Windows guest boot and editor acceptance. Normal-user login startup and native Mac testing remain pending.
