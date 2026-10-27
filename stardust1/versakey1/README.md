# VersaKey1

VersaKey is one interactive Rust CLI for Windows and Linux with five explicit
generator subcommands. Each deterministically creates `key.key` in the current
directory. After selecting a subcommand, enter a decimal byte count from `1`
through `20000000000`, then enter the password twice with hidden input.

Build the single executable with the pinned Rust toolchain:

```text
cargo build --release --locked
```

Use a release build, especially for large files. Run a generator through Cargo:

```text
cargo run --release -- argon2-aes
cargo run --release -- scrypt-aes
cargo run --release -- pbkdf2-aes
cargo run --release -- argon2-chacha20
cargo run --release -- argon2-blake3
```

Or run the compiled executable directly:

```text
# Windows PowerShell
.\target\release\versakey.exe argon2-aes

# Linux
./target/release/versakey argon2-aes
```

Build separately on each OS. `versakey --help` lists the subcommands;
`versakey scrypt-aes --help` describes one command. A missing or unknown
subcommand, or an unexpected argument, fails before any password prompt or
file write. There is no implicit default generator. Size and password are
entered interactively; passwords are never accepted as command-line arguments.

## Choose a suite

| Subcommand | Construction | Intended choice |
| --- | --- | --- |
| `argon2-aes` | Argon2id v1.3 (64 MiB, 3 iterations, 1 lane, 32-byte output) -> AES-256-CTR | Original suite; preserves the output compatibility of earlier VersaKey versions. |
| `scrypt-aes` | scrypt (`N=2^16`, `r=8`, `p=1`, about 64 MiB, 32-byte output) -> AES-256-CTR | Memory-hard alternative to Argon2id. |
| `pbkdf2-aes` | PBKDF2-HMAC-SHA-256 (600,000 iterations, 32-byte output) -> AES-256-CTR | Widely standardized, low-memory alternative. |
| `argon2-chacha20` | Argon2id v1.3 (64 MiB, 3 iterations, 1 lane, 32-byte output) -> ChaCha20 | Non-AES stream-cipher alternative. |
| `argon2-blake3` | Argon2id v1.3 (64 MiB, 3 iterations, 1 lane, 32-byte output) -> keyed BLAKE3 XOF | Direct extendable-output alternative. |

The suites are intentionally domain-separated and non-interchangeable. The
same password and size reproduce the same bytes only when the same suite and
compatibility-critical constants are used. The five subcommands deliberately
produce different bytes from one another, and `key.key` contains no suite
header. Record the subcommand and requested size if a key must be reproduced later.

All five subcommands retain their predecessor's application salt, pepper,
domain, KDF parameters, and stream construction. Existing password-and-size
combinations continue to reproduce the original bytes with this mapping:

| Previous executable | New command |
| --- | --- |
| `versakey` | `versakey argon2-aes` |
| `versakey-scrypt` | `versakey scrypt-aes` |
| `versakey-pbkdf2` | `versakey pbkdf2-aes` |
| `versakey-chacha20` | `versakey argon2-chacha20` |
| `versakey-blake3` | `versakey argon2-blake3` |

The separate executable targets have been removed. Changing a suite's salt,
pepper, domain, KDF parameters, or stream construction breaks that suite's
compatibility. The CLI names are not themselves inputs to derivation.

## Output handling

Every subcommand writes or replaces the same file, `key.key`, in the current
directory. Running a second suite therefore replaces the first suite's output;
move or rename a key first if both files are needed.

Generation streams through a 1 MiB buffer. Its memory use is bounded by that
buffer plus the selected KDF's fixed working set and does not grow with the
requested file size, including at the 20,000,000,000-byte limit.

A completed temporary file is flushed, synchronized, and atomically moved to
`key.key`. A derivation or write failure before the final replacement preserves
an earlier `key.key`. A rare Unix directory-sync failure after replacement is
reported explicitly as already committed because the replacement cannot then
be rolled back.

