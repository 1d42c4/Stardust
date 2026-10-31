# Stardust

Stardust combines **Astra XOR** and **all five VersaKey generators** in one Rust
command-line executable for Windows and Linux. The original sibling projects
are independent; Stardust builds and runs without either source directory.

Read the [HTML edition with explained Windows and Linux test results](README.html).

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

<!-- BEGIN VALIDATION REPORT -->

## Test results

Validation record · 31 October 2026

### Every test, explained.

The complete release test suite ran on Windows and native Linux under Ubuntu
WSL2. This record includes the existing suite and **59 additional tests on each
platform**. Each result below comes from the recorded test logs.

| Result | Count | Details |
| --- | --- | --- |
| Windows tests passed | **387** | 0 failed · 0 ignored |
| Linux tests passed | **390** | 0 failed · 0 ignored |
| New tests per platform | **59** | Failures, boundaries & file safety |

### Found and fixed: buffered success reporting

Key generation previously returned without flushing its final success message.
Two new regression tests reproduced an unwritten message and a silently missed
flush error. The message is now flushed, and a failure explicitly says the key
was already created. An integration test checks this behavior with all five
generators and confirms that the committed key remains reproducible.

### Required validation checks

| Check | Windows | Linux / WSL2 |
| --- | --- | --- |
| Full release tests, locked dependencies | Passed | Passed |
| Formatting: cargo fmt --all -- --check | Passed | Passed |
| Clippy: all targets, warnings denied | Passed | Passed |
| Locked release build | Passed | Passed |

### Recorded runs

**Windows:** 2026-10-31 04:04:48 UTC · x86_64-pc-windows-msvc ·
[Full Windows log](docs/test-results/windows.log)

**Linux:** 2026-10-31 04:05:36 UTC · x86_64-unknown-linux-gnu ·
[Full Linux log](docs/test-results/linux.log)

Both use Rust 1.98.1, cached locked dependencies, and two test threads. Linux
fixtures ran on the Linux filesystem.
[Structured results and source hashes](docs/test-results/results.json) ·
[Run details and release hashes](docs/validation.md)

### Individual test results

404 distinct test IDs across both platforms. “Not included” means the test is
absent from that platform's test binary; it does not claim a pass. Conditional
tests cover OS-specific links, permissions, locking, and file identities. The
binary and documentation harnesses also completed, with zero tests defined in
each.

