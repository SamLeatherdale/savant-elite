# AGENTS.md — savant-elite

> Guidelines for AI coding agents working in this Rust codebase.

---

## RULE 0 - THE FUNDAMENTAL OVERRIDE PREROGATIVE

If I tell you to do something, even if it goes against what follows below, YOU MUST LISTEN TO ME. I AM IN CHARGE, NOT YOU.

---

## Irreversible Git & Filesystem Actions — DO NOT EVER BREAK GLASS

1. **Absolutely forbidden commands:** `git reset --hard`, `git clean -fd`, `rm -rf`, or any command that can delete or overwrite code/data must never be run unless the user explicitly provides the exact command and states, in the same message, that they understand and want the irreversible consequences.
2. **No guessing:** If there is any uncertainty about what a command might delete or overwrite, stop immediately and ask the user for specific approval. "I think it's safe" is never acceptable.
3. **Safer alternatives first:** When cleanup or rollbacks are needed, request permission to use non-destructive options (`git status`, `git diff`, `git stash`, copying to backups) before ever considering a destructive command.
4. **Mandatory explicit plan:** Even after explicit user authorization, restate the command verbatim, list exactly what will be affected, and wait for a confirmation that your understanding is correct. Only then may you execute it—if anything remains ambiguous, refuse and escalate.
5. **Document the confirmation:** When running any approved destructive command, record (in the session notes / final response) the exact user text that authorized it, the command actually run, and the execution time. If that record is absent, the operation did not happen.

---

## Git Branch: ONLY Use `main`, NEVER `master`

**The default branch is** `main`**. The** `master` **branch exists only for legacy URL compatibility.**

- **All work happens on** `main` — commits, PRs, feature branches all merge to `main`
- **Never reference** `master` **in code or docs** — if you see `master` anywhere, it's a bug that needs fixing

---

## Toolchain: Rust & Cargo

We only use **Cargo** in this project, NEVER any other package manager.

- **Edition:** Rust 2021
- **Binary:** `savant` (installed via `cargo install --path .`)
- **Dependency versions:** Explicit versions for stability; keep the set minimal

### Key Dependencies

| Crate                  | Purpose                                |
| ---------------------- | -------------------------------------- |
| `clap` (derive)        | Command-line argument parsing          |
| `clap_complete`        | Shell completion generation            |
| `hidapi`               | USB HID device communication           |
| `rusb` / `libusb1-sys` | Low-level USB access (vendored libusb) |
| `anyhow`               | Ergonomic error handling               |
| `hex`                  | Hex encoding/decoding for USB payloads |
| `rich_rust`            | Rich terminal output                   |
| `dirs`                 | Platform-standard directory paths      |
| `serde` + `serde_json` | Serialization                          |
| `chrono`               | Date/time utilities                    |

### Dev Dependencies

| Crate        | Purpose                           |
| ------------ | --------------------------------- |
| `assert_cmd` | CLI integration testing           |
| `predicates` | Test assertions                   |
| `tempfile`   | Temporary file/directory creation |

---

## Code Editing Discipline

- For subtle/complex changes: do them methodically yourself

### No File Proliferation

If you want to change something or add a feature, **revise existing code files in place**.

**NEVER** create variations like:

- `mainV2.rs`
- `main_improved.rs`
- `main_enhanced.rs`

New files are reserved for **genuinely new functionality** that makes zero sense to include in any existing file. The bar for creating new files is **incredibly high**.

---

## Backwards Compatibility

We do not care about backwards compatibility—we're in early development with no users. We want to do things the **RIGHT** way with **NO TECH DEBT**.

- Never create "compatibility shims"
- Never create wrapper functions for deprecated APIs
- Just fix the code directly

---

## Compiler Checks (CRITICAL)

**After any substantive code changes, you MUST verify no errors were introduced:**

```bash
# Check for compiler errors and warnings
cargo check --all-targets

# Check for clippy lints (pedantic + nursery are enabled)
cargo clippy --all-targets -- -D warnings

# Verify formatting
cargo fmt --check
```

If you see errors, **carefully understand and resolve each issue**. Read sufficient context to fix them the RIGHT way.

---

---

## Beads (bd) — Dependency-Aware Issue Tracking

Beads provides a lightweight, dependency-aware issue database and CLI (`bd`) for selecting "ready work," setting priorities, and tracking status. It complements MCP Agent Mail's messaging and file reservations.

**Important:** `bd` is non-invasive—it NEVER runs git commands automatically. You must manually commit changes after `bd sync --flush-only`.

Further information is available in the dedicated beads skill

---

## ast-grep vs ripgrep

**Use** `ast-grep` **when structure matters.** It parses code and matches AST nodes, ignoring comments/strings, and can **safely rewrite** code.

- Refactors/codemods: rename APIs, change import forms
- Policy checks: enforce patterns across a repo
- Editor/automation: LSP mode, `--json` output

**Use** `ripgrep` **when text is enough.** Fastest way to grep literals/regex.

- Recon: find strings, TODOs, log lines, config values
- Pre-filter: narrow candidate files before ast-grep

### Rule of Thumb

- Need correctness or **applying changes** → `ast-grep`
- Need raw speed or **hunting text** → `rg`
- Often combine: `rg` to shortlist files, then `ast-grep` to match/modify

### Rust Examples

```bash
# Find structured code (ignores comments)
ast-grep run -l Rust -p 'fn $NAME($$ARGS) -> $RET { $$BODY }'

# Find all unwrap() calls
ast-grep run -l Rust -p '$EXPR.unwrap()'

# Quick textual hunt
rg -n 'println!' -t rust

# Combine speed + precision
rg -l -t rust 'unwrap\(' | xargs ast-grep run -l Rust -p '$X.unwrap()' --json
```

---

## Morph Warp Grep — AI-Powered Code Search

**Use** `mcp__morph-mcp__warp_grep` **for exploratory "how does X work?" questions.** An AI agent expands your query, greps the codebase, reads relevant files, and returns precise line ranges with full context.

**Use** `ripgrep` **for targeted searches.** When you know exactly what you're looking for.

**Use** `ast-grep` **for structural patterns.** When you need AST precision for matching/rewriting.

### When to Use What

| Scenario                                   | Tool        | Why                                    |
| ------------------------------------------ | ----------- | -------------------------------------- |
| "How does the USB HID protocol work here?" | `warp_grep` | Exploratory; don't know where to start |
| "Where is the pedal programming logic?"    | `warp_grep` | Need to understand architecture        |
| "Find all uses of `hidapi`"                | `ripgrep`   | Targeted literal search                |
| "Find files with `println!`"               | `ripgrep`   | Simple pattern                         |
| "Replace all `unwrap()` with `expect()`"   | `ast-grep`  | Structural refactor                    |

### warp_grep Usage

```
mcp__morph-mcp__warp_grep(
  repoPath: "/data/projects/savant-elite",
  query: "How does the foot pedal USB communication work?"
)
```

Returns structured results with file paths, line ranges, and extracted code snippets.

### Anti-Patterns

- **Don't** use `warp_grep` to find a specific function name → use `ripgrep`
- **Don't** use `ripgrep` to understand "how does X work" → wastes time with manual reads
- **Don't** use `ripgrep` for codemods → risks collateral edits
