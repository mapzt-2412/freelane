# Reconnect a moved VM

For a moved VM whose original path is missing, click **Locate VM** (**Tìm lại máy ảo**) and select its new `vm.toml`. This replaces only that bookmark and refreshes the library. Open the updated entry separately to inspect runtime status. The CLI equivalent is:

```sh
freelane library-relink /absolute/path/to/library.toml /old/path/vm.toml /new/path/vm.toml
```

The old path must be missing. The replacement must have a valid bounded configuration, the same guest type, and readable nonempty disk/installer files. A replacement already bookmarked at the same canonical path, a busy/corrupt library, invalid configuration or inaccessible original path leaves the library unchanged. Other bookmarks and guest files are preserved. Linux dialogs require zenity; the CLI works without it.

This is explicit bookmark repair, not identity authentication, an automatic file search or a VM migration. Relative guest paths continue resolving beside the moved configuration; repair absolute references manually when their targets have also moved. Firmware, TPM, shared directories, active supervisors and exported app-launcher paths need separate validation. Stop the VM before moving its workspace. Never move a running workspace with this command; it does not move files or reconnect a supervisor from its previous runtime key.

[Portable tests and Linux UI evidence](evidence/library-relink.json). The UI acceptance used a test-only dialog stub. Native Mac dialogs and actual guest boot after relocation remain pending.

Locate VM is available without checking disks during rendering. Availability is checked only by the background repair worker. Accessible existing symlink/lexical aliases are compared by canonical path; inaccessible aliases cannot be verified.
