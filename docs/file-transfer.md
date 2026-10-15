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

## Receive a guest file

Run the matching agent in the guest user session with an existing, dedicated outbox:

```text
freelane-guest-agent --export-root <existing-outbox-directory>
```

`--transfer-root` (inbox) and `--export-root` (outbox) are independent and disabled by default. Put the file to export directly in the outbox. In the Display tab, enter its filename under **File name in guest outbox**, click **Receive file from guest**, and choose an existing host destination folder, including an external drive. The CLI equivalent is:

```text
freelane file-receive vm.toml "Result.txt" /path/to/host-folder
```

Requires advertised `file_export` protocol v11; older agents continue supporting their existing features. Only a single portable filename is accepted: no paths, subdirectories or directory browsing. Each session reads one regular file through its opened handle, in chunks up to 16 KiB, with a 64 GiB per-file transfer bound (not a virtual-disk limit). The agent hashes each bounded read and returns the checksum on Finish after checking source size/modification time. Host publication requires the announced size and matching final SHA-256; changed bytes, malformed responses, cancellation and destination conflicts leave no completed output. Existing files, including dangling symlinks, are preserved. The temporary host file is private and created in the selected folder; that local folder must be trusted against other processes replacing temporary entries. This is an explicit copy, not a live mount or conflict-aware synchronization. Do not edit a source while transferring; this does not create an atomic filesystem snapshot.

One active read per agent, a five-minute idle expiry and channel disconnect revoke its token. No automatic retries, execution, deletion of guest source files or automatic opening of received files. Startup installation currently starts the agent without an outbox; stop that agent and run the command above to opt in for the session. Native Mac/Windows ARM interactive acceptance remains pending.
