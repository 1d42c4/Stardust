# Local validation: 2026-10-31

The expanded suite passed on Windows and native Linux in Ubuntu WSL2 with pinned
Rust 1.98.1 and cached, locked dependencies. No tests failed, were ignored, or were
filtered out. These are local runs; no hosted CI execution is claimed.

| Check | Windows x86_64 MSVC | Linux x86_64 GNU, Ubuntu WSL2 |
| --- | --- | --- |
| Unit regressions | 280 passed | 284 passed |
| Combined command dispatch | 15 passed | 15 passed |
| Combined generator/XOR workflows | 31 passed | 31 passed |
| Generator subprocess tests | 8 passed | 8 passed |
| XOR/in-place subprocess tests | 53 passed | 52 passed |
| Total | **387 passed** | **390 passed** |
| `cargo fmt --all -- --check` | Passed | Passed |
| `cargo clippy --all-targets --locked --offline -- -D warnings` | Passed | Passed |
| Locked offline release build | Passed | Passed |

The binary and documentation harnesses also completed successfully with zero
tests defined. There are 404 distinct test IDs across the two platforms. Compared
with the previous 328/331-test baseline, this audit adds 59 tests on each platform.
All inherited tests and original known-answer vectors remain intact.

Found and fixed: the generator's final success message was not explicitly flushed.
Two new tests reproduced buffered text remaining unwritten and a missed flush
error. Final status is now flushed, with failures classified as already committed.
A real-generation regression covers this behavior for all five suites and verifies
the committed bytes remain reproducible. No generator constants or output formats
changed. A private synchronization callback seam makes the original key transaction
testable immediately before and after publication without changing its policy.

The new tests also cover interrupted/oversized input, exact Unicode and NUL password
bytes, large-length framing, counter exhaustion and rollback, combined stream I/O
failures, late file sync failures, cleanup ownership, hard links, native filenames,
and Windows/Unix replacement behavior. See [testing.md](testing.md) for the breakdown.

Commands used:

```text
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/test-windows.ps1 -Offline
wsl.exe -d Ubuntu -- bash /mnt/c/Users/cw/Desktop/stardust/stardust2/scripts/test-linux.sh --offline
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-readme-report.ps1
```

Windows run began at **04:04:48 UTC**; Linux at **04:05:36 UTC** on 2026-10-31.
Linux kernel: `6.18.54.1-microsoft-standard-WSL2`. Linux test fixtures and the source
snapshot were on native Linux storage. Both scripts used two test threads and
production-cost KDFs in release mode. Build directories remained separate.

Open [the README and individual test report](../README.html#test-results).
Portable raw evidence is saved in [windows.log](test-results/windows.log) and
[linux.log](test-results/linux.log). [results.json](test-results/results.json)
contains every result, its explanation, and the audited source-file hashes.
Original run logs also remain under `target/validation/`.

| Release artifact | SHA-256 |
| --- | --- |
| `target/release/stardust.exe` | `16f087ed3e81fd3d453816777f97e8421838157d8ce4e39f59caeefa13e5b84b` |
| `target/linux/release/stardust` | `a096a9d304d02dd60c96c98b19ae84a1543448f82568322835a1816f53aecc29` |

Limits: automated tests do not write a 20 GB key, simulate physical power loss,
or verify real hidden-password entry on every terminal. Successful generator
tests use injected password input with real production derivation and file writing.
Deployment filesystem durability and ACL behavior still require environment-specific
review. This report is a snapshot of the recorded local runs.