Normal errors and Rust unwinding remove the temporary file. A forced process
kill, abort, or power loss can leave a partial `.key.key.*` file behind. After
confirming that no VersaKey process is running, such a stale file can be
removed. It is not deleted automatically because another concurrent VersaKey
process may still own it.

On Windows, the completed temporary file is synchronized before the atomic
replacement, but the platform does not provide the same directory-sync path
used on Unix. A sudden power loss immediately after replacement can therefore
have weaker rename-durability guarantees than on Unix. The replacement also
inherits the containing directory's access-control policy rather than
preserving a custom ACL from an older `key.key`; use a suitably restricted
directory for sensitive output.

## Determinism and security

- The requested byte length and suite domain participate in derivation. A
  different size changes the whole output rather than sharing the shorter
  file's stream prefix.
- Output entropy is limited by the password-derived 256-bit key. A very large
  `key.key` is deterministic pseudorandom material, not a true-random one-time
  pad merely because it is large.
- Individual bytes and short substrings can and will repeat in a sufficiently
  large random-looking file. No generator can make every byte unique after 256
  bytes, and none of these suites cycles or copies a shorter digest.
- The AES-256-CTR suites use one persistent stream whose 128-bit counter is not
  reused within a generated file. Since AES is a permutation, their aligned
  16-byte blocks cannot repeat at the supported sizes. That specific
  no-repeated-block property does not apply to ChaCha20 or BLAKE3 output.
- The compiled-in pepper can be extracted from an executable. It diversifies
  builds but is not a substitute for a strong password or an external secret.
- VersaKey zeroizes the password values and key/buffer state it owns. The
  third-party scrypt and PBKDF2 implementations do not guarantee erasure of
  every internal intermediate allocation after derivation.
- Anyone who obtains `key.key` can test password guesses offline by regenerating
  it. Argon2id and scrypt use memory-hard work to raise guessing cost, but no
  KDF adds entropy to a weak password. Use a long, high-entropy passphrase.
- PBKDF2 is CPU-hard but not memory-hard. Its low memory requirement and broad
  standardization can be useful for constrained or compatibility-oriented
  deployments, but attackers can parallelize guesses more economically than
  against the configured Argon2id or scrypt suites. Prefer a memory-hard suite
  unless that PBKDF2 tradeoff is specifically needed.

## Tests

Run the complete test suite using production-strength KDFs in release mode:

```text
cargo test --release --locked
```

Each subcommand has named tests for its original known-answer output, fixed
configuration, repeated small and large generations in separate directories,
password/size sensitivity, exact file lengths, and failed replacements.
Successful CLI tests use the real parser, dispatch, derivation, and file writer
with an injected password reader; subprocess tests cover help, argument
validation, suite selection, and invalid size handling.

For each subcommand, a 2 MiB + 137 byte fixture crosses two production buffer
boundaries. Tests compare its entire content against direct AES counter
encryption, a single ChaCha20 expansion, or a single BLAKE3 XOF read. They also
reject every repeating whole-file period from 1 byte through 1 MiB and check
for duplicate aligned 16-byte blocks in that fixed fixture. These are
regression checks for accidental repetition or stream resets, not a proof
that arbitrary output has no repeated substrings or an exhaustive test of
every supported file size.

Shared reliability tests cover partial/interrupted writes, write-zero errors,
injected disk failures on either side of buffer boundaries, preservation of
an existing key before commit, temporary-file cleanup on errors and unwinding,
counter limits, bounded writes, hidden-password handling, and Unix permissions.
The algorithms, KDF costs, zeroization, and atomic replacement path are shared
by all subcommands.

Additional checks:

```text
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
```

On Linux, `bash scripts/test-linux.sh` runs formatting, tests, lint, and a
release build. From the project directory on Windows with Ubuntu installed
in WSL, run `wsl.exe -d Ubuntu -- bash scripts/test-linux.sh`. The script uses
`target/linux` to keep Linux build artifacts separate from native Windows builds.
