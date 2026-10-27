# Instructions for future AI agents

The primary goal is **utmost reliability on Linux and Windows**. Treat both as
first-class platforms. Preserve user data and exact compatibility with both
original applications before optimizing, simplifying, or adding convenience.

Read `README.md`, `docs/architecture.md`, `docs/compatibility.md`, and
`docs/testing.md` before modifying behavior. Stardust combines Astra's XOR and
all five VersaKey generators in one executable; its source is self-contained.

## Non-negotiable invariants

1. Never change a shipped generator's salt, pepper, domain, format tags, length
   framing, KDF parameters, nonce, counter order, or stream construction as a
   branding/refactoring change. Existing password + size + suite must reproduce
   the same bytes. Do not regenerate expected vectors to hide a regression.
2. Keep generation in the current directory and XOR beside the resolved
   executable. Use explicit paths and native `OsString`s. Never mutate global
   cwd, silently select a suite, repeat a short key, or generate a missing key.
3. Preserve atomic replacement. Write a unique same-directory temporary file,
   finish and sync it, recheck the relevant invariants, then publish. Never
   delete/truncate the old output first. Distinguish failures before and after
   commit. In-place XOR must retain the same transactional guarantees.
4. Keep keys and executables protected during XOR, including aliases/hard links.
   Preserve Linux permission/identity checks and Windows sharing/reparse-point/
   file-ID handling. Do not replace identity checks with string comparisons.
5. Keep memory bounded independently of file size, check stream counter limits,
   and propagate partial/interrupted/zero-write failures correctly. Retain
   zeroization and hidden password entry. Never add passwords to logs, CLI
   arguments, process environments or test fixture artifacts containing real data.
6. Never silently remove another running process's temporary files. Directory
   snapshots and adversarial concurrent mutation are not promised by this app;
   do not claim stronger guarantees than the code and tests establish.

## How to make an auditable change

- Keep routing/view/controller logic in `src/app`, generation in `src/keygen`,
  and XOR in `src/xor`. Keep models and transforms free from terminal/global
  environment access. Use explicit test seams such as `app::run_with`.
- Preserve separate transaction policies unless an explicit feature requires
  unification. A shared filename does not imply identical overwrite semantics.
- Make focused changes. Add a meaningful regression for fixes involving data,
  compatibility, I/O, concurrency or platform behavior. Cover failure paths as
  carefully as success. Retain all inherited tests.
- Build with the pinned Rust toolchain and `--locked`. Review algorithm/library
  upgrades against original known answers and both OS suites. Do not weaken
  production KDF costs or warnings to make tests faster or pass.
- Run `scripts/test-windows.ps1` on Windows and `bash scripts/test-linux.sh` on
  Linux/WSL. Both must pass tests, formatting, lint and release builds. If an OS
  cannot be tested, report that gap explicitly; do not claim it passed.
- Inspect platform-specific code under both `cfg(windows)` and `cfg(unix)`;
  one successful compile cannot verify the other. Use Linux-native fixture
  storage for permissions, links and FIFOs. Keep build directories separate.
- Update compatibility/architecture/test documentation when changing a contract
  or test seam. Report what changed, how it was verified, and practical limits.

There is no requirement to ask permission for routine local edits, tests or
refactors already authorized by the user. Respect the session's filesystem and
execution permissions. Do not modify the sibling original apps to make Stardust
pass. Do not publish releases or send messages unless the user authorizes it.
