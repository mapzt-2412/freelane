# P0 Mac acceptance checklist

Use a separate test VM and record Mac model/RAM, macOS version, QEMU/swtpm versions, guest build and source revision. The Linux/cloud evidence does not replace these checks. This checklist is prepared for the Mac testing the user deferred.

1. Download the current preview DMG. Mount, copy FreeLane to Applications and launch it using the supported macOS Open Anyway flow for ad-hoc builds. Confirm Vietnamese text, window sizing and settings.
2. Install supported QEMU/swtpm and obtain matching ARM firmware code/variable templates. Use legitimate ARM64 media; Windows also needs the appropriate secure firmware and TPM configuration. Do not reuse a user's active guest state as a template.
3. Launch from Terminal with installed tool paths until Finder tool discovery is completed:

```sh
export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"
/Applications/FreeLane.app/Contents/MacOS/freelane-desktop
```

4. Create a **new** test workspace using the wizard. Confirm ARM64 media yourself; automatic architecture verification is unfinished. Choose HVF explicitly. Run preflight:

```sh
/Applications/FreeLane.app/Contents/MacOS/freelane doctor-json /absolute/path/to/test-vm/vm.toml
```

A passing preflight does not verify signing entitlements, OS boot, Secure Boot, drivers or performance. Missing/failed checks must be fixed before treating boot as accepted.

5. Start and install Windows/Ubuntu. Record actual progress/readiness; then reboot and confirm guest data persists. Exercise pause/resume and request shutdown; wait for actual exit rather than treating an acknowledgment as completion.
6. Install matching guest drivers and the agent in a normal interactive user account. Verify ping, app launch, logout/login startup, update and uninstall separately from the Audit-session cloud checks.
7. Export a per-app launcher, open it from Finder and pin it in the Dock. Test stopped-VM startup, an already running VM, closing/reopening a viewer and multiple app viewers. Confirm uncertain launches are not replayed.
8. With only the disposable test guest running, test host sleep/wake and reconnect, then verify guest data. Record failures rather than bypassing them.
9. Record boot/app launch timings, idle/load CPU and memory, display/input behavior and all failures. Do not claim speed advantages without comparable workload measurements.

Report each check as passed/failed/not run with the relevant revision and a concise result. Do not share credentials or clipboard/document contents in diagnostic results.