Test IDs link to their source files. Use the
[HTML edition](README.html#test-results) to search and filter the table.

| Test | What it checks | Windows | Linux |
| --- | --- | --- | --- |
| broken generator status pipe fails before password or generation<br>[`app_dispatch/broken_generator_status_pipe_fails_before_password_or_generation`](tests/app_dispatch.rs) | Breaks the generator's output stream and verifies generation and password prompting never begin. | Passed | Passed |
| closed status pipe does not turn committed xor into failure<br>[`app_dispatch/closed_status_pipe_does_not_turn_committed_xor_into_failure`](tests/app_dispatch.rs) | Breaks XOR status output after publication and verifies the committed operation remains successful. | Passed | Passed |
| command names remain literal xor filenames<br>[`app_dispatch/command_names_remain_literal_xor_filenames`](tests/app_dispatch.rs) | Uses generator names and option-like names as literal XOR filenames after the option terminator. | Passed | Passed |
| confirmation read failure preserves the existing key and hard link<br>[`app_dispatch/confirmation_read_failure_preserves_the_existing_key_and_hard_link`](tests/app_dispatch.rs) | Fails confirmation for all five generators and checks old bytes and the original hard-link relationship survive. | Passed | Passed |
| each generator routes to its original configuration<br>[`app_dispatch/each_generator_routes_to_its_original_configuration`](tests/app_dispatch.rs) | Checks every generator command selects the expected suite, domain, and application salt. | Passed | Passed |
| every generator accepts a native non unicode working directory<br>[`app_dispatch/every_generator_accepts_a_native_non_unicode_working_directory`](tests/app_dispatch.rs) | Generates real keys from all five suites inside an OS-native non-Unicode directory without using a lossy path. | Passed | Passed |
| final flush failure reports commit and keeps each generated key<br>[`app_dispatch/final_flush_failure_reports_commit_and_keeps_each_generated_key`](tests/app_dispatch.rs) | For every production suite, fails final status flush and checks exit code, committed bytes, cleanup, and reproducibility. | Passed | Passed |
| generating a key never requires an executable file<br>[`app_dispatch/generating_a_key_never_requires_an_executable_file`](tests/app_dispatch.rs) | Generates through the application controller with a nonexistent executable path. | Passed | Passed |
| global version does not require a directory or terminal<br>[`app_dispatch/global_version_does_not_require_a_directory_or_terminal`](tests/app_dispatch.rs) | Checks global version output works with nonexistent environment paths and no password reader. | Passed | Passed |
| invalid global commands never read input or echo secrets<br>[`app_dispatch/invalid_global_commands_never_read_input_or_echo_secrets`](tests/app_dispatch.rs) | Checks malformed top-level commands fail without prompting or echoing synthetic secret arguments. | Passed | Passed |
| malformed xor commands retain astra usage exit code<br>[`app_dispatch/malformed_xor_commands_retain_astra_usage_exit_code`](tests/app_dispatch.rs) | Checks invalid XOR command forms retain usage exit code 2. | Passed | Passed |
| native non unicode subcommand is rejected without lossy dispatch<br>[`app_dispatch/native_non_unicode_subcommand_is_rejected_without_lossy_dispatch`](tests/app_dispatch.rs) | Checks a non-Unicode command selector is rejected instead of being guessed through lossy text. | Passed | Passed |
| root help lists all six operations and both location policies<br>[`app_dispatch/root_help_lists_all_six_operations_and_both_location_policies`](tests/app_dispatch.rs) | Checks root help lists five generators, XOR, and their distinct directory rules. | Passed | Passed |
| wrong generator size is rejected before password in combined dispatch<br>[`app_dispatch/wrong_generator_size_is_rejected_before_password_in_combined_dispatch`](tests/app_dispatch.rs) | Checks application-level dispatch rejects invalid sizes for every suite before password entry. | Passed | Passed |
| xor help aliases explain the same contract without files<br>[`app_dispatch/xor_help_aliases_explain_the_same_contract_without_files`](tests/app_dispatch.rs) | Checks the application-level XOR help aliases produce identical help without file access. | Passed | Passed |
| failed regeneration keeps a key usable for restoration<br>[`combined_workflows/argon2_aes::failed_regeneration_keeps_a_key_usable_for_restoration`](tests/combined_workflows.rs) | Fails a password-confirmation regeneration and proves the old key still restores existing ciphertext. | Passed | Passed |
| generated key crosses both buffer sizes and restores exactly<br>[`combined_workflows/argon2_aes::generated_key_crosses_both_buffer_sizes_and_restores_exactly`](tests/combined_workflows.rs) | Generates a multi-MiB key, checks XOR against a byte oracle, and restores the original source. | Passed | Passed |
| generated key supports atomic in place round trip<br>[`combined_workflows/argon2_aes::generated_key_supports_atomic_in_place_round_trip`](tests/combined_workflows.rs) | Uses the selected generated key for in-place XOR twice and checks exact restoration. | Passed | Passed |
| generated short key cannot damage an input or existing output<br>[`combined_workflows/argon2_aes::generated_short_key_cannot_damage_an_input_or_existing_output`](tests/combined_workflows.rs) | Generates a key one byte too short and checks both XOR modes preserve all existing files. | Passed | Passed |
| generation and xor retain their distinct directory rules<br>[`combined_workflows/argon2_aes::generation_and_xor_retain_their_distinct_directory_rules`](tests/combined_workflows.rs) | Checks generation uses its working directory while XOR uses files beside the executable. | Passed | Passed |
| regenerated key restores a previous ciphertext<br>[`combined_workflows/argon2_aes::regenerated_key_restores_a_previous_ciphertext`](tests/combined_workflows.rs) | Regenerates the selected suite's key from identical inputs and uses it to restore earlier ciphertext. | Passed | Passed |
| failed regeneration keeps a key usable for restoration<br>[`combined_workflows/argon2_blake3::failed_regeneration_keeps_a_key_usable_for_restoration`](tests/combined_workflows.rs) | Fails a password-confirmation regeneration and proves the old key still restores existing ciphertext. | Passed | Passed |
| generated key crosses both buffer sizes and restores exactly<br>[`combined_workflows/argon2_blake3::generated_key_crosses_both_buffer_sizes_and_restores_exactly`](tests/combined_workflows.rs) | Generates a multi-MiB key, checks XOR against a byte oracle, and restores the original source. | Passed | Passed |
| generated key supports atomic in place round trip<br>[`combined_workflows/argon2_blake3::generated_key_supports_atomic_in_place_round_trip`](tests/combined_workflows.rs) | Uses the selected generated key for in-place XOR twice and checks exact restoration. | Passed | Passed |
| generated short key cannot damage an input or existing output<br>[`combined_workflows/argon2_blake3::generated_short_key_cannot_damage_an_input_or_existing_output`](tests/combined_workflows.rs) | Generates a key one byte too short and checks both XOR modes preserve all existing files. | Passed | Passed |
| generation and xor retain their distinct directory rules<br>[`combined_workflows/argon2_blake3::generation_and_xor_retain_their_distinct_directory_rules`](tests/combined_workflows.rs) | Checks generation uses its working directory while XOR uses files beside the executable. | Passed | Passed |
| regenerated key restores a previous ciphertext<br>[`combined_workflows/argon2_blake3::regenerated_key_restores_a_previous_ciphertext`](tests/combined_workflows.rs) | Regenerates the selected suite's key from identical inputs and uses it to restore earlier ciphertext. | Passed | Passed |
| failed regeneration keeps a key usable for restoration<br>[`combined_workflows/argon2_chacha20::failed_regeneration_keeps_a_key_usable_for_restoration`](tests/combined_workflows.rs) | Fails a password-confirmation regeneration and proves the old key still restores existing ciphertext. | Passed | Passed |
| generated key crosses both buffer sizes and restores exactly<br>[`combined_workflows/argon2_chacha20::generated_key_crosses_both_buffer_sizes_and_restores_exactly`](tests/combined_workflows.rs) | Generates a multi-MiB key, checks XOR against a byte oracle, and restores the original source. | Passed | Passed |
| generated key supports atomic in place round trip<br>[`combined_workflows/argon2_chacha20::generated_key_supports_atomic_in_place_round_trip`](tests/combined_workflows.rs) | Uses the selected generated key for in-place XOR twice and checks exact restoration. | Passed | Passed |
| generated short key cannot damage an input or existing output<br>[`combined_workflows/argon2_chacha20::generated_short_key_cannot_damage_an_input_or_existing_output`](tests/combined_workflows.rs) | Generates a key one byte too short and checks both XOR modes preserve all existing files. | Passed | Passed |
| generation and xor retain their distinct directory rules<br>[`combined_workflows/argon2_chacha20::generation_and_xor_retain_their_distinct_directory_rules`](tests/combined_workflows.rs) | Checks generation uses its working directory while XOR uses files beside the executable. | Passed | Passed |
| regenerated key restores a previous ciphertext<br>[`combined_workflows/argon2_chacha20::regenerated_key_restores_a_previous_ciphertext`](tests/combined_workflows.rs) | Regenerates the selected suite's key from identical inputs and uses it to restore earlier ciphertext. | Passed | Passed |
| failed regeneration keeps a key usable for restoration<br>[`combined_workflows/pbkdf2_aes::failed_regeneration_keeps_a_key_usable_for_restoration`](tests/combined_workflows.rs) | Fails a password-confirmation regeneration and proves the old key still restores existing ciphertext. | Passed | Passed |
| generated key crosses both buffer sizes and restores exactly<br>[`combined_workflows/pbkdf2_aes::generated_key_crosses_both_buffer_sizes_and_restores_exactly`](tests/combined_workflows.rs) | Generates a multi-MiB key, checks XOR against a byte oracle, and restores the original source. | Passed | Passed |
| generated key supports atomic in place round trip<br>[`combined_workflows/pbkdf2_aes::generated_key_supports_atomic_in_place_round_trip`](tests/combined_workflows.rs) | Uses the selected generated key for in-place XOR twice and checks exact restoration. | Passed | Passed |
| generated short key cannot damage an input or existing output<br>[`combined_workflows/pbkdf2_aes::generated_short_key_cannot_damage_an_input_or_existing_output`](tests/combined_workflows.rs) | Generates a key one byte too short and checks both XOR modes preserve all existing files. | Passed | Passed |
| generation and xor retain their distinct directory rules<br>[`combined_workflows/pbkdf2_aes::generation_and_xor_retain_their_distinct_directory_rules`](tests/combined_workflows.rs) | Checks generation uses its working directory while XOR uses files beside the executable. | Passed | Passed |
| regenerated key restores a previous ciphertext<br>[`combined_workflows/pbkdf2_aes::regenerated_key_restores_a_previous_ciphertext`](tests/combined_workflows.rs) | Regenerates the selected suite's key from identical inputs and uses it to restore earlier ciphertext. | Passed | Passed |
| failed regeneration keeps a key usable for restoration<br>[`combined_workflows/scrypt_aes::failed_regeneration_keeps_a_key_usable_for_restoration`](tests/combined_workflows.rs) | Fails a password-confirmation regeneration and proves the old key still restores existing ciphertext. | Passed | Passed |
| generated key crosses both buffer sizes and restores exactly<br>[`combined_workflows/scrypt_aes::generated_key_crosses_both_buffer_sizes_and_restores_exactly`](tests/combined_workflows.rs) | Generates a multi-MiB key, checks XOR against a byte oracle, and restores the original source. | Passed | Passed |
| generated key supports atomic in place round trip<br>[`combined_workflows/scrypt_aes::generated_key_supports_atomic_in_place_round_trip`](tests/combined_workflows.rs) | Uses the selected generated key for in-place XOR twice and checks exact restoration. | Passed | Passed |
| generated short key cannot damage an input or existing output<br>[`combined_workflows/scrypt_aes::generated_short_key_cannot_damage_an_input_or_existing_output`](tests/combined_workflows.rs) | Generates a key one byte too short and checks both XOR modes preserve all existing files. | Passed | Passed |
| generation and xor retain their distinct directory rules<br>[`combined_workflows/scrypt_aes::generation_and_xor_retain_their_distinct_directory_rules`](tests/combined_workflows.rs) | Checks generation uses its working directory while XOR uses files beside the executable. | Passed | Passed |
| regenerated key restores a previous ciphertext<br>[`combined_workflows/scrypt_aes::regenerated_key_restores_a_previous_ciphertext`](tests/combined_workflows.rs) | Regenerates the selected suite's key from identical inputs and uses it to restore earlier ciphertext. | Passed | Passed |
| xor does not prompt or generate when key is missing<br>[`combined_workflows/xor_does_not_prompt_or_generate_when_key_is_missing`](tests/combined_workflows.rs) | Checks missing-key XOR fails without prompting, generating a replacement key, or changing source bytes. | Passed | Passed |
| argon2 aes validates before password or file access<br>[`keygen_cli/argon2_aes_validates_before_password_or_file_access`](tests/keygen_cli.rs) | Runs argon2-aes as a subprocess and checks invalid sizes fail before password prompts or key replacement. | Passed | Passed |
| argon2 blake3 validates before password or file access<br>[`keygen_cli/argon2_blake3_validates_before_password_or_file_access`](tests/keygen_cli.rs) | Runs argon2-blake3 as a subprocess and checks invalid sizes fail before password prompts or key replacement. | Passed | Passed |
| argon2 chacha20 validates before password or file access<br>[`keygen_cli/argon2_chacha20_validates_before_password_or_file_access`](tests/keygen_cli.rs) | Runs argon2-chacha20 as a subprocess and checks invalid sizes fail before password prompts or key replacement. | Passed | Passed |
| each subcommand has help without starting generation<br>[`keygen_cli/each_subcommand_has_help_without_starting_generation`](tests/keygen_cli.rs) | Runs every generator help form without prompting or creating a key. | Passed | Passed |
| help and version exit successfully without prompting or writing<br>[`keygen_cli/help_and_version_exit_successfully_without_prompting_or_writing`](tests/keygen_cli.rs) | Runs global help and version as subprocesses and checks success without interactive or file side effects. | Passed | Passed |
| missing unknown or extra arguments never default to a generator<br>[`keygen_cli/missing_unknown_or_extra_arguments_never_default_to_a_generator`](tests/keygen_cli.rs) | Checks invalid generator command lines never silently select a default operation. | Passed | Passed |
| pbkdf2 aes validates before password or file access<br>[`keygen_cli/pbkdf2_aes_validates_before_password_or_file_access`](tests/keygen_cli.rs) | Runs pbkdf2-aes as a subprocess and checks invalid sizes fail before password prompts or key replacement. | Passed | Passed |
| scrypt aes validates before password or file access<br>[`keygen_cli/scrypt_aes_validates_before_password_or_file_access`](tests/keygen_cli.rs) | Runs scrypt-aes as a subprocess and checks invalid sizes fail before password prompts or key replacement. | Passed | Passed |
| aligned blocks do not repeat in fixed large fixture<br>[`lib/keygen::cli::command_tests::argon2_aes::aligned_blocks_do_not_repeat_in_fixed_large_fixture`](src/keygen/command_tests.rs) | Scans the fixed large command fixture for repeated aligned blocks. | Passed | Passed |
| exact lengths replace existing files without temporary files<br>[`lib/keygen::cli::command_tests::argon2_aes::exact_lengths_replace_existing_files_without_temporary_files`](src/keygen/command_tests.rs) | Runs the selected command at boundary sizes and checks exact replacement length and temporary cleanup. | Passed | Passed |
| failed replacement cleans up and reports no success<br>[`lib/keygen::cli::command_tests::argon2_aes::failed_replacement_cleans_up_and_reports_no_success`](src/keygen/command_tests.rs) | Forces a command's key replacement to fail and checks cleanup and absence of a success message. | Passed | Passed |
| invalid input and password failures preserve existing key<br>[`lib/keygen::cli::command_tests::argon2_aes::invalid_input_and_password_failures_preserve_existing_key`](src/keygen/command_tests.rs) | Checks the selected generator preserves its old key after bad sizes, empty passwords, or mismatched confirmation. | Passed | Passed |
| large key is not a repeated short key<br>[`lib/keygen::cli::command_tests::argon2_aes::large_key_is_not_a_repeated_short_key`](src/keygen/command_tests.rs) | Checks a large generated key is not formed by repeating the corresponding short key. | Passed | Passed |
| matches original binary known answer<br>[`lib/keygen::cli::command_tests::argon2_aes::matches_original_binary_known_answer`](src/keygen/command_tests.rs) | Compares the selected generator command with the frozen original-binary byte vector. | Passed | Passed |
| multi buffer output matches independent expansion<br>[`lib/keygen::cli::command_tests::argon2_aes::multi_buffer_output_matches_independent_expansion`](src/keygen/command_tests.rs) | Checks a multi-MiB command output against separately implemented stream expansion. | Passed | Passed |
| original suite and configuration are frozen<br>[`lib/keygen::cli::command_tests::argon2_aes::original_suite_and_configuration_are_frozen`](src/keygen/command_tests.rs) | Pins each shipped command's suite, salt, pepper, and domain to its original values. | Passed | Passed |
| password and size change the output<br>[`lib/keygen::cli::command_tests::argon2_aes::password_and_size_change_the_output`](src/keygen/command_tests.rs) | For the selected command, changes password and size separately and checks output changes. | Passed | Passed |
| small and large keys are deterministic across directories<br>[`lib/keygen::cli::command_tests::argon2_aes::small_and_large_keys_are_deterministic_across_directories`](src/keygen/command_tests.rs) | Generates matching small and large fixtures in different directories and compares their bytes. | Passed | Passed |
| aligned blocks do not repeat in fixed large fixture<br>[`lib/keygen::cli::command_tests::argon2_blake3::aligned_blocks_do_not_repeat_in_fixed_large_fixture`](src/keygen/command_tests.rs) | Scans the fixed large command fixture for repeated aligned blocks. | Passed | Passed |
| exact lengths replace existing files without temporary files<br>[`lib/keygen::cli::command_tests::argon2_blake3::exact_lengths_replace_existing_files_without_temporary_files`](src/keygen/command_tests.rs) | Runs the selected command at boundary sizes and checks exact replacement length and temporary cleanup. | Passed | Passed |
| failed replacement cleans up and reports no success<br>[`lib/keygen::cli::command_tests::argon2_blake3::failed_replacement_cleans_up_and_reports_no_success`](src/keygen/command_tests.rs) | Forces a command's key replacement to fail and checks cleanup and absence of a success message. | Passed | Passed |
| invalid input and password failures preserve existing key<br>[`lib/keygen::cli::command_tests::argon2_blake3::invalid_input_and_password_failures_preserve_existing_key`](src/keygen/command_tests.rs) | Checks the selected generator preserves its old key after bad sizes, empty passwords, or mismatched confirmation. | Passed | Passed |
| large key is not a repeated short key<br>[`lib/keygen::cli::command_tests::argon2_blake3::large_key_is_not_a_repeated_short_key`](src/keygen/command_tests.rs) | Checks a large generated key is not formed by repeating the corresponding short key. | Passed | Passed |
| matches original binary known answer<br>[`lib/keygen::cli::command_tests::argon2_blake3::matches_original_binary_known_answer`](src/keygen/command_tests.rs) | Compares the selected generator command with the frozen original-binary byte vector. | Passed | Passed |
| multi buffer output matches independent expansion<br>[`lib/keygen::cli::command_tests::argon2_blake3::multi_buffer_output_matches_independent_expansion`](src/keygen/command_tests.rs) | Checks a multi-MiB command output against separately implemented stream expansion. | Passed | Passed |
| original suite and configuration are frozen<br>[`lib/keygen::cli::command_tests::argon2_blake3::original_suite_and_configuration_are_frozen`](src/keygen/command_tests.rs) | Pins each shipped command's suite, salt, pepper, and domain to its original values. | Passed | Passed |
| password and size change the output<br>[`lib/keygen::cli::command_tests::argon2_blake3::password_and_size_change_the_output`](src/keygen/command_tests.rs) | For the selected command, changes password and size separately and checks output changes. | Passed | Passed |
| small and large keys are deterministic across directories<br>[`lib/keygen::cli::command_tests::argon2_blake3::small_and_large_keys_are_deterministic_across_directories`](src/keygen/command_tests.rs) | Generates matching small and large fixtures in different directories and compares their bytes. | Passed | Passed |
| aligned blocks do not repeat in fixed large fixture<br>[`lib/keygen::cli::command_tests::argon2_chacha20::aligned_blocks_do_not_repeat_in_fixed_large_fixture`](src/keygen/command_tests.rs) | Scans the fixed large command fixture for repeated aligned blocks. | Passed | Passed |
| exact lengths replace existing files without temporary files<br>[`lib/keygen::cli::command_tests::argon2_chacha20::exact_lengths_replace_existing_files_without_temporary_files`](src/keygen/command_tests.rs) | Runs the selected command at boundary sizes and checks exact replacement length and temporary cleanup. | Passed | Passed |
| failed replacement cleans up and reports no success<br>[`lib/keygen::cli::command_tests::argon2_chacha20::failed_replacement_cleans_up_and_reports_no_success`](src/keygen/command_tests.rs) | Forces a command's key replacement to fail and checks cleanup and absence of a success message. | Passed | Passed |
| invalid input and password failures preserve existing key<br>[`lib/keygen::cli::command_tests::argon2_chacha20::invalid_input_and_password_failures_preserve_existing_key`](src/keygen/command_tests.rs) | Checks the selected generator preserves its old key after bad sizes, empty passwords, or mismatched confirmation. | Passed | Passed |
| large key is not a repeated short key<br>[`lib/keygen::cli::command_tests::argon2_chacha20::large_key_is_not_a_repeated_short_key`](src/keygen/command_tests.rs) | Checks a large generated key is not formed by repeating the corresponding short key. | Passed | Passed |
| matches original binary known answer<br>[`lib/keygen::cli::command_tests::argon2_chacha20::matches_original_binary_known_answer`](src/keygen/command_tests.rs) | Compares the selected generator command with the frozen original-binary byte vector. | Passed | Passed |
| multi buffer output matches independent expansion<br>[`lib/keygen::cli::command_tests::argon2_chacha20::multi_buffer_output_matches_independent_expansion`](src/keygen/command_tests.rs) | Checks a multi-MiB command output against separately implemented stream expansion. | Passed | Passed |
| original suite and configuration are frozen<br>[`lib/keygen::cli::command_tests::argon2_chacha20::original_suite_and_configuration_are_frozen`](src/keygen/command_tests.rs) | Pins each shipped command's suite, salt, pepper, and domain to its original values. | Passed | Passed |
| password and size change the output<br>[`lib/keygen::cli::command_tests::argon2_chacha20::password_and_size_change_the_output`](src/keygen/command_tests.rs) | For the selected command, changes password and size separately and checks output changes. | Passed | Passed |
| small and large keys are deterministic across directories<br>[`lib/keygen::cli::command_tests::argon2_chacha20::small_and_large_keys_are_deterministic_across_directories`](src/keygen/command_tests.rs) | Generates matching small and large fixtures in different directories and compares their bytes. | Passed | Passed |
| aligned blocks do not repeat in fixed large fixture<br>[`lib/keygen::cli::command_tests::pbkdf2_aes::aligned_blocks_do_not_repeat_in_fixed_large_fixture`](src/keygen/command_tests.rs) | Scans the fixed large command fixture for repeated aligned blocks. | Passed | Passed |
| exact lengths replace existing files without temporary files<br>[`lib/keygen::cli::command_tests::pbkdf2_aes::exact_lengths_replace_existing_files_without_temporary_files`](src/keygen/command_tests.rs) | Runs the selected command at boundary sizes and checks exact replacement length and temporary cleanup. | Passed | Passed |
| failed replacement cleans up and reports no success<br>[`lib/keygen::cli::command_tests::pbkdf2_aes::failed_replacement_cleans_up_and_reports_no_success`](src/keygen/command_tests.rs) | Forces a command's key replacement to fail and checks cleanup and absence of a success message. | Passed | Passed |
| invalid input and password failures preserve existing key<br>[`lib/keygen::cli::command_tests::pbkdf2_aes::invalid_input_and_password_failures_preserve_existing_key`](src/keygen/command_tests.rs) | Checks the selected generator preserves its old key after bad sizes, empty passwords, or mismatched confirmation. | Passed | Passed |
| large key is not a repeated short key<br>[`lib/keygen::cli::command_tests::pbkdf2_aes::large_key_is_not_a_repeated_short_key`](src/keygen/command_tests.rs) | Checks a large generated key is not formed by repeating the corresponding short key. | Passed | Passed |
| matches original binary known answer<br>[`lib/keygen::cli::command_tests::pbkdf2_aes::matches_original_binary_known_answer`](src/keygen/command_tests.rs) | Compares the selected generator command with the frozen original-binary byte vector. | Passed | Passed |
| multi buffer output matches independent expansion<br>[`lib/keygen::cli::command_tests::pbkdf2_aes::multi_buffer_output_matches_independent_expansion`](src/keygen/command_tests.rs) | Checks a multi-MiB command output against separately implemented stream expansion. | Passed | Passed |
| original suite and configuration are frozen<br>[`lib/keygen::cli::command_tests::pbkdf2_aes::original_suite_and_configuration_are_frozen`](src/keygen/command_tests.rs) | Pins each shipped command's suite, salt, pepper, and domain to its original values. | Passed | Passed |
| password and size change the output<br>[`lib/keygen::cli::command_tests::pbkdf2_aes::password_and_size_change_the_output`](src/keygen/command_tests.rs) | For the selected command, changes password and size separately and checks output changes. | Passed | Passed |
| small and large keys are deterministic across directories<br>[`lib/keygen::cli::command_tests::pbkdf2_aes::small_and_large_keys_are_deterministic_across_directories`](src/keygen/command_tests.rs) | Generates matching small and large fixtures in different directories and compares their bytes. | Passed | Passed |
| aligned blocks do not repeat in fixed large fixture<br>[`lib/keygen::cli::command_tests::scrypt_aes::aligned_blocks_do_not_repeat_in_fixed_large_fixture`](src/keygen/command_tests.rs) | Scans the fixed large command fixture for repeated aligned blocks. | Passed | Passed |
| exact lengths replace existing files without temporary files<br>[`lib/keygen::cli::command_tests::scrypt_aes::exact_lengths_replace_existing_files_without_temporary_files`](src/keygen/command_tests.rs) | Runs the selected command at boundary sizes and checks exact replacement length and temporary cleanup. | Passed | Passed |
| failed replacement cleans up and reports no success<br>[`lib/keygen::cli::command_tests::scrypt_aes::failed_replacement_cleans_up_and_reports_no_success`](src/keygen/command_tests.rs) | Forces a command's key replacement to fail and checks cleanup and absence of a success message. | Passed | Passed |
| invalid input and password failures preserve existing key<br>[`lib/keygen::cli::command_tests::scrypt_aes::invalid_input_and_password_failures_preserve_existing_key`](src/keygen/command_tests.rs) | Checks the selected generator preserves its old key after bad sizes, empty passwords, or mismatched confirmation. | Passed | Passed |
| large key is not a repeated short key<br>[`lib/keygen::cli::command_tests::scrypt_aes::large_key_is_not_a_repeated_short_key`](src/keygen/command_tests.rs) | Checks a large generated key is not formed by repeating the corresponding short key. | Passed | Passed |
| matches original binary known answer<br>[`lib/keygen::cli::command_tests::scrypt_aes::matches_original_binary_known_answer`](src/keygen/command_tests.rs) | Compares the selected generator command with the frozen original-binary byte vector. | Passed | Passed |
| multi buffer output matches independent expansion<br>[`lib/keygen::cli::command_tests::scrypt_aes::multi_buffer_output_matches_independent_expansion`](src/keygen/command_tests.rs) | Checks a multi-MiB command output against separately implemented stream expansion. | Passed | Passed |
| original suite and configuration are frozen<br>[`lib/keygen::cli::command_tests::scrypt_aes::original_suite_and_configuration_are_frozen`](src/keygen/command_tests.rs) | Pins each shipped command's suite, salt, pepper, and domain to its original values. | Passed | Passed |
| password and size change the output<br>[`lib/keygen::cli::command_tests::scrypt_aes::password_and_size_change_the_output`](src/keygen/command_tests.rs) | For the selected command, changes password and size separately and checks output changes. | Passed | Passed |
| small and large keys are deterministic across directories<br>[`lib/keygen::cli::command_tests::scrypt_aes::small_and_large_keys_are_deterministic_across_directories`](src/keygen/command_tests.rs) | Generates matching small and large fixtures in different directories and compares their bytes. | Passed | Passed |
| subcommands have distinct domains and outputs<br>[`lib/keygen::cli::command_tests::subcommands_have_distinct_domains_and_outputs`](src/keygen/command_tests.rs) | Checks the five command configurations use distinct domains and produce distinct keys. | Passed | Passed |
| canonically equivalent unicode passwords must match exact bytes<br>[`lib/keygen::cli::reliability_tests::canonically_equivalent_unicode_passwords_must_match_exact_bytes`](src/keygen/cli_reliability_tests.rs) | Rejects a composed character and its decomposed equivalent as mismatched passwords. | Passed | Passed |
| committed directory sync error is not reported as success<br>[`lib/keygen::cli::reliability_tests::committed_directory_sync_error_is_not_reported_as_success`](src/keygen/cli_reliability_tests.rs) | Passes a post-publication directory-sync error through the CLI and checks the commit is disclosed without a success message. | Passed | Passed |
| crlf size input is accepted<br>[`lib/keygen::cli::reliability_tests::crlf_size_input_is_accepted`](src/keygen/cli_reliability_tests.rs) | Checks Windows CRLF line endings parse as the requested size. | Passed | Passed |
| embedded nul password is not truncated<br>[`lib/keygen::cli::reliability_tests::embedded_nul_password_is_not_truncated`](src/keygen/cli_reliability_tests.rs) | Checks the prompt seam preserves password bytes on both sides of an embedded NUL. | Passed | Passed |
| final flush failure reports that the key was created<br>[`lib/keygen::cli::reliability_tests::final_flush_failure_reports_that_the_key_was_created`](src/keygen/cli_reliability_tests.rs) | Fails only the post-generation flush and checks the error explicitly states the key was created. | Passed | Passed |
| generating message flush failure prevents generation<br>[`lib/keygen::cli::reliability_tests::generating_message_flush_failure_prevents_generation`](src/keygen/cli_reliability_tests.rs) | Fails the pre-generation status flush after both passwords and checks the generator is never called. | Passed | Passed |
| generator panic never prints a success message<br>[`lib/keygen::cli::reliability_tests::generator_panic_never_prints_a_success_message`](src/keygen/cli_reliability_tests.rs) | Panics in the generator callback and verifies no created-key message appears. | Passed | Passed |
| infinite size input stops after 129 bytes<br>[`lib/keygen::cli::reliability_tests::infinite_size_input_stops_after_129_bytes`](src/keygen/cli_reliability_tests.rs) | Feeds an endless size stream and verifies reading stops at the rejection bound before prompting. | Passed | Passed |
| interrupted size reads are retried without losing digits<br>[`lib/keygen::cli::reliability_tests::interrupted_size_reads_are_retried_without_losing_digits`](src/keygen/cli_reliability_tests.rs) | Alternates Interrupted errors with single-byte reads and checks the complete size is parsed. | Passed | Passed |
| invalid utf8 size fails before password or generation<br>[`lib/keygen::cli::reliability_tests::invalid_utf8_size_fails_before_password_or_generation`](src/keygen/cli_reliability_tests.rs) | Provides invalid UTF-8 size bytes and checks InvalidData is returned before prompting. | Passed | Passed |
| maximum length leading zero input preserves the value<br>[`lib/keygen::cli::reliability_tests::maximum_length_leading_zero_input_preserves_the_value`](src/keygen/cli_reliability_tests.rs) | Parses a 128-byte size line with leading zeros and checks the resulting value is exactly one. | Passed | Passed |
| partial and interrupted console writes preserve complete status<br>[`lib/keygen::cli::reliability_tests::partial_and_interrupted_console_writes_preserve_complete_status`](src/keygen/cli_reliability_tests.rs) | Combines short writes with Interrupted errors and checks complete status text without password disclosure. | Passed | Passed |
| read failure after size digits does not use a partial number<br>[`lib/keygen::cli::reliability_tests::read_failure_after_size_digits_does_not_use_a_partial_number`](src/keygen/cli_reliability_tests.rs) | Fails input after valid digits and checks the CLI does not generate using the partial size. | Passed | Passed |
| size at eof without newline is accepted<br>[`lib/keygen::cli::reliability_tests::size_at_eof_without_newline_is_accepted`](src/keygen/cli_reliability_tests.rs) | Checks a valid size is accepted when EOF follows immediately without a newline. | Passed | Passed |
| size prompt flush failure does not consume input<br>[`lib/keygen::cli::reliability_tests::size_prompt_flush_failure_does_not_consume_input`](src/keygen/cli_reliability_tests.rs) | Fails flushing the size prompt and checks no size bytes are consumed. | Passed | Passed |
| size reader leaves following lines unconsumed<br>[`lib/keygen::cli::reliability_tests::size_reader_leaves_following_lines_unconsumed`](src/keygen/cli_reliability_tests.rs) | Checks size reading stops at the first newline instead of consuming later lines. | Passed | Passed |
| success status reaches a buffered writer before returning<br>[`lib/keygen::cli::reliability_tests::success_status_reaches_a_buffered_writer_before_returning`](src/keygen/cli_reliability_tests.rs) | Uses a real BufWriter and checks successful generation flushes its final status before returning. | Passed | Passed |
| unicode whitespace is counted in bytes at the input limit<br>[`lib/keygen::cli::reliability_tests::unicode_whitespace_is_counted_in_bytes_at_the_input_limit`](src/keygen/cli_reliability_tests.rs) | Checks multibyte whitespace obeys the 128-byte size-input bound, not a character count. | Passed | Passed |
| whitespace only password is preserved as nonempty<br>[`lib/keygen::cli::reliability_tests::whitespace_only_password_is_preserved_as_nonempty`](src/keygen/cli_reliability_tests.rs) | Checks a nonempty whitespace-only password is preserved rather than silently trimmed. | Passed | Passed |
| zero length console write fails before reading size<br>[`lib/keygen::cli::reliability_tests::zero_length_console_write_fails_before_reading_size`](src/keygen/cli_reliability_tests.rs) | Uses a zero-progress console writer and checks WriteZero occurs before size input is consumed. | Passed | Passed |
| cli accepts maximum without narrowing the byte count<br>[`lib/keygen::cli::tests::cli_accepts_maximum_without_narrowing_the_byte_count`](src/keygen/cli_tests.rs) | Passes the 20-billion-byte maximum through the prompt seam without truncating the integer or writing a huge file. | Passed | Passed |
| cli bounds size input before asking for password<br>[`lib/keygen::cli::tests::cli_bounds_size_input_before_asking_for_password`](src/keygen/cli_tests.rs) | Checks the exact 128-byte size-line limit and rejects a longer line before password entry. | Passed | Passed |
| cli checks size before asking for password<br>[`lib/keygen::cli::tests::cli_checks_size_before_asking_for_password`](src/keygen/cli_tests.rs) | Supplies invalid sizes and verifies neither passwords nor generation are requested. | Passed | Passed |
| cli distinguishes post commit status failure<br>[`lib/keygen::cli::tests::cli_distinguishes_post_commit_status_failure`](src/keygen/cli_tests.rs) | Fails the final status write and checks the error explicitly acknowledges completed key creation. | Passed | Passed |
| cli errors and output do not reveal password<br>[`lib/keygen::cli::tests::cli_errors_and_output_do_not_reveal_password`](src/keygen/cli_tests.rs) | Checks mismatch diagnostics and console output do not contain the synthetic password. | Passed | Passed |
| cli preserves unicode and whitespace password exactly<br>[`lib/keygen::cli::tests::cli_preserves_unicode_and_whitespace_password_exactly`](src/keygen/cli_tests.rs) | Passes Unicode and surrounding spaces through the prompt without trimming or normalization. | Passed | Passed |
| cli prompts for password exactly twice<br>[`lib/keygen::cli::tests::cli_prompts_for_password_exactly_twice`](src/keygen/cli_tests.rs) | Checks that one successful generation reads a password and one confirmation. | Passed | Passed |
| cli propagates first password read error<br>[`lib/keygen::cli::tests::cli_propagates_first_password_read_error`](src/keygen/cli_tests.rs) | Fails initial hidden password input and verifies the I/O error stops generation. | Passed | Passed |
| cli propagates generation failure without success message<br>[`lib/keygen::cli::tests::cli_propagates_generation_failure_without_success_message`](src/keygen/cli_tests.rs) | Injects a generator failure and verifies no success message appears. | Passed | Passed |
| cli propagates second password read error<br>[`lib/keygen::cli::tests::cli_propagates_second_password_read_error`](src/keygen/cli_tests.rs) | Fails confirmation input and verifies the error propagates after exactly two prompts. | Passed | Passed |
| cli rejects empty password before generation<br>[`lib/keygen::cli::tests::cli_rejects_empty_password_before_generation`](src/keygen/cli_tests.rs) | Checks an empty password cannot reach the generator. | Passed | Passed |
| cli rejects password mismatch before generation<br>[`lib/keygen::cli::tests::cli_rejects_password_mismatch_before_generation`](src/keygen/cli_tests.rs) | Checks different password and confirmation bytes prevent generation. | Passed | Passed |
| cli reports eof before password or generation<br>[`lib/keygen::cli::tests::cli_reports_eof_before_password_or_generation`](src/keygen/cli_tests.rs) | Ends size input immediately and checks failure occurs before any password prompt. | Passed | Passed |
| cli success uses exact size and password<br>[`lib/keygen::cli::tests::cli_success_uses_exact_size_and_password`](src/keygen/cli_tests.rs) | Checks a successful prompt passes the exact requested size and password to generation. | Passed | Passed |
| non unicode arguments fail without panicking or defaulting<br>[`lib/keygen::cli::tests::non_unicode_arguments_fail_without_panicking_or_defaulting`](src/keygen/cli_tests.rs) | Checks native non-Unicode generator arguments fail cleanly without a default suite. | Passed | Passed |
| complete data is synced before replacement and directory after<br>[`lib/keygen::storage::tests::complete_data_is_synced_before_replacement_and_directory_after`](src/keygen/storage_tests.rs) | Checks production, complete-file sync, publication, and directory sync occur in that order. | Passed | Passed |
| directory sync failure retains the committed key<br>[`lib/keygen::storage::tests::directory_sync_failure_retains_the_committed_key`](src/keygen/storage_tests.rs) | Fails key directory sync after publication and checks the new key remains with an explicit committed error. | Passed | Passed |
| directory sync panic does not remove committed key<br>[`lib/keygen::storage::tests::directory_sync_panic_does_not_remove_committed_key`](src/keygen/storage_tests.rs) | Panics after key publication and verifies cleanup cannot delete the committed key. | Passed | Passed |
| file as parent fails without truncating it<br>[`lib/keygen::storage::tests::file_as_parent_fails_without_truncating_it`](src/keygen/storage_tests.rs) | Uses a regular file as the key directory and checks rejection preserves that file. | Passed | Passed |
| file sync failure does not publish a new key<br>[`lib/keygen::storage::tests::file_sync_failure_does_not_publish_a_new_key`](src/keygen/storage_tests.rs) | Fails key file sync and checks no new key or temporary file remains. | Passed | Passed |
| file sync failure preserves existing key and cleans temporary<br>[`lib/keygen::storage::tests::file_sync_failure_preserves_existing_key_and_cleans_temporary`](src/keygen/storage_tests.rs) | Injects a storage-full error during key sync and checks old key preservation and temporary cleanup. | Passed | Passed |
| file sync panic preserves old key and cleans temporary<br>[`lib/keygen::storage::tests::file_sync_panic_preserves_old_key_and_cleans_temporary`](src/keygen/storage_tests.rs) | Panics during key sync and checks old bytes and temporary cleanup. | Passed | Passed |
| key replacement preserves hard links and platform reader policy<br>[`lib/keygen::storage::tests::key_replacement_preserves_hard_links_and_platform_reader_policy`](src/keygen/storage_tests.rs) | Checks old hard links keep their bytes; Linux open readers survive replacement, while Windows safely refuses and succeeds after closing the reader. | Passed | Passed |
| key symlink is replaced without modifying its target<br>[`lib/keygen::storage::tests::key_symlink_is_replaced_without_modifying_its_target`](src/keygen/storage_tests.rs) | On Unix, checks key generation replaces the symlink entry while preserving its referent. | Not included | Passed |
| missing directory fails before producer runs<br>[`lib/keygen::storage::tests::missing_directory_fails_before_producer_runs`](src/keygen/storage_tests.rs) | Attempts key creation under a missing directory and checks no writer callback or directory creation occurs. | Passed | Passed |
| nested failure cannot remove another active temporary<br>[`lib/keygen::storage::tests::nested_failure_cannot_remove_another_active_temporary`](src/keygen/storage_tests.rs) | Runs a failing key transaction inside an active one and checks it cannot delete the outer temporary file. | Passed | Passed |
| persist failure skips directory sync and preserves destination<br>[`lib/keygen::storage::tests::persist_failure_skips_directory_sync_and_preserves_destination`](src/keygen/storage_tests.rs) | Blocks key persistence with a populated directory and checks no directory sync or deletion follows. | Passed | Passed |
| producer failure skips both sync stages<br>[`lib/keygen::storage::tests::producer_failure_skips_both_sync_stages`](src/keygen/storage_tests.rs) | Fails key production and checks neither file sync nor directory sync is called. | Passed | Passed |
| stale temporary files survive success and failure<br>[`lib/keygen::storage::tests::stale_temporary_files_survive_success_and_failure`](src/keygen/storage_tests.rs) | Leaves another process's key temporary sentinel in place and checks both successful and failed transactions preserve it. | Passed | Passed |
| unicode directory supports atomic key replacement<br>[`lib/keygen::storage::tests::unicode_directory_supports_atomic_key_replacement`](src/keygen/storage_tests.rs) | Replaces a key inside a directory containing spaces and Unicode and checks exact bytes and cleanup. | Passed | Passed |
| windows delete lock preserves existing key and cleans temporary<br>[`lib/keygen::storage::tests::windows_delete_lock_preserves_existing_key_and_cleans_temporary`](src/keygen/storage_tests.rs) | Blocks key deletion sharing and checks failed replacement preserves old bytes and removes its temporary. | Passed | Not included |
| aes ctr stream matches direct counter encryption<br>[`lib/keygen::tests::aes_ctr_stream_matches_direct_counter_encryption`](src/keygen/tests.rs) | Compares the AES stream with separately encrypted big-endian counter blocks. | Passed | Passed |
| aligned blocks do not repeat in large fixture<br>[`lib/keygen::tests::aligned_blocks_do_not_repeat_in_large_fixture`](src/keygen/tests.rs) | Checks a large AES output for repeated aligned blocks. | Passed | Passed |
| aligned blocks stay unique across many keys<br>[`lib/keygen::tests::aligned_blocks_stay_unique_across_many_keys`](src/keygen/tests.rs) | Checks aligned-block uniqueness across a collection of synthetic AES keys. | Passed | Passed |
| all five suites are domain separated with cheap kdfs<br>[`lib/keygen::tests::all_five_suites_are_domain_separated_with_cheap_kdfs`](src/keygen/tests.rs) | Uses inexpensive test KDFs to confirm the five constructions produce distinct output. | Passed | Passed |
| atomic writer cleans up when destination cannot be replaced<br>[`lib/keygen::tests::atomic_writer_cleans_up_when_destination_cannot_be_replaced`](src/keygen/tests.rs) | Forces key publication to fail and verifies temporary cleanup and preservation of the destination. | Passed | Passed |
| atomic writer creates the requested file<br>[`lib/keygen::tests::atomic_writer_creates_the_requested_file`](src/keygen/tests.rs) | Writes through the key transaction and checks the published file has exactly the requested bytes. | Passed | Passed |
| atomic writer preserves existing file on failure<br>[`lib/keygen::tests::atomic_writer_preserves_existing_file_on_failure`](src/keygen/tests.rs) | Injects a key-writing error and verifies the previous file survives unchanged. | Passed | Passed |
| atomic writer removes temporary file after failure<br>[`lib/keygen::tests::atomic_writer_removes_temporary_file_after_failure`](src/keygen/tests.rs) | Checks a failed key write leaves no temporary artifact. | Passed | Passed |
| atomic writer removes temporary file during unwind<br>[`lib/keygen::tests::atomic_writer_removes_temporary_file_during_unwind`](src/keygen/tests.rs) | Panics during key writing and checks stack unwinding removes the temporary file. | Passed | Passed |
| atomic writer replaces and truncates existing file<br>[`lib/keygen::tests::atomic_writer_replaces_and_truncates_existing_file`](src/keygen/tests.rs) | Replaces a longer key and verifies no trailing old bytes remain. | Passed | Passed |
| blake3 keyed xof stream matches known answer<br>[`lib/keygen::tests::blake3_keyed_xof_stream_matches_known_answer`](src/keygen/tests.rs) | Checks the keyed BLAKE3 stream against a fixed expected byte vector. | Passed | Passed |
| chacha20 fails closed at its counter limit<br>[`lib/keygen::tests::chacha20_fails_closed_at_its_counter_limit`](src/keygen/tests.rs) | Seeks to ChaCha20's last supported block and checks an extra byte fails instead of wrapping. | Passed | Passed |
| chacha20 zero key stream matches known answer<br>[`lib/keygen::tests::chacha20_zero_key_stream_matches_known_answer`](src/keygen/tests.rs) | Compares a zero-key ChaCha20 stream with a fixed known-answer vector. | Passed | Passed |
| counter exhaustion does not publish a partial key<br>[`lib/keygen::tests::counter_exhaustion_does_not_publish_a_partial_key`](src/keygen/tests.rs) | Exhausts ChaCha20 after one block inside a real key transaction and checks rollback and cleanup. | Passed | Passed |
| derived key is deterministic<br>[`lib/keygen::tests::derived_key_is_deterministic`](src/keygen/tests.rs) | Derives twice from identical test inputs and compares the keys. | Passed | Passed |
| different aes keys change stream<br>[`lib/keygen::tests::different_aes_keys_change_stream`](src/keygen/tests.rs) | Checks different synthetic AES keys produce different streams. | Passed | Passed |
| different domain changes derived key<br>[`lib/keygen::tests::different_domain_changes_derived_key`](src/keygen/tests.rs) | Changes only the configuration domain and checks the derived key changes. | Passed | Passed |
| different lengths do not share a stream prefix<br>[`lib/keygen::tests::different_lengths_do_not_share_a_stream_prefix`](src/keygen/tests.rs) | Checks requested length participates in derivation rather than merely truncating one common key stream. | Passed | Passed |
| different password changes derived key<br>[`lib/keygen::tests::different_password_changes_derived_key`](src/keygen/tests.rs) | Changes only the password and checks the derived key changes. | Passed | Passed |
| different pepper changes derived key<br>[`lib/keygen::tests::different_pepper_changes_derived_key`](src/keygen/tests.rs) | Changes only the application pepper and checks the derived key changes. | Passed | Passed |
| different salt changes derived key<br>[`lib/keygen::tests::different_salt_changes_derived_key`](src/keygen/tests.rs) | Changes only the application salt and checks the derived key changes. | Passed | Passed |
| effective salt binds requested length<br>[`lib/keygen::tests::effective_salt_binds_requested_length`](src/keygen/tests.rs) | Checks the framed Argon2 salt changes when requested output length changes. | Passed | Passed |
| effective salt is unambiguous<br>[`lib/keygen::tests::effective_salt_is_unambiguous`](src/keygen/tests.rs) | Checks length-prefixed domain and salt fields cannot collide through simple concatenation. | Passed | Passed |
| empty configuration fields are rejected<br>[`lib/keygen::tests::empty_configuration_fields_are_rejected`](src/keygen/tests.rs) | Rejects empty salt, pepper, and domain fields. | Passed | Passed |
| error messages never include password material<br>[`lib/keygen::tests::error_messages_never_include_password_material`](src/keygen/tests.rs) | Checks generator error formatting does not disclose synthetic password material. | Passed | Passed |
| every kdf distinguishes unicode normalization forms<br>[`lib/keygen::tests::every_kdf_distinguishes_unicode_normalization_forms`](src/keygen/tests.rs) | Checks Argon2, scrypt, and PBKDF2 distinguish composed and decomposed password bytes. | Passed | Passed |
| every kdf preserves embedded nul password bytes<br>[`lib/keygen::tests::every_kdf_preserves_embedded_nul_password_bytes`](src/keygen/tests.rs) | Checks all three KDFs use the complete password, including bytes after NUL. | Passed | Passed |
| every stream generator handles partial writes<br>[`lib/keygen::tests::every_stream_generator_handles_partial_writes`](src/keygen/tests.rs) | Limits each write and checks AES, ChaCha20, and BLAKE3 still emit complete correct output. | Passed | Passed |
| every stream generator is independent of buffer partitioning<br>[`lib/keygen::tests::every_stream_generator_is_independent_of_buffer_partitioning`](src/keygen/tests.rs) | Checks all stream generators emit identical bytes with different internal chunk sizes. | Passed | Passed |
| every stream generator propagates write failure<br>[`lib/keygen::tests::every_stream_generator_propagates_write_failure`](src/keygen/tests.rs) | Injects output errors into each stream generator and checks propagation. | Passed | Passed |
| every stream generator rejects invalid size and zero buffer before writing<br>[`lib/keygen::tests::every_stream_generator_rejects_invalid_size_and_zero_buffer_before_writing`](src/keygen/tests.rs) | Checks all stream generators reject invalid lengths and zero buffers before emitting bytes. | Passed | Passed |
| every stream generator reports write zero<br>[`lib/keygen::tests::every_stream_generator_reports_write_zero`](src/keygen/tests.rs) | Checks all stream generators reject a writer returning zero rather than looping forever. | Passed | Passed |
| every stream generator retries interrupted writes<br>[`lib/keygen::tests::every_stream_generator_retries_interrupted_writes`](src/keygen/tests.rs) | Injects Interrupted errors into each generator's writer and checks retry preserves output. | Passed | Passed |
| every stream generator uses bounded production writes<br>[`lib/keygen::tests::every_stream_generator_uses_bounded_production_writes`](src/keygen/tests.rs) | Measures every generator's write requests and checks the production one-MiB bound. | Passed | Passed |
| every stream generator writes exact boundary lengths<br>[`lib/keygen::tests::every_stream_generator_writes_exact_boundary_lengths`](src/keygen/tests.rs) | Checks each generator emits exactly the requested length around important block and buffer boundaries. | Passed | Passed |
| every stream preserves existing key on disk failure across buffer boundaries<br>[`lib/keygen::tests::every_stream_preserves_existing_key_on_disk_failure_across_buffer_boundaries`](src/keygen/tests.rs) | Injects disk failures into every expansion stream around buffer boundaries and checks transactional rollback. | Passed | Passed |
| every suite rejects invalid input without touching existing file<br>[`lib/keygen::tests::every_suite_rejects_invalid_input_without_touching_existing_file`](src/keygen/tests.rs) | Checks all five suites reject invalid inputs while preserving an existing key. | Passed | Passed |
| fixed key stream has prefix property<br>[`lib/keygen::tests::fixed_key_stream_has_prefix_property`](src/keygen/tests.rs) | Checks stream expansion with a fixed derived key has consistent prefixes at different lengths. | Passed | Passed |
| framed salt capacity overflow is rejected without allocating<br>[`lib/keygen::tests::framed_salt_capacity_overflow_is_rejected_without_allocating`](src/keygen/tests.rs) | Passes impossible salt capacities and checks checked arithmetic or allocation rejection prevents a panic. | Passed | Passed |
| framed salts keep all eight size bytes above four gib<br>[`lib/keygen::tests::framed_salts_keep_all_eight_size_bytes_above_four_gib`](src/keygen/tests.rs) | Checks both salt formats retain all eight big-endian length bytes above 4 GiB and at the maximum. | Passed | Passed |
| generated file is named key key and has exact raw content<br>[`lib/keygen::tests::generated_file_is_named_key_key_and_has_exact_raw_content`](src/keygen/tests.rs) | Checks key generation publishes key.key with the exact raw stream and no added header. | Passed | Passed |
| generated file replaces a longer existing key<br>[`lib/keygen::tests::generated_file_replaces_a_longer_existing_key`](src/keygen/tests.rs) | Generates a shorter key over a longer one and checks exact replacement without an old tail. | Passed | Passed |
| generated key file has owner only permissions<br>[`lib/keygen::tests::generated_key_file_has_owner_only_permissions`](src/keygen/tests.rs) | On Unix, checks newly generated keys are readable and writable only by their owner. | Not included | Passed |
| hundreds of stream lengths match one shot reference<br>[`lib/keygen::tests::hundreds_of_stream_lengths_match_one_shot_reference`](src/keygen/tests.rs) | Compares hundreds of chunked AES stream lengths with a one-shot reference. | Passed | Passed |
| invalid configuration does not touch existing file<br>[`lib/keygen::tests::invalid_configuration_does_not_touch_existing_file`](src/keygen/tests.rs) | Supplies invalid generator configuration and checks an existing key is untouched. | Passed | Passed |
| invalid generation does not touch existing file<br>[`lib/keygen::tests::invalid_generation_does_not_touch_existing_file`](src/keygen/tests.rs) | Supplies invalid generation lengths and checks the existing key survives. | Passed | Passed |
| invalid stream sizes are rejected before writing<br>[`lib/keygen::tests::invalid_stream_sizes_are_rejected_before_writing`](src/keygen/tests.rs) | Checks AES stream length validation occurs before output is written. | Passed | Passed |
| library rejects empty password without touching existing file<br>[`lib/keygen::tests::library_rejects_empty_password_without_touching_existing_file`](src/keygen/tests.rs) | Calls the generator library with an empty password and checks the old key survives. | Passed | Passed |
| maximum size counter and chunk arithmetic is safe<br>[`lib/keygen::tests::maximum_size_counter_and_chunk_arithmetic_is_safe`](src/keygen/tests.rs) | Checks counter and chunk arithmetic at 20 billion bytes without allocating or writing that much data. | Passed | Passed |
| new kdfs reject empty passwords and zero pbkdf2 rounds<br>[`lib/keygen::tests::new_kdfs_reject_empty_passwords_and_zero_pbkdf2_rounds`](src/keygen/tests.rs) | Checks scrypt and PBKDF2 input validation, including rejection of zero PBKDF2 iterations. | Passed | Passed |
| parse size accepts exact boundaries and whitespace<br>[`lib/keygen::tests::parse_size_accepts_exact_boundaries_and_whitespace`](src/keygen/tests.rs) | Checks one byte and the maximum are accepted with permitted surrounding whitespace. | Passed | Passed |
| parse size accepts hundreds of valid values<br>[`lib/keygen::tests::parse_size_accepts_hundreds_of_valid_values`](src/keygen/tests.rs) | Parses a broad set of valid decimal byte counts. | Passed | Passed |
| parse size accepts values near maximum<br>[`lib/keygen::tests::parse_size_accepts_values_near_maximum`](src/keygen/tests.rs) | Checks decimal values near the maximum are accepted exactly. | Passed | Passed |
| parse size rejects empty and zero<br>[`lib/keygen::tests::parse_size_rejects_empty_and_zero`](src/keygen/tests.rs) | Rejects empty, whitespace-only, and zero byte counts. | Passed | Passed |
| parse size rejects every common unit suffix<br>[`lib/keygen::tests::parse_size_rejects_every_common_unit_suffix`](src/keygen/tests.rs) | Rejects size suffixes such as KB, MB, and GiB; sizes must be plain byte counts. | Passed | Passed |
| parse size rejects non decimal notation<br>[`lib/keygen::tests::parse_size_rejects_non_decimal_notation`](src/keygen/tests.rs) | Rejects signs, separators, fractions, exponents, and other non-decimal input. | Passed | Passed |
| parse size rejects values above maximum and u64<br>[`lib/keygen::tests::parse_size_rejects_values_above_maximum_and_u64`](src/keygen/tests.rs) | Rejects oversized values, including decimal integers that overflow u64. | Passed | Passed |
| password is unicode and whitespace sensitive<br>[`lib/keygen::tests::password_is_unicode_and_whitespace_sensitive`](src/keygen/tests.rs) | Checks the KDF treats Unicode and whitespace password bytes as significant. | Passed | Passed |
| pbkdf2 derivation is deterministic and input sensitive<br>[`lib/keygen::tests::pbkdf2_derivation_is_deterministic_and_input_sensitive`](src/keygen/tests.rs) | Checks PBKDF2 repeatability and sensitivity to password, size, salt, pepper, and domain. | Passed | Passed |
| pbkdf2 sha256 matches known vector<br>[`lib/keygen::tests::pbkdf2_sha256_matches_known_vector`](src/keygen/tests.rs) | Compares PBKDF2-HMAC-SHA-256 with a fixed known-answer vector. | Passed | Passed |
| peppered effective salt is unambiguous and binds every input<br>[`lib/keygen::tests::peppered_effective_salt_is_unambiguous_and_binds_every_input`](src/keygen/tests.rs) | Checks peppered salt framing distinguishes field boundaries and binds all configuration fields and length. | Passed | Passed |
| production kdf parameters are pinned<br>[`lib/keygen::tests::production_kdf_parameters_are_pinned`](src/keygen/tests.rs) | Pins production memory, iteration, lane, and related KDF settings. | Passed | Passed |
| production stream writes are bounded to one mibibyte<br>[`lib/keygen::tests::production_stream_writes_are_bounded_to_one_mibibyte`](src/keygen/tests.rs) | Checks production AES output requests stay within one MiB. | Passed | Passed |
| production suite outputs match known answers<br>[`lib/keygen::tests::production_suite_outputs_match_known_answers`](src/keygen/tests.rs) | Checks production-cost derivation and expansion for all suites against frozen expected output. | Passed | Passed |
| scrypt derivation is deterministic and input sensitive<br>[`lib/keygen::tests::scrypt_derivation_is_deterministic_and_input_sensitive`](src/keygen/tests.rs) | Checks scrypt repeatability and sensitivity to password, size, salt, pepper, and domain. | Passed | Passed |
| scrypt matches rfc 7914 first vector<br>[`lib/keygen::tests::scrypt_matches_rfc_7914_first_vector`](src/keygen/tests.rs) | Compares the scrypt primitive with RFC 7914's first known-answer vector. | Passed | Passed |
| stream counter exhaustion after progress preserves the exact prefix<br>[`lib/keygen::tests::stream_counter_exhaustion_after_progress_preserves_the_exact_prefix`](src/keygen/tests.rs) | Exhausts ChaCha20 after a complete block and checks the wrapper returns CounterExhausted without outputting invalid bytes. | Passed | Passed |
| stream does not repeat common digest periods<br>[`lib/keygen::tests::stream_does_not_repeat_common_digest_periods`](src/keygen/tests.rs) | Checks AES expansion does not repeat short digest-sized patterns. | Passed | Passed |
| stream halves are different<br>[`lib/keygen::tests::stream_halves_are_different`](src/keygen/tests.rs) | Checks the two halves of a fixed generated AES stream are not identical. | Passed | Passed |
| stream handles partial writes<br>[`lib/keygen::tests::stream_handles_partial_writes`](src/keygen/tests.rs) | Checks AES stream writing completes when the writer accepts only short fragments. | Passed | Passed |
| stream is independent of buffer partitioning<br>[`lib/keygen::tests::stream_is_independent_of_buffer_partitioning`](src/keygen/tests.rs) | Checks AES stream bytes are independent of internal buffer size. | Passed | Passed |
| stream propagates write failure<br>[`lib/keygen::tests::stream_propagates_write_failure`](src/keygen/tests.rs) | Fails an AES output writer and verifies the error propagates. | Passed | Passed |
| stream reports write zero<br>[`lib/keygen::tests::stream_reports_write_zero`](src/keygen/tests.rs) | Checks AES output rejects zero-progress writes. | Passed | Passed |
| stream retries interrupted writes<br>[`lib/keygen::tests::stream_retries_interrupted_writes`](src/keygen/tests.rs) | Checks AES output retries Interrupted writes without losing data. | Passed | Passed |
| stream writes exact lengths around important boundaries<br>[`lib/keygen::tests::stream_writes_exact_lengths_around_important_boundaries`](src/keygen/tests.rs) | Checks AES output length around cipher-block and output-buffer boundaries. | Passed | Passed |
| suite display names are stable and distinct<br>[`lib/keygen::tests::suite_display_names_are_stable_and_distinct`](src/keygen/tests.rs) | Pins human-readable suite labels and checks they remain distinct. | Passed | Passed |
| suites select the expected stream generators<br>[`lib/keygen::tests::suites_select_the_expected_stream_generators`](src/keygen/tests.rs) | Checks each suite selects its intended AES, ChaCha20, or BLAKE3 expansion. | Passed | Passed |
| validate size has exact boundaries<br>[`lib/keygen::tests::validate_size_has_exact_boundaries`](src/keygen/tests.rs) | Checks the numeric size API accepts exactly the documented inclusive range. | Passed | Passed |
| zero buffer size is rejected before writing<br>[`lib/keygen::tests::zero_buffer_size_is_rejected_before_writing`](src/keygen/tests.rs) | Checks AES stream generation rejects a zero-sized buffer before writing anything. | Passed | Passed |
| control characters<br>[`lib/xor::arguments::control_characters`](src/xor/tests/arguments.rs) | Rejects control characters in XOR filenames on both platforms. | Passed | Passed |
| device names<br>[`lib/xor::arguments::device_names`](src/xor/tests/arguments.rs) | Rejects reserved Windows device names, including case and extension variants. | Passed | Passed |
| empty and dot names<br>[`lib/xor::arguments::empty_and_dot_names`](src/xor/tests/arguments.rs) | Rejects empty, dot, and parent-directory filenames. | Passed | Passed |
| help aliases<br>[`lib/xor::arguments::help_aliases`](src/xor/tests/arguments.rs) | Checks the XOR parser recognizes both short and long help options. | Passed | Passed |
| help describes the complete new contract<br>[`lib/xor::arguments::help_describes_the_complete_new_contract`](src/xor/tests/arguments.rs) | Checks XOR help documents location, key length, in-place mode, and file-safety rules. | Passed | Passed |
| in place is explicit and has one filename<br>[`lib/xor::arguments::in_place_is_explicit_and_has_one_filename`](src/xor/tests/arguments.rs) | Checks in-place parsing requires its explicit option and exactly one source filename. | Passed | Passed |
| in place terminator preserves literal option and native names<br>[`lib/xor::arguments::in_place_terminator_preserves_literal_option_and_native_names`](src/xor/tests/arguments.rs) | Checks the option terminator preserves option-like and OS-native in-place filenames. | Passed | Passed |
| in place uses the same filename validation<br>[`lib/xor::arguments::in_place_uses_the_same_filename_validation`](src/xor/tests/arguments.rs) | Checks in-place arguments cannot bypass ordinary XOR filename restrictions. | Passed | Passed |
| internal temporary names<br>[`lib/xor::arguments::internal_temporary_names`](src/xor/tests/arguments.rs) | Rejects filenames using the reserved XOR temporary prefix. | Passed | Passed |
| lone dash is a filename<br>[`lib/xor::arguments::lone_dash_is_a_filename`](src/xor/tests/arguments.rs) | Checks a single dash is accepted as a literal filename. | Passed | Passed |
| native non unicode names are preserved<br>[`lib/xor::arguments::native_non_unicode_names_are_preserved`](src/xor/tests/arguments.rs) | Checks parsing retains native filename bytes or UTF-16 units without lossy conversion. | Passed | Passed |
| ordinary device like names are allowed<br>[`lib/xor::arguments::ordinary_device_like_names_are_allowed`](src/xor/tests/arguments.rs) | Checks harmless names resembling Windows devices are still accepted. | Passed | Passed |
| rejects all removed modes<br>[`lib/xor::arguments::rejects_all_removed_modes`](src/xor/tests/arguments.rs) | Checks the parser rejects removed XOR modes. | Passed | Passed |
| rejects wrong argument counts including old cli<br>[`lib/xor::arguments::rejects_wrong_argument_counts_including_old_cli`](src/xor/tests/arguments.rs) | Checks XOR parsing rejects incorrect filename counts and obsolete command forms. | Passed | Passed |
| spaces and unicode are preserved<br>[`lib/xor::arguments::spaces_and_unicode_are_preserved`](src/xor/tests/arguments.rs) | Checks the parser preserves filename spaces and Unicode characters. | Passed | Passed |
| terminator allows option like names<br>[`lib/xor::arguments::terminator_allows_option_like_names`](src/xor/tests/arguments.rs) | Checks double dash makes later option-like arguments literal filenames. | Passed | Passed |
| trailing spaces and dots<br>[`lib/xor::arguments::trailing_spaces_and_dots`](src/xor/tests/arguments.rs) | Rejects filenames ending in spaces or dots to prevent ambiguous Windows aliases. | Passed | Passed |
| two filenames select fixed key processing<br>[`lib/xor::arguments::two_filenames_select_fixed_key_processing`](src/xor/tests/arguments.rs) | Checks ordinary XOR parsing selects two filenames with the fixed key.key policy. | Passed | Passed |
| unix paths<br>[`lib/xor::arguments::unix_paths`](src/xor/tests/arguments.rs) | Rejects Unix absolute, parent, and subdirectory paths where plain filenames are required. | Passed | Passed |
| version aliases<br>[`lib/xor::arguments::version_aliases`](src/xor/tests/arguments.rs) | Checks the XOR parser recognizes both short and long version flags. | Passed | Passed |
| windows paths<br>[`lib/xor::arguments::windows_paths`](src/xor/tests/arguments.rs) | Rejects Windows drive, UNC, and separator-containing paths where plain filenames are required. | Passed | Passed |
| windows special characters<br>[`lib/xor::arguments::windows_special_characters`](src/xor/tests/arguments.rs) | Rejects Windows-special characters in filenames on both platforms. | Passed | Passed |
| concurrent readers only observe complete versions<br>[`lib/xor::atomic_file::in_place_tests::concurrent_readers_only_observe_complete_versions`](src/xor/tests/in_place.rs) | Reads concurrently during atomic in-place replacement and rejects any partially written version. | Passed | Passed |
| file sync failure after complete transformation preserves source<br>[`lib/xor::atomic_file::in_place_tests::file_sync_failure_after_complete_transformation_preserves_source`](src/xor/tests/in_place.rs) | Fails sync after a complete in-place transformation and checks the original source survives. | Passed | Passed |
| in place preserves access bits and clears special bits<br>[`lib/xor::atomic_file::in_place_tests::in_place_preserves_access_bits_and_clears_special_bits`](src/xor/tests/in_place.rs) | On Unix, checks in-place output retains access permissions while clearing special mode bits. | Not included | Passed |
| long parent paths support atomic overwrite<br>[`lib/xor::atomic_file::in_place_tests::long_parent_paths_support_atomic_overwrite`](src/xor/tests/in_place.rs) | Exercises in-place atomic replacement under a long directory path. | Passed | Passed |
| missing or replaced source path fails before transformation<br>[`lib/xor::atomic_file::in_place_tests::missing_or_replaced_source_path_fails_before_transformation`](src/xor/tests/in_place.rs) | Changes the in-place source path before processing and verifies the transaction refuses to transform it. | Passed | Passed |
| original survives partial work and existing readers keep old bytes<br>[`lib/xor::atomic_file::in_place_tests::original_survives_partial_work_and_existing_readers_keep_old_bytes`](src/xor/tests/in_place.rs) | Checks in-place replacement keeps the original visible during production and old readers retain original bytes afterward. | Passed | Passed |
| panic during transformation preserves source and releases handle<br>[`lib/xor::atomic_file::in_place_tests::panic_during_transformation_preserves_source_and_releases_handle`](src/xor/tests/in_place.rs) | Panics during in-place processing and checks rollback, temporary cleanup, and handle release. | Passed | Passed |
| protected source is refused before transformation<br>[`lib/xor::atomic_file::in_place_tests::protected_source_is_refused_before_transformation`](src/xor/tests/in_place.rs) | Checks in-place transactions reject a protected source identity before transforming bytes. | Passed | Passed |
| same length source edit during transformation is detected<br>[`lib/xor::atomic_file::in_place_tests::same_length_source_edit_during_transformation_is_detected`](src/xor/tests/in_place.rs) | On Unix, edits source bytes without changing length and checks in-place change detection. | Not included | Passed |
| short read after a full block preserves source and releases handle<br>[`lib/xor::atomic_file::in_place_tests::short_read_after_a_full_block_preserves_source_and_releases_handle`](src/xor/tests/in_place.rs) | Ends in-place input after partial progress and checks rollback and handle release. | Passed | Passed |
| source changed after reading but before publication is detected<br>[`lib/xor::atomic_file::in_place_tests::source_changed_after_reading_but_before_publication_is_detected`](src/xor/tests/in_place.rs) | On Unix, changes source after transformation and checks the final pre-publication validation refuses it. | Not included | Passed |
| source replaced during transformation is not overwritten<br>[`lib/xor::atomic_file::in_place_tests::source_replaced_during_transformation_is_not_overwritten`](src/xor/tests/in_place.rs) | Replaces the in-place source path during transformation and checks the replacement file survives. | Passed | Passed |
| source stays write protected after transformation<br>[`lib/xor::atomic_file::in_place_tests::source_stays_write_protected_after_transformation`](src/xor/tests/in_place.rs) | On Windows, verifies the source remains protected against writers between transformation and publication. | Passed | Not included |
| write failure after partial progress preserves source<br>[`lib/xor::atomic_file::in_place_tests::write_failure_after_partial_progress_preserves_source`](src/xor/tests/in_place.rs) | Fails in-place output after partial writing and checks original source preservation. | Passed | Passed |
| destination appearing during file sync is not overwritten<br>[`lib/xor::atomic_file::tests::destination_appearing_during_file_sync_is_not_overwritten`](src/xor/tests/transactions.rs) | Creates a competing output during file sync and checks the final recheck preserves it. | Passed | Passed |
| directory appearing before publish is preserved<br>[`lib/xor::atomic_file::tests::directory_appearing_before_publish_is_preserved`](src/xor/tests/transactions.rs) | Creates a directory at the destination during processing and checks it is not replaced. | Passed | Passed |
| directory sync failure reports that output was published<br>[`lib/xor::atomic_file::tests::directory_sync_failure_reports_that_output_was_published`](src/xor/tests/transactions.rs) | Fails directory sync after XOR rename and checks the error discloses publication and preserves the new output. | Passed | Passed |
| existing output is unchanged until success<br>[`lib/xor::atomic_file::tests::existing_output_is_unchanged_until_success`](src/xor/tests/transactions.rs) | Inspects the old output during production and checks it changes only after transaction completion. | Passed | Passed |
| failed rename does not sync directory or remove destination<br>[`lib/xor::atomic_file::tests::failed_rename_does_not_sync_directory_or_remove_destination`](src/xor/tests/transactions.rs) | Forces XOR rename failure and checks no post-publication sync or destination deletion occurs. | Passed | Passed |
| failed windows rename preserves output and cleans temp<br>[`lib/xor::atomic_file::tests::failed_windows_rename_preserves_output_and_cleans_temp`](src/xor/tests/transactions.rs) | Blocks Windows output rename and checks old output and cleanup. | Passed | Not included |
| failure after partial writes preserves existing output<br>[`lib/xor::atomic_file::tests::failure_after_partial_writes_preserves_existing_output`](src/xor/tests/transactions.rs) | Injects a transaction error after substantial writing and checks old output bytes survive. | Passed | Passed |
| failure does not create new output<br>[`lib/xor::atomic_file::tests::failure_does_not_create_new_output`](src/xor/tests/transactions.rs) | Fails a transaction targeting a missing output and checks the target remains absent. | Passed | Passed |
| file sync failure keeps new output absent<br>[`lib/xor::atomic_file::tests::file_sync_failure_keeps_new_output_absent`](src/xor/tests/transactions.rs) | Fails XOR file sync and checks a previously absent output remains absent. | Passed | Passed |
| file sync failure preserves existing output<br>[`lib/xor::atomic_file::tests::file_sync_failure_preserves_existing_output`](src/xor/tests/transactions.rs) | Injects a storage-full error during XOR sync and checks the existing output survives. | Passed | Passed |
| file sync panic cleans temporary without replacing output<br>[`lib/xor::atomic_file::tests::file_sync_panic_cleans_temporary_without_replacing_output`](src/xor/tests/transactions.rs) | Panics during XOR sync and checks unwinding cleans temporary data without replacing output. | Passed | Passed |
| identity distinguishes files and recognizes hard links<br>[`lib/xor::atomic_file::tests::identity_distinguishes_files_and_recognizes_hard_links`](src/xor/tests/transactions.rs) | Checks native file identities distinguish separate files and identify hard-link aliases. | Passed | Passed |
| independent temporary files have unique names<br>[`lib/xor::atomic_file::tests::independent_temporary_files_have_unique_names`](src/xor/tests/transactions.rs) | Creates overlapping transactions and checks their temporary filenames differ. | Passed | Passed |
| missing parent is not created<br>[`lib/xor::atomic_file::tests::missing_parent_is_not_created`](src/xor/tests/transactions.rs) | Checks XOR does not silently create a missing destination directory. | Passed | Passed |
| new output appears only after success<br>[`lib/xor::atomic_file::tests::new_output_appears_only_after_success`](src/xor/tests/transactions.rs) | Checks a missing XOR output stays absent throughout production and appears only after success. | Passed | Passed |
| new unix output is private<br>[`lib/xor::atomic_file::tests::new_unix_output_is_private`](src/xor/tests/transactions.rs) | Checks a new Unix XOR output has owner-only permissions. | Not included | Passed |
| output becoming read only fails without data loss<br>[`lib/xor::atomic_file::tests::output_becoming_read_only_fails_without_data_loss`](src/xor/tests/transactions.rs) | Marks output read-only during processing and checks final validation refuses replacement. | Passed | Passed |
| panic unwinds without deleting existing output<br>[`lib/xor::atomic_file::tests::panic_unwinds_without_deleting_existing_output`](src/xor/tests/transactions.rs) | Panics during ordinary XOR production and checks the prior output survives cleanup. | Passed | Passed |
| producer failure never reaches file sync<br>[`lib/xor::atomic_file::tests::producer_failure_never_reaches_file_sync`](src/xor/tests/transactions.rs) | Fails XOR production and checks incomplete output is never synced or published. | Passed | Passed |
| protected hard link appearing during sync is not overwritten<br>[`lib/xor::atomic_file::tests::protected_hard_link_appearing_during_sync_is_not_overwritten`](src/xor/tests/transactions.rs) | Adds a protected-key hard link at the output during sync and checks the final identity check rejects it. | Passed | Passed |
| read only output is not replaced<br>[`lib/xor::atomic_file::tests::read_only_output_is_not_replaced`](src/xor/tests/transactions.rs) | Checks a read-only output is rejected before replacement. | Passed | Passed |
| rechecks hard link identity before publish<br>[`lib/xor::atomic_file::tests::rechecks_hard_link_identity_before_publish`](src/xor/tests/transactions.rs) | Changes destination identity during processing and checks protected hard links are caught before publication. | Passed | Passed |
| refuses destination edited in place during processing<br>[`lib/xor::atomic_file::tests::refuses_destination_edited_in_place_during_processing`](src/xor/tests/transactions.rs) | On Unix, edits the destination through its existing inode and checks replacement is refused. | Not included | Passed |
| refuses destination permission changes during processing<br>[`lib/xor::atomic_file::tests::refuses_destination_permission_changes_during_processing`](src/xor/tests/transactions.rs) | On Unix, changes output permissions during processing and checks final validation detects it. | Not included | Passed |
| refuses destination removed during processing<br>[`lib/xor::atomic_file::tests::refuses_destination_removed_during_processing`](src/xor/tests/transactions.rs) | Deletes output during production and checks the transaction refuses to recreate it. | Passed | Passed |
| refuses destination replaced during processing<br>[`lib/xor::atomic_file::tests::refuses_destination_replaced_during_processing`](src/xor/tests/transactions.rs) | Replaces output with another file during production and checks the competing file survives. | Passed | Passed |
| refuses new destination created during processing<br>[`lib/xor::atomic_file::tests::refuses_new_destination_created_during_processing`](src/xor/tests/transactions.rs) | Creates a competing destination during production and checks it is not overwritten. | Passed | Passed |
| refuses same length destination edit during processing<br>[`lib/xor::atomic_file::tests::refuses_same_length_destination_edit_during_processing`](src/xor/tests/transactions.rs) | On Unix, changes output bytes at constant length and checks metadata validation rejects replacement. | Not included | Passed |
| stale temporary is preserved across success and failure<br>[`lib/xor::atomic_file::tests::stale_temporary_is_preserved_across_success_and_failure`](src/xor/tests/transactions.rs) | Leaves another process's XOR temporary sentinel in place and checks both successful and failed transactions preserve it. | Passed | Passed |
| unix access bits are preserved without special bits<br>[`lib/xor::atomic_file::tests::unix_access_bits_are_preserved_without_special_bits`](src/xor/tests/transactions.rs) | Checks ordinary Unix XOR replacement retains access bits while clearing special mode bits. | Not included | Passed |
| unrelated hard link to old output is not modified<br>[`lib/xor::atomic_file::tests::unrelated_hard_link_to_old_output_is_not_modified`](src/xor/tests/transactions.rs) | Checks replacing one output directory entry leaves another hard link to the old file unchanged. | Passed | Passed |
| windows active output writer is refused before production<br>[`lib/xor::atomic_file::tests::windows_active_output_writer_is_refused_before_production`](src/xor/tests/transactions.rs) | Opens output for writing and checks the Windows transaction fails before producing data. | Passed | Not included |
| windows device and stream names are refused<br>[`lib/xor::atomic_file::tests::windows_device_and_stream_names_are_refused`](src/xor/tests/transactions.rs) | Checks Windows device and alternate-data-stream names cannot become transaction destinations. | Passed | Not included |
| windows output denies writers until publication<br>[`lib/xor::atomic_file::tests::windows_output_denies_writers_until_publication`](src/xor/tests/transactions.rs) | Checks a Windows output remains protected against writers through final publication. | Passed | Not included |
| direct api cannot bypass name validation<br>[`lib/xor::file_checks::direct_api_cannot_bypass_name_validation`](src/xor/tests/file_checks.rs) | Calls the XOR library directly with invalid filenames and verifies the same validation applies. | Passed | Passed |
| growth detected after writes does not publish new output<br>[`lib/xor::file_checks::growth_detected_after_writes_does_not_publish_new_output`](src/xor/tests/file_checks.rs) | Detects source growth after writing and checks the transaction does not publish a new output. | Passed | Passed |
| length changes are detected in both directions<br>[`lib/xor::file_checks::length_changes_are_detected_in_both_directions`](src/xor/tests/file_checks.rs) | Checks metadata validation detects both source shrinkage and growth. | Passed | Passed |
| plain file check rejects directories<br>[`lib/xor::file_checks::plain_file_check_rejects_directories`](src/xor/tests/file_checks.rs) | Checks regular-file validation excludes directory metadata. | Passed | Passed |
| same length timestamp changes are detected<br>[`lib/xor::file_checks::same_length_timestamp_changes_are_detected`](src/xor/tests/file_checks.rs) | Checks metadata validation catches changes even when file length stays constant. | Passed | Passed |
| short key fails before opening executable or creating temporary<br>[`lib/xor::file_checks::short_key_fails_before_opening_executable_or_creating_temporary`](src/xor/tests/file_checks.rs) | Checks a short-key error is detected before executable access or temporary output creation. | Passed | Passed |
| stable metadata passes<br>[`lib/xor::file_checks::stable_metadata_passes`](src/xor/tests/file_checks.rs) | Checks unchanged file metadata passes change detection. | Passed | Passed |
| streaming failure after full block preserves destination<br>[`lib/xor::file_checks::streaming_failure_after_full_block_preserves_destination`](src/xor/tests/file_checks.rs) | Fails streaming after one full XOR block and checks the transaction preserves old output. | Passed | Passed |
| unix device is rejected before open<br>[`lib/xor::file_checks::unix_device_is_rejected_before_open`](src/xor/tests/file_checks.rs) | Checks the Unix device /dev/null is refused as an input before opening it. | Not included | Passed |
| windows input handle denies writes and removal until closed<br>[`lib/xor::file_checks::windows_input_handle_denies_writes_and_removal_until_closed`](src/xor/tests/file_checks.rs) | Checks open XOR inputs deny writers and deletion until the protecting handle closes. | Passed | Not included |
| all byte values<br>[`lib/xor::streams::all_byte_values`](src/xor/tests/streams.rs) | XORs complementary byte values and checks every result is 255. | Passed | Passed |
| boundaries and multiple buffers match independent oracle<br>[`lib/xor::streams::boundaries_and_multiple_buffers_match_independent_oracle`](src/xor/tests/streams.rs) | Compares XOR at small and multi-buffer lengths with an independent byte-by-byte oracle. | Passed | Passed |
| data read errors propagate after a full buffer<br>[`lib/xor::streams::data_read_errors_propagate_after_a_full_buffer`](src/xor/tests/streams.rs) | Fails source reading after one complete XOR buffer and checks the source-specific error and output prefix. | Passed | Passed |
| empty a and nonempty key<br>[`lib/xor::streams::empty_a_and_nonempty_key`](src/xor/tests/streams.rs) | Checks an empty source produces no output despite a nonempty key stream. | Passed | Passed |
| empty input never reads the key stream<br>[`lib/xor::streams::empty_input_never_reads_the_key_stream`](src/xor/tests/streams.rs) | Uses a key reader that panics if touched and proves empty XOR consumes no key bytes. | Passed | Passed |
| empty key fails for nonempty a<br>[`lib/xor::streams::empty_key_fails_for_nonempty_a`](src/xor/tests/streams.rs) | Checks nonempty XOR with an empty key fails with unexpected EOF. | Passed | Passed |
| equal inputs produce zeros<br>[`lib/xor::streams::equal_inputs_produce_zeros`](src/xor/tests/streams.rs) | Checks XOR of a byte sequence with itself is all zeros across a buffer boundary. | Passed | Passed |
| errors after partial writes propagate<br>[`lib/xor::streams::errors_after_partial_writes_propagate`](src/xor/tests/streams.rs) | Fails XOR output after partial progress and checks the write error is returned. | Passed | Passed |
| every pair of byte values matches the xor oracle<br>[`lib/xor::streams::every_pair_of_byte_values_matches_the_xor_oracle`](src/xor/tests/streams.rs) | Checks all 65,536 possible pairs of input and key bytes against direct XOR. | Passed | Passed |
| final eof check propagates read errors<br>[`lib/xor::streams::final_eof_check_propagates_read_errors`](src/xor/tests/streams.rs) | Fails the final source EOF probe and checks XOR reports the error. | Passed | Passed |
| growth of a fails even from empty<br>[`lib/xor::streams::growth_of_a_fails_even_from_empty`](src/xor/tests/streams.rs) | Checks source growth detection for both empty and nonempty expected inputs. | Passed | Passed |
| key read errors propagate after a full buffer<br>[`lib/xor::streams::key_read_errors_propagate_after_a_full_buffer`](src/xor/tests/streams.rs) | Fails key reading after a full XOR buffer and checks key-specific error reporting. | Passed | Passed |
| key short by one fails<br>[`lib/xor::streams::key_short_by_one_fails`](src/xor/tests/streams.rs) | Checks a key one byte shorter than the source produces unexpected EOF. | Passed | Passed |
| key tail is never read<br>[`lib/xor::streams::key_tail_is_never_read`](src/xor/tests/streams.rs) | Uses a reader that fails beyond the needed key prefix and checks XOR never touches the unused tail. | Passed | Passed |
| known vector<br>[`lib/xor::streams::known_vector`](src/xor/tests/streams.rs) | Checks XOR against a small, fixed expected byte vector. | Passed | Passed |
| longer key uses only its prefix<br>[`lib/xor::streams::longer_key_uses_only_its_prefix`](src/xor/tests/streams.rs) | Checks extra key bytes do not alter the output. | Passed | Passed |
| maximum u64 length uses a bounded first read<br>[`lib/xor::streams::maximum_u64_length_uses_a_bounded_first_read`](src/xor/tests/streams.rs) | Requests a virtual u64::MAX XOR length, fails its first read, and verifies that read is only 64 KiB. | Passed | Passed |
| randomized oracles and round trips<br>[`lib/xor::streams::randomized_oracles_and_round_trips`](src/xor/tests/streams.rs) | Runs deterministic pseudo-random XOR fixtures against an independent oracle and checks double-XOR restoration. | Passed | Passed |
| repeated interruptions during final eof check are retried<br>[`lib/xor::streams::repeated_interruptions_during_final_eof_check_are_retried`](src/xor/tests/streams.rs) | Injects seven Interrupted errors into the final EOF probe and checks the eighth read completes successfully. | Passed | Passed |
| short and interrupted reads use all bytes<br>[`lib/xor::streams::short_and_interrupted_reads_use_all_bytes`](src/xor/tests/streams.rs) | Splits source and key reads and injects interruptions, checking no bytes are lost. | Passed | Passed |
| short and interrupted writes are completed<br>[`lib/xor::streams::short_and_interrupted_writes_are_completed`](src/xor/tests/streams.rs) | Limits XOR output writes and injects interruptions, checking complete correct output. | Passed | Passed |
| short key after full block does not write partial second block<br>[`lib/xor::streams::short_key_after_full_block_does_not_write_partial_second_block`](src/xor/tests/streams.rs) | Ends the key during the second block and checks only the complete first block reached output. | Passed | Passed |
| shortened a fails<br>[`lib/xor::streams::shortened_a_fails`](src/xor/tests/streams.rs) | Checks XOR detects a source shorter than its declared length. | Passed | Passed |
| simultaneous short interrupted reads and writes preserve alignment<br>[`lib/xor::streams::simultaneous_short_interrupted_reads_and_writes_preserve_alignment`](src/xor/tests/streams.rs) | Interrupts and fragments both inputs and output together across multiple buffers and checks exact alignment. | Passed | Passed |
| source failure does not consume key for an incomplete block<br>[`lib/xor::streams::source_failure_does_not_consume_key_for_an_incomplete_block`](src/xor/tests/streams.rs) | Fails source input mid-block and checks key consumption and output stop at the previous complete block. | Passed | Passed |
| two empty inputs<br>[`lib/xor::streams::two_empty_inputs`](src/xor/tests/streams.rs) | Checks two empty XOR streams produce empty output. | Passed | Passed |
| virtual key larger than four gib is not eagerly read<br>[`lib/xor::streams::virtual_key_larger_than_four_gib_is_not_eagerly_read`](src/xor/tests/streams.rs) | Models a huge key and verifies XOR reads only the needed prefix in bounded chunks. | Passed | Passed |
| write failure stops before reading another block<br>[`lib/xor::streams::write_failure_stops_before_reading_another_block`](src/xor/tests/streams.rs) | Fails output in the second XOR block and verifies neither input advances into the third. | Passed | Passed |
| zero key preserves all bytes<br>[`lib/xor::streams::zero_key_preserves_all_bytes`](src/xor/tests/streams.rs) | Checks XOR against an all-zero key preserves source bytes across a buffer boundary. | Passed | Passed |
| zero writes fail instead of spinning<br>[`lib/xor::streams::zero_writes_fail_instead_of_spinning`](src/xor/tests/streams.rs) | Checks zero-progress XOR output writes fail promptly rather than spinning. | Passed | Passed |
| all old processing flags and three file syntax fail<br>[`xor_cli/all_old_processing_flags_and_three_file_syntax_fail`](tests/xor_cli.rs) | Runs the executable with removed options and the old three-file syntax and expects usage failure. | Passed | Passed |
| case aliases of inputs are rejected<br>[`xor_cli/case_aliases_of_inputs_are_rejected`](tests/xor_cli.rs) | On Windows, refuses output names differing only in case from a protected input. | Passed | Not included |
| device stream and ambiguous names fail on every platform<br>[`xor_cli/device_stream_and_ambiguous_names_fail_on_every_platform`](tests/xor_cli.rs) | Exercises unsafe device, alternate-stream, and ambiguous filenames through the real executable. | Passed | Passed |
| directory a is refused<br>[`xor_cli/directory_a_is_refused`](tests/xor_cli.rs) | Uses a directory as XOR input and checks safe rejection. | Passed | Passed |
| directory key is refused<br>[`xor_cli/directory_key_is_refused`](tests/xor_cli.rs) | Uses a directory as key.key and checks XOR rejects it. | Passed | Passed |
| directory output and its contents are preserved<br>[`xor_cli/directory_output_and_its_contents_are_preserved`](tests/xor_cli.rs) | Uses a populated directory as XOR output and verifies it remains intact. | Passed | Passed |
| empty a accepts empty or longer key<br>[`xor_cli/empty_a_accepts_empty_or_longer_key`](tests/xor_cli.rs) | Runs empty-file XOR with empty and nonempty keys and checks an empty output. | Passed | Passed |
| empty key does not create output<br>[`xor_cli/empty_key_does_not_create_output`](tests/xor_cli.rs) | Uses a zero-length key with nonempty input and checks no output appears. | Passed | Passed |
| executable directory can contain spaces and unicode<br>[`xor_cli/executable_directory_can_contain_spaces_and_unicode`](tests/xor_cli.rs) | Launches a copied binary from a directory with spaces and Unicode and checks XOR succeeds there. | Passed | Passed |
| executable symlink uses real executable directory<br>[`xor_cli/executable_symlink_uses_real_executable_directory`](tests/xor_cli.rs) | Launches through a Unix symlink and checks XOR uses the resolved binary's directory. | Not included | Passed |
| help and version work without input files<br>[`xor_cli/help_and_version_work_without_input_files`](tests/xor_cli.rs) | Runs XOR help and version without any input files. | Passed | Passed |
| directory input is preserved<br>[`xor_cli/in_place::directory_input_is_preserved`](tests/cases/in_place.rs) | Attempts in-place XOR on a directory and checks its contents survive. | Passed | Passed |
| executable and its hard links are never overwritten<br>[`xor_cli/in_place::executable_and_its_hard_links_are_never_overwritten`](tests/cases/in_place.rs) | Attempts in-place replacement of the executable and its aliases and checks protection. | Passed | Passed |
| executable directory controls in place target<br>[`xor_cli/in_place::executable_directory_controls_in_place_target`](tests/cases/in_place.rs) | Launches from another working directory and checks in-place XOR selects the executable's neighboring file. | Passed | Passed |
| in place symlinks and dangling links are refused<br>[`xor_cli/in_place::in_place_symlinks_and_dangling_links_are_refused`](tests/cases/in_place.rs) | Checks in-place XOR refuses both live and dangling source symlinks. | Not included | Passed |
| invalid in place arguments do not modify files<br>[`xor_cli/in_place::invalid_in_place_arguments_do_not_modify_files`](tests/cases/in_place.rs) | Runs invalid in-place command forms and checks existing files remain unchanged. | Passed | Passed |
| key and its hard links are never overwritten<br>[`xor_cli/in_place::key_and_its_hard_links_are_never_overwritten`](tests/cases/in_place.rs) | Attempts in-place XOR on key.key and its hard links and checks key protection. | Passed | Passed |
| known vector and status<br>[`xor_cli/in_place::known_vector_and_status`](tests/cases/in_place.rs) | Runs XOR on known bytes and checks exact output plus the reported byte count. | Passed | Passed |
| missing input is not created<br>[`xor_cli/in_place::missing_input_is_not_created`](tests/cases/in_place.rs) | Attempts in-place XOR on a missing source and checks it remains absent. | Passed | Passed |
| missing short or empty key preserves original<br>[`xor_cli/in_place::missing_short_or_empty_key_preserves_original`](tests/cases/in_place.rs) | Tries in-place XOR with absent, short, and empty keys and checks the original source survives. | Passed | Passed |
| other hard links keep original bytes<br>[`xor_cli/in_place::other_hard_links_keep_original_bytes`](tests/cases/in_place.rs) | Checks in-place XOR replaces one directory entry without rewriting the original file's other hard links. | Passed | Passed |
| readonly original is preserved and readonly key is usable<br>[`xor_cli/in_place::readonly_original_is_preserved_and_readonly_key_is_usable`](tests/cases/in_place.rs) | Checks in-place XOR preserves a read-only source while still allowing a read-only key. | Passed | Passed |
| round trips at empty and buffer boundaries with longer keys<br>[`xor_cli/in_place::round_trips_at_empty_and_buffer_boundaries_with_longer_keys`](tests/cases/in_place.rs) | Checks in-place double-XOR restoration for empty and boundary-sized files with surplus key bytes. | Passed | Passed |
| spaces unicode dashes and native names work in place<br>[`xor_cli/in_place::spaces_unicode_dashes_and_native_names_work_in_place`](tests/cases/in_place.rs) | Checks in-place XOR handles spaces, Unicode, dashes, and native non-Unicode names. | Passed | Passed |
| windows active writer is refused<br>[`xor_cli/in_place::windows_active_writer_is_refused`](tests/cases/in_place.rs) | Keeps an in-place source open for writing and checks safe refusal. | Passed | Not included |
| windows case aliases preserve key protection<br>[`xor_cli/in_place::windows_case_aliases_preserve_key_protection`](tests/cases/in_place.rs) | Checks case-insensitive Windows aliases cannot bypass in-place key protection. | Passed | Not included |
| windows locked input rename failure preserves original<br>[`xor_cli/in_place::windows_locked_input_rename_failure_preserves_original`](tests/cases/in_place.rs) | Holds a rename-blocking source handle during in-place XOR and checks the original survives. | Passed | Not included |
| independent parallel invocations<br>[`xor_cli/independent_parallel_invocations`](tests/xor_cli.rs) | Runs multiple independent XOR processes concurrently and checks their results and cleanup. | Passed | Passed |
| key fifo is rejected without opening it<br>[`xor_cli/key_fifo_is_rejected_without_opening_it`](tests/xor_cli.rs) | Uses a Unix FIFO as key.key and checks rejection without a blocking open. | Not included | Passed |
| key symlinks cannot escape executable directory<br>[`xor_cli/key_symlinks_cannot_escape_executable_directory`](tests/xor_cli.rs) | Checks XOR refuses a key symlink pointing outside the executable directory. | Not included | Passed |
| known vector and status<br>[`xor_cli/known_vector_and_status`](tests/xor_cli.rs) | Runs XOR on known bytes and checks exact output plus the reported byte count. | Passed | Passed |
| launch directory is ignored for both inputs and output<br>[`xor_cli/launch_directory_is_ignored_for_both_inputs_and_output`](tests/xor_cli.rs) | Runs XOR from another directory and verifies all file selection remains beside the executable. | Passed | Passed |
| length check precedes destination inspection<br>[`xor_cli/length_check_precedes_destination_inspection`](tests/xor_cli.rs) | Combines a short key with an invalid destination and checks key-length preflight fails first. | Passed | Passed |
| locked output rename fails without data loss<br>[`xor_cli/locked_output_rename_fails_without_data_loss`](tests/xor_cli.rs) | Holds a Windows rename-blocking output handle and checks failed publication preserves old data. | Passed | Not included |
| longer key produces exactly a length<br>[`xor_cli/longer_key_produces_exactly_a_length`](tests/xor_cli.rs) | Uses an oversized key and checks XOR output is exactly the source length. | Passed | Passed |
| missing a preserves output<br>[`xor_cli/missing_a_preserves_output`](tests/xor_cli.rs) | Runs XOR with a missing source and checks an existing output remains intact. | Passed | Passed |
| missing key is required for empty a too<br>[`xor_cli/missing_key_is_required_for_empty_a_too`](tests/xor_cli.rs) | Checks even an empty XOR source fails when key.key is missing. | Passed | Passed |
| missing key preserves output even with another b file<br>[`xor_cli/missing_key_preserves_output_even_with_another_b_file`](tests/xor_cli.rs) | Checks XOR never substitutes a differently named file for a missing key and preserves output. | Passed | Passed |
| native non unicode filenames end to end<br>[`xor_cli/native_non_unicode_filenames_end_to_end`](tests/xor_cli.rs) | Runs XOR through the real binary using native filenames that cannot be represented as Unicode. | Passed | Passed |
| never falls back to a in working directory<br>[`xor_cli/never_falls_back_to_a_in_working_directory`](tests/xor_cli.rs) | Places a source only in the launch directory and verifies XOR refuses to use it. | Passed | Passed |
| never falls back to key in working directory<br>[`xor_cli/never_falls_back_to_key_in_working_directory`](tests/xor_cli.rs) | Places a key only in the launch directory and verifies XOR refuses to use it. | Passed | Passed |
| output cannot be a hard link to either input<br>[`xor_cli/output_cannot_be_a_hard_link_to_either_input`](tests/xor_cli.rs) | Uses hard-link aliases of source or key as output and verifies identity-based rejection. | Passed | Passed |
| output cannot be either input<br>[`xor_cli/output_cannot_be_either_input`](tests/xor_cli.rs) | Checks ordinary XOR cannot overwrite its source or key. | Passed | Passed |
| output cannot replace executable or its hard link<br>[`xor_cli/output_cannot_replace_executable_or_its_hard_link`](tests/xor_cli.rs) | Uses the executable or a hard-link alias as output and checks executable protection. | Passed | Passed |
| output symlinks are refused including dangling links<br>[`xor_cli/output_symlinks_are_refused_including_dangling_links`](tests/xor_cli.rs) | Checks XOR refuses live and dangling output symlinks and preserves their targets. | Not included | Passed |
| parallel fixture copy and launch are reliable<br>[`xor_cli/parallel_fixture_copy_and_launch_are_reliable`](tests/xor_cli.rs) | Stresses concurrent executable copying and process launch to guard against fixture races. | Passed | Passed |
| readonly inputs can be processed<br>[`xor_cli/readonly_inputs_can_be_processed`](tests/xor_cli.rs) | Runs XOR using read-only source and key files and checks successful output. | Passed | Passed |
| readonly output is preserved<br>[`xor_cli/readonly_output_is_preserved`](tests/xor_cli.rs) | Runs the executable against a read-only destination and checks its original contents survive. | Passed | Passed |
| rejects absolute paths even to files beside executable<br>[`xor_cli/rejects_absolute_paths_even_to_files_beside_executable`](tests/xor_cli.rs) | Checks absolute paths are refused even when their files happen to be beside the binary. | Passed | Passed |
| rejects bad counts unknown options and empty names<br>[`xor_cli/rejects_bad_counts_unknown_options_and_empty_names`](tests/xor_cli.rs) | Runs malformed XOR argument counts, unknown flags, and empty filenames and expects usage failure. | Passed | Passed |
| rejects parent paths subdirectories and both separator styles<br>[`xor_cli/rejects_parent_paths_subdirectories_and_both_separator_styles`](tests/xor_cli.rs) | Rejects traversal and subdirectory paths using either platform's path separators. | Passed | Passed |
| replacement removes old trailing bytes<br>[`xor_cli/replacement_removes_old_trailing_bytes`](tests/xor_cli.rs) | Runs shorter XOR output over a longer file and checks no old suffix remains. | Passed | Passed |
| round trip across multiple buffers<br>[`xor_cli/round_trip_across_multiple_buffers`](tests/xor_cli.rs) | Runs XOR twice on data spanning multiple buffers and checks exact byte restoration. | Passed | Passed |
| same file can supply a and key<br>[`xor_cli/same_file_can_supply_a_and_key`](tests/xor_cli.rs) | Uses key.key as the source too and checks a separate output contains zeros. | Passed | Passed |
| short key preserves existing output<br>[`xor_cli/short_key_preserves_existing_output`](tests/xor_cli.rs) | Runs XOR with insufficient key bytes and checks an existing output survives. | Passed | Passed |
| spaces unicode and option like filenames<br>[`xor_cli/spaces_unicode_and_option_like_filenames`](tests/xor_cli.rs) | Runs real XOR commands using spaces, Unicode, and names resembling options. | Passed | Passed |
| symlink inputs are refused even inside the same directory<br>[`xor_cli/symlink_inputs_are_refused_even_inside_the_same_directory`](tests/xor_cli.rs) | Checks XOR refuses source symlinks even when their targets are beside the executable. | Not included | Passed |
| windows input open for writing fails safely<br>[`xor_cli/windows_input_open_for_writing_fails_safely`](tests/xor_cli.rs) | Runs XOR with an actively writable input and checks safe failure. | Passed | Not included |
| windows output open for writing is preserved<br>[`xor_cli/windows_output_open_for_writing_is_preserved`](tests/xor_cli.rs) | Keeps output open for writing and checks the executable refuses replacement without damage. | Passed | Not included |

### What these results establish

The tests exercise compatibility vectors, real production KDFs, stream boundaries,
injected I/O and synchronization failures, atomic publication, cleanup, command
dispatch, and native platform behavior. Tests that cover multiple generators or
multiple inputs remain one test result each.

They do not simulate physical power loss, write a full 20 GB key, or verify hidden
password entry on every real terminal. Successful automated generation uses an
injected password reader. Filesystem durability and ACL behavior still depend on
the deployment environment. These are local results; no hosted CI run is claimed.

This is a saved validation snapshot. To refresh the HTML report, run both platform
scripts, then
`powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-readme-report.ps1`.
Copy the refreshed validation section into this Markdown appendix. The report
builder refuses incomplete runs, mismatched totals, missing explanations, or
application source files newer than the recorded runs.

<!-- END VALIDATION REPORT -->
