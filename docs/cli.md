# CLI

Build using `cargo build --release -p voids3-cli`; use `voids3 --help` for current command syntax. The CLI shares the desktop's local state and OS keychain. Close the desktop application before operating on the queue.

```sh
voids3 connections list
voids3 connections test <profile-uuid>
voids3 ls <profile-uuid> assets/
voids3 ls <profile-uuid> assets/ --token <continuation-token>
voids3 buckets <profile-uuid>
voids3 upload <profile-uuid> ./archive.zip backups/archive.zip
voids3 download <profile-uuid> backups/archive.zip ./archive.zip
voids3 presign <profile-uuid> backups/archive.zip --seconds 3600
voids3 cp <profile-uuid> source.txt copies/source.txt
voids3 rm <profile-uuid> copies/source.txt --yes
voids3 sync <profile-uuid> ./assets assets/ --mode bidirectional
voids3 sync <profile-uuid> ./assets assets/ --mode upload --apply-additions
voids3 queue list
voids3 queue resume <transfer-uuid>
voids3 queue cancel <transfer-uuid>
voids3 queue clear
```

`buckets` is optional and needs account-wide listing permission; normal commands do not. `sync` defaults to preview and only copies missing files with `--apply-additions`. Existing pairs remain unresolved conflicts and nothing is deleted. Ctrl+C requests a pause and waits for persistent state; restart using `queue resume`.

To save a new profile, create JSON containing the non-secret Profile fields shown in an exported connection, including a new UUID. Set `VOIDS3_ACCESS_KEY`, `VOIDS3_SECRET_KEY`, and optional `VOIDS3_SESSION_TOKEN` in the process environment, then run `voids3 connections save profile.json`. Credentials are moved into the OS keychain, not the profile JSON. Avoid shell-history commands containing real secrets and clear the temporary environment values afterward.

Presigned URLs printed by the CLI are intentionally sensitive output. Do not redirect them to a public log.
