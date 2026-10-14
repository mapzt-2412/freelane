# P2 file transfer and document opening

Continue the accepted roadmap in the current authorized checkout. Agent transfers are explicit copies, not a live shared mount. Ubuntu 9p remains available.

1. Implement a guest storage transaction using a pinned directory capability, anonymous Linux file / exclusively opened Windows staging, sequential bounded chunks, declared size and streaming SHA-256. Complete by atomic no-replace link of the verified file handle, cancel by removing only owned staging. Test Unicode, large files, checksum/size mismatch, duplicate chunks, cancellation, destination conflicts and directory replacement.
2. Add opt-in protocol v5 transfer begin/chunk/finish/cancel and capability discovery. Default agent root remains disabled. Preserve v1–v4. Bound one active transfer, no mutation retries, no contents in logs.
3. Add host transfer orchestration and asynchronous desktop file selection/progress/cancel, English/Vietnamese.
4. Add explicit document opening only for a verified completed transfer, with literal paths and fixed text editors for txt/csv. Never auto-open executable/script files.
5. Exercise actual agent transport and native Windows build, review, commit/push and publish a verified preview. Actual Mac/Windows interactive acceptance remains deferred.

Expected step 1: focused filesystem regressions initially fail because Transfer is missing, then pass; workspace suite remains green. Expected subsequent steps: protocol and host tests fail before implementation, then native transport succeeds with matching SHA-256 and preservation of conflicting originals.
