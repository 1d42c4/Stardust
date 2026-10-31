# Testing and release evidence

See [the recorded local validation](validation.md) for the completed Windows
and Linux runs and release artifacts.

Run the platform script from the README. Both use the pinned toolchain and lock
file and require formatting, the entire release suite, Clippy with `-D warnings`,
and a release build. Do not replace a Linux run with a Windows cross-compilation.
The GitHub Actions workflow assumes this folder is the repository root.

| Test group | Purpose |
| --- | --- |
| `src/keygen/tests.rs` | Original primitive vectors, KDF sensitivity, framing, stream/counter limits, partition independence, short/interrupted/zero writes, disk failures and cleanup |
| `src/keygen/command_tests.rs` | Five original command configurations and known answers; 2 MiB + 137 byte fixtures checked against independent expansion and repeated-pattern regressions |
| `src/keygen/cli_tests.rs` | Exact hidden-password flow, input bounds, validation ordering, input/output failures and post-commit status errors |
| `src/keygen/cli_reliability_tests.rs` | Bounded and interrupted size input, UTF-8 and CRLF, exact password bytes, buffered console output, flush failures, and panic/error status |
| `src/keygen/storage_tests.rs` | File/directory sync failures and panics, publication ordering, cleanup ownership, missing parents, hard links, native directory names, and OS-specific replacement policy |
| `src/xor/tests/` | Original parser, stream, transaction, identity/change detection, permissions, rollback and in-place tests |
| `tests/keygen_cli.rs` | Real subprocess help/version and all generator input failures before password/file access |
| `tests/xor_cli.rs`, `tests/cases/` | Original subprocess XOR/in-place tests, locks, aliases, Unicode/native filenames, executable directory, parallel copies/launches |
| `tests/app_dispatch.rs` | New combined routing, command-like filenames, error codes, no side effects on help/invalid input, output-pipe failures |
| `tests/combined_workflows.rs` | New six-workflow matrix for every generator: byte oracle and round trip, in-place, cwd/executable separation, regeneration, failed regeneration, and short-key preservation |

The initial application and combined-workflow files added **43 tests**. The
2026-10-31 audit adds **59 tests on each platform** (60 distinct new test IDs,
including separate Windows-lock and Unix-symlink cases): 20 prompt regressions,
15 storage tests per OS, 6 KDF/framing/counter tests, 8 XOR-stream tests,
7 XOR-transaction tests, and 3 application-dispatch tests. The two new buffered
status regressions failed before the final status flush was added and pass after
the fix. The dispatch regression also exercises that failure with all five real
production generators and confirms the committed key is reproducible.

Original tests are retained with module paths and CLI branding adjusted. Platform-specific totals
differ because Windows handle/case checks and Linux permissions/symlink/FIFO
checks compile on their respective platforms.

Production-cost KDF tests run in release mode. Defaulting scripts to two test
threads bounds simultaneous memory-hard work; tests still explicitly exercise
concurrent file operations. Cheap KDF parameters are confined to private test
helpers. Fixtures use isolated temporary directories. The test code never
generates a real key in the repository or uses the user's files.

Automatic coverage is not a hardware power-loss test or a proof about every
filesystem. Maximum-size validation and counter arithmetic are tested without
writing a 20 GB fixture. Terminal password entry is replaced by a callback for
successful automated generator runs; subprocess tests never pipe a password as
a substitute for hidden input. A release audit should also exercise real hidden
password entry and cancellation on each supported terminal, plus its deployment
filesystem and ACL policy.

Local run logs are `target/validation/windows.log` and `target/validation/linux.log`.
The Linux script creates a temporary source snapshot and native Linux fixtures,
with separate build output under `target/linux`. The snapshot is removed only
after its resolved path is checked to be the exact directory this run created.

## HTML report

Open [README.html](../README.html) for the complete README followed by a searchable
table explaining all 404 distinct test IDs and showing the recorded outcome on
each OS. The final runs contain 387 Windows tests and 390 Linux tests. Neither
platform ignored or filtered any tests. A platform-specific test absent from the
other binary is labeled **Not included**, never passed or skipped.

After both platform scripts pass, regenerate the page from PowerShell:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-readme-report.ps1
```

The dependency-free renderer supports the Markdown constructs used in this README.
It copies the raw logs into `docs/test-results/` and writes `results.json` containing
individual results, explanations, timestamps, and source SHA-256 hashes. Explanations
are maintained in `docs/test-descriptions.txt`; missing entries fail generation.
The report builder also rejects incomplete runs, duplicate test IDs, mismatched
totals, nonzero ignored/filtered counts, and source files newer than either run.
The report is a saved local snapshot, not live CI or a guarantee about later edits.

The storage tests preserve VersaKey's existing policy: on Windows, an open reader
can cause safe key-replacement failure; closing it permits retry. On Unix, replacing
a key symlink replaces the link entry without modifying its target. These policies
remain distinct from XOR's stronger alias and input protections.
