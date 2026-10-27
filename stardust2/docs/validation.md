# Local validation: 2026-10-27

The finished Stardust source passed the platform scripts with the pinned Rust
1.98.1 toolchain and cached, locked dependencies. No tests were ignored.

| Check | Windows x86_64 MSVC | Linux x86_64 GNU, Ubuntu WSL2 |
| --- | --- | --- |
| Unit regressions | 224 passed | 228 passed |
| Combined command dispatch | 12 passed | 12 passed |
| Combined generator/XOR workflows | 31 passed | 31 passed |
| Generator subprocess tests | 8 passed | 8 passed |
| XOR/in-place subprocess tests | 53 passed | 52 passed |
| Total | **328 passed** | **331 passed** |
| `cargo fmt --all -- --check` | Passed | Passed |
| `cargo clippy --all-targets --locked -- -D warnings` | Passed | Passed |
| Locked release build | Passed | Passed |

The 43 dispatch/workflow tests are new. Both inherited regression collections
are retained, including original known-answer outputs for all five commands.
Linux fixtures ran on native Linux storage, not the Windows-mounted workspace.
Linux kernel: `6.18.54.1-microsoft-standard-WSL2`. The CI configuration is supplied;
these results are local validation, not a claim that hosted CI has run.

Commands used:

```text
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/test-windows.ps1 -Offline
wsl.exe -d Ubuntu -- bash "/mnt/c/Users/cw/Desktop/New folder/stardust/scripts/test-linux.sh" --offline
```

Full logs are in `target/validation/windows.log` and `target/validation/linux.log`.
Generated artifacts are local build output and are excluded from version control.

| Artifact | SHA-256 |
| --- | --- |
| `target/release/stardust.exe` | `836d8bbd2f1455ea294f53b6f5d652f9bdedb8b103f50c27bef03989fa055d5e` |
| `target/linux/release/stardust` | `9daf97b1af896c8a22ccc8f952e2aaab09d33b3462e09d1abbc3bb8c4360c176` |

Source comparison also checked the relocated generator and XOR function bodies,
shipped generator configurations, and Astra's transaction/file-identity code
against the original apps. Differences in these sections are module imports,
visibility, and formatting; the algorithm and transaction bodies are retained.

Limits: the tests do not write a 20 GB fixture, simulate physical power loss, or
exercise real interactive hidden-password entry on every terminal. The successful
generator paths use injected password readers with the production parser, KDF,
stream and file writer. These bounds and the release audit steps are documented
in [testing.md](testing.md).
