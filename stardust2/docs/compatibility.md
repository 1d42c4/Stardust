# Compatibility contract

Preserve data formats and observable operation policies, even when names change.

## Generator constants

| Item | Original value retained |
| --- | --- |
| Application salt | `versakey application salt 2026-08-11 v1` |
| Application pepper | `VersaKey-build-pepper-change-me-v1` |
| Argon2 | Argon2id v1.3; 65,536 KiB; 3 iterations; 1 lane; 32-byte output |
| scrypt | log N = 16; r = 8; p = 1; 32-byte output |
| PBKDF2 | HMAC-SHA-256; 600,000 iterations; 32-byte output |
| AES | AES-256-CTR, big-endian 128-bit counter, zero initial IV |
| ChaCha20 | Original 12-byte zero nonce and persistent counter stream |
| BLAKE3 | Keyed XOF with the original versioned stream tag |
| Length | 1..=20,000,000,000 bytes; bound into salt as big-endian u64 |
| File | Raw `key.key`, no header, in current working directory |

The five distinct domains and all framing tags still start with `versakey/`.
They are data-format identifiers, not product branding. Changing them prevents
reproduction of previously generated keys. Original fixed vectors in
`src/keygen/command_tests.rs` freeze the five shipped commands; vectors in
`src/keygen/tests.rs` also check the underlying derivation/stream helpers.

Key regeneration replaces an existing file only after full output and file sync.
Successful generation's final status-write failure remains an error explicitly
saying that the key was created. Generator validation/processing errors retain
exit code 1. Password and confirmation are read hidden, kept exact (including
Unicode and whitespace), and owned values are zeroized. Empty or mismatching
passwords fail. Underlying third-party scrypt/PBKDF2 intermediates do not all have
guaranteed erasure, as in the original app.

## XOR behavior

All names are resolved beside the canonical executable, including when it is
launched through a Linux symlink. Unsafe Windows names are also rejected on
Linux. Native non-Unicode filenames remain native `OsString`s. `--` ends option
parsing. The hidden temporary prefix remains `.astra-xor-` and is reserved.

The key must cover the whole input. There are no cycling/truncating modes. Empty
input still requires an existing plain key file; unused key tails are unread.
Both normal and in-place operations detect short reads, input growth and metadata
changes. Old outputs survive pre-commit failures. Source/key/executable identities
are protected according to the original mode, including Windows case aliases and
hard links. Existing output permissions are retained; Unix special bits are
cleared. New Unix XOR outputs are private.

In-place replacement owns and closes the source reader before the Windows rename
while retaining writer protection through another handle. Other hard links and
existing readers retain the old version. Do not replace this with an in-place
write/truncate implementation. XOR argument errors retain exit code 2 and runtime
errors code 1. As in Astra, closed status pipes do not fail a committed operation.

## Deliberate interface changes

There is one executable, named `stardust`. Add `xor` before Astra's arguments;
VersaKey's five subcommands are unchanged. Help/version/status branding identifies
Stardust. Top-level help describes both sets of operations. No legacy binaries or
implicit default operation are installed. Directory policies, key bytes, file
contents and failure policies remain those of the original apps.
