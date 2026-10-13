# Mutable settings import/export

Export CPU/RAM, name, guest type, acceleration/display, network/agent and sharing settings to a versioned JSON file. Disk, firmware, NVRAM, TPM state, backend binary, headless/serial mode and physical USB identity stay with the existing VM. This creates a settings template; full VM backup remains the checkpoint/backup workflow.

```sh
freelane settings-export /absolute/path/to/vm.toml /absolute/path/to/new-settings.json
freelane settings-import /absolute/path/to/vm.toml /absolute/path/to/settings.json
```

Export is private (0600), synchronized and atomically published without replacing any existing destination. The destination parent must be owned by you without group/world write access. Input readers accept regular files only and cap reads at 1 MiB. Unsupported versions, unknown fields, wrong guest type, relative media/share references and invalid resources are rejected.

Import updates an existing VM through the normal backup/lock/conflict-preserving settings writer. The VM must be stopped. The command reports the backup path; restart requirements and source VM identity remain unchanged. Export converts external ISO/share references to absolute paths. Update those references if media or shared folders move; the VM's relative disk/firmware paths continue resolving within its original workspace.

In Settings, **Import settings** loads a preview into the form; **Save changes** applies it. Cancellation or invalid input preserves the draft and configuration. **Export saved settings** exports the persisted configuration, including when the form has unsaved edits. macOS uses native open/save dialogs; Linux requires zenity for these dialogs and supports the CLI otherwise. Physical-Mac dialog acceptance remains pending.

Applying HVF clears the target’s TCG threading override; storage and TPM identity remain unchanged. [Linux UI evidence](evidence/settings-bundles.json) covers preview/save/export using a test-only dialog stub, not native dialog acceptance.
