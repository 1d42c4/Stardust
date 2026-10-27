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
| `src/xor/tests/` | Original parser, stream, transaction, identity/change detection, permissions, rollback and in-place tests |
| `tests/keygen_cli.rs` | Real subprocess help/version and all generator input failures before password/file access |
| `tests/xor_cli.rs`, `tests/cases/` | Original subprocess XOR/in-place tests, locks, aliases, Unicode/native filenames, executable directory, parallel copies/launches |
| `tests/app_dispatch.rs` | New combined routing, command-like filenames, error codes, no side effects on help/invalid input, output-pipe failures |
| `tests/combined_workflows.rs` | New six-workflow matrix for every generator: byte oracle and round trip, in-place, cwd/executable separation, regeneration, failed regeneration, and short-key preservation |

The new application and combined-workflow files add **43 tests**. Original tests
are retained with module paths and CLI branding adjusted. Platform-specific totals
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
