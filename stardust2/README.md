# Stardust

Stardust combines **Astra XOR** and **all five VersaKey generators** in one Rust
command-line executable for Windows and Linux. The original sibling projects
are independent; Stardust builds and runs without either source directory.

## Build and run

Install Rust with the C/C++ build tools appropriate to your OS. The repository
pins Rust **1.98.1**, matching VersaKey, in `rust-toolchain.toml`.

```text
cargo build --release --locked
```

The executable is `target/release/stardust.exe` on Windows and
`target/release/stardust` on Linux. Build separately for each OS. Use release
builds for the production KDFs and large files.

```powershell
# Windows: run from the executable's directory so both operations use its key.
cd target\release
.\stardust.exe argon2-aes
# Enter the requested size in bytes, then the password twice (hidden).
# Put input.bin here, beside stardust.exe and key.key.
.\stardust.exe xor input.bin masked.bin
.\stardust.exe xor masked.bin restored.bin
```

```bash
# Linux
cd target/release
./stardust argon2-aes
./stardust xor input.bin masked.bin
./stardust xor masked.bin restored.bin
```

Use `stardust --help`, `stardust xor --help`, or
`stardust <generator> --help` for help. Replace `stardust` with `./stardust` on
Linux or `.\stardust.exe` in PowerShell if it is not on PATH.

## Commands and compatibility

| Original invocation | Stardust invocation | Behavior |
| --- | --- | --- |
| `astra-xor FILE_A OUTPUT` | `stardust xor FILE_A OUTPUT` | XOR with the prefix of `key.key`; exact input length |
| `astra-xor --in-place FILE_A` | `stardust xor --in-place FILE_A` | Atomically replace the source after completing and syncing the result |
| `versakey argon2-aes` | `stardust argon2-aes` | Argon2id + AES-256-CTR |
| `versakey scrypt-aes` | `stardust scrypt-aes` | scrypt + AES-256-CTR |
| `versakey pbkdf2-aes` | `stardust pbkdf2-aes` | PBKDF2-HMAC-SHA-256 + AES-256-CTR |
| `versakey argon2-chacha20` | `stardust argon2-chacha20` | Argon2id + ChaCha20 |
| `versakey argon2-blake3` | `stardust argon2-blake3` | Argon2id + keyed BLAKE3 XOF |

Executable names, help branding, and the `xor` dispatch word change. Generated
bytes, input validation, file safety rules, stream sizes, and the original
operations' exit codes are retained. There is no default generator or implicit
file transformation. `stardust help xor` and `stardust help <generator>` also work.

**The original location rules are intentionally preserved:**

- Generators write or replace `key.key` in the **current working directory**.
- XOR reads and writes files **beside the resolved executable**, regardless of
  the working directory. Only filenames are accepted, with `--` available for
  names beginning with a dash.
- To use both together, run generation from the executable's directory, or put
  the generated `key.key` beside the executable before running XOR.

Each generator prompts for a decimal size from **1 through 20,000,000,000 bytes**
and two matching hidden passwords. Passwords are never command-line arguments.
The same password, size, and suite produce exactly the original VersaKey bytes;
`key.key` contains no header. Size participates in derivation, so generating a
larger key does not preserve the shorter key's prefix. Record the suite and size
if you need to reproduce a key. Running another generator replaces `key.key`.

XOR requires `key.key` even for empty input. The key must be at least as long as
the input; it is never repeated. XOR twice with the same key restores the input.
The key and executable, including their hard links, cannot be replaced by XOR.
Without `--in-place`, the output cannot alias the source. File symlinks, Windows
reparse points, devices, directories, and unsafe filenames are refused. In-place
replacement needs temporary space for the complete result, and other hard links
keep their original bytes.

## Reliability and limits

Both operations stream with bounded memory (64 KiB per XOR input buffer;
1 MiB key output buffer plus the KDF working set). They write to a same-directory
temporary file and sync the completed file before atomic replacement. Failure
before replacement preserves the old output. Post-replacement directory-sync
failure is explicitly reported as already committed.

The two transaction implementations are separate because their original policies
differ: XOR protects file identities and detects changes; key generation replaces
`key.key` using VersaKey's existing policy. Do not run competing writers against
the same files. Change detection is not a snapshot or protection against hostile
concurrent directory manipulation. Filesystems and hardware still determine
power-loss durability. Windows lacks the Unix directory-sync path and replacement
inherits directory ACL policy rather than preserving an old custom ACL.

Normal errors and unwinding clean up temporary files. Forced termination or
power loss can leave `.key.key.*` or `.astra-xor-*.tmp`; remove a stale file only
after confirming that no process owns it. No startup cleanup deletes another
process's temporary files.

XOR does not authenticate data or manage keys. Password-derived deterministic
output is not a true-random one-time pad; its entropy is limited by the password
and derived key. The compiled-in pepper is not an external secret. Owned password,
key, and generator buffer values retain VersaKey's zeroization, with its existing
third-party KDF limitations. See [compatibility details](docs/compatibility.md).

## Development and tests

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/test-windows.ps1
# Add -Offline when all dependencies are cached.
```

```bash
bash scripts/test-linux.sh
# Add --offline when all dependencies are cached.
```

From PowerShell in this project with Ubuntu WSL installed:

```powershell
wsl.exe -d Ubuntu -- bash scripts/test-linux.sh --offline
```

Both scripts run formatting, all release tests, Clippy with warnings denied, and
a release build. Linux tests use a source snapshot and fixtures on the Linux
filesystem; Linux artifacts go under `target/linux` to avoid mixing with Windows.
Logs are under `target/validation`. CI repeats checks on native Windows and Linux.
The scripts default to two test threads to bound concurrent KDF memory use.

The suite preserves both original test collections and adds combined workflows
for every generator. It covers known answers, independent byte oracles, large
buffer boundaries, restoration, directory rules, malformed arguments, native
filenames, injected I/O failures, temporary cleanup, locks, links, and permissions.
See [testing](docs/testing.md), [architecture](docs/architecture.md), and
[future-agent instructions](agent.md) before changing the implementation.
