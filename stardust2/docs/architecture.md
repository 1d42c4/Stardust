# Architecture and audit map

The project uses a small model/view/controller application layer with independent
key-generation and XOR domains. This is a local CLI, so it does not need an MVC
framework, event bus, dependency-injection container, or a database.

```text
main.rs -> app::run_cli
             |
             +-> app/model.rs       command routing and exit-code policy
             +-> app/view.rs        combined help
             +-> app/controller.rs  explicit paths, I/O and dispatch
                       |
                       +-> keygen/cli.rs -> keygen/controller.rs
                       |       |                 |
                       |       +-> commands.rs   +-> derivation.rs
                       |                         +-> storage.rs (transaction)
                       |                         +-> stream.rs (writer callback)
                       +-> xor/controller.rs
                               +-> model.rs (filename/argument rules)
                               +-> stream.rs
                               +-> file_safety.rs + atomic_file.rs
```

| Concern | Files to inspect |
| --- | --- |
| CLI routing, exit codes, help | `src/app/model.rs`, `controller.rs`, `view.rs` |
| Hidden prompts and size-input bound | `src/keygen/cli.rs` |
| Frozen salt, pepper and suite domains | `src/keygen/commands.rs` |
| KDF costs, format tags and errors | `src/keygen/model.rs` |
| KDF derivation and length framing | `src/keygen/derivation.rs` |
| Cipher/XOF counters and bounded writes | `src/keygen/stream.rs` |
| Key publication and post-commit errors | `src/keygen/storage.rs` |
| XOR path policy and input preflight | `src/xor/model.rs`, `controller.rs` |
| XOR transform and key consumption | `src/xor/stream.rs` |
| Native file identities and change detection | `src/xor/file_safety.rs` |
| XOR transaction and in-place ownership | `src/xor/atomic_file.rs` |

Production modules are small and import only within their domain. `keygen` and
`xor` never invoke one another. Integration happens only through the application
layer and the user's `key.key`. This makes it possible to audit the transaction
or derivation code without reading terminal code. Historical broad test modules
use test-only imports in `mod.rs`; those imports do not expand the production API.

`app::run_with` accepts explicit working-directory and resolved-executable paths,
a `BufRead`, a `Write`, and a password-reader callback. Tests exercise the same
dispatch and production KDF/writer as the executable. Production uses `rpassword`
for hidden terminal input. There is no test password flag or environment-variable
bypass in the executable. No controller changes the process working directory.

There are deliberately two atomic writers. Unifying them would silently change
original behavior: Astra refuses links/protected identities and checks metadata;
VersaKey uses `NamedTempFile::persist` with its original replacement semantics.
Any future unification requires an explicit policy design and regressions for
both sets of behavior, including Windows file sharing and Linux directory sync.

The only inherited native FFI is Windows `GetFileInformationByHandleEx` in
`xor/file_safety.rs`. Its safety comments explain the handle, buffer, class and
layout invariants. Keep handle lifetimes through publication; comparing names
alone cannot protect hard-linked files.

Source provenance: imported from sibling `astra` (package `astra-xor` 0.3.0) and
`versakey1` (package `versakey` 0.1.0). Production algorithms and file transactions
were moved into modules; imports, visibility, command dispatch and help branding
were adapted. All dependencies and checksums were retained from VersaKey's lock
file. The original source folders are not build dependencies.
