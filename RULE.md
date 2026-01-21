# ABSOLUTE RULES

## GENERAL RULES

- Always ask for consent and provide `cargo add` commands before adding dependencies. Use latest versions unless justified.
- Explicitly justify any changes to `features`, `edition`, or `toolchain`.
- Execute tasks reliably and sequentially; do not parallelize them.

## DOCUMENTATION

- When updating documentation, adhere to these maintenance principles:
  - Progressive Disclosure: Refactor structure to reveal information incrementally; keep the entry point minimal and link to details.
  - Identify Essentials: Separate "always-relevant" core intent from "task-specific" details. Use concise, actionable statements.
  - Analyze & Organize: Identify contradictions before merging. Group instructions into logical categories to prevent fragmentation.
  - Cleanup (Flag for Deletion): Move redundant, obvious, or non-actionable instructions to a `## Flagged for Deletion` section for final review.
- Clarity at a Glance: Avoid parentheses or supplemental fluff. Prioritize brevity and high-signal data to ensure maintainability.

## DEVELOPMENT WORKFLOW

- 1. Red: Write the simplest failing test first.
- 2. Green: Implement the minimum code needed to make tests pass.
- 3. Refactor: Refactor only after tests are passing.
- 4. After changes, run verification sequentially: `cargo fmt --all` → `cargo check --all-targets` → `cargo clippy --all-targets -- -D warnings` → `cargo test`.
- 5. If errors occur, focus on the *first* error, propose a fix, and re-run verification.

## ASYNCHRONOUS BY DESIGN

- Start with Async Red tests for features requiring non-blocking I/O (FS watching, metadata fetching).
- Use `tokio::test` for asynchronous test cases.
- Decouple the Core logic from the TUI rendering loop using explicit message-passing (prefer `tokio::sync::mpsc` over `std::sync::mpsc` in async contexts).

## TESTABLE TUI & MOCKING

- Use mocking for external I/O and File System operations to ensure test determinism and speed.
- Avoid `thread::sleep` in tests; use synchronization primitives (channels, barriers) to ensure deterministic behavior.
- Decouple the Core layer from `ratatui` to allow pure logic testing without TUI dependencies.
- Implement UI Component Tests for critical elements (Tabs, Status Bar) using `ratatui::backend::TestBackend`.

## STRUCTURE (CORE / UI)

- Keep `core` independent from `ratatui`. UI must only depend on Core, never the reverse.
- Prefer small modules over growing `mod.rs`. `mod.rs` should mainly re-export submodules.
- Suggested Core modules:
  - `core/entries.rs` (listing/filtering/sorting)
  - `core/navigation.rs` (path move, parent/child)
  - `core/selection.rs` (cursor, selection rules)
  - `core/metadata.rs` (size/time)
  - `core/git.rs` (branch info)
  - `core/config.rs` (config read/parse)
- Suggested UI modules:
  - `ui/layout.rs` (area splitting only)
  - `ui/top_bar.rs` / `ui/main_pane.rs` / `ui/bottom_bar.rs` / `ui/tabs.rs`
  - `ui/event.rs` (key mapping to messages)
- UI communicates with Core via explicit messages (event → intent → state update).

## PLATFORM ABSTRACTION

- Treat OS-specific paths and behaviors as external dependencies.
- Use `std::path::PathBuf` exclusively for path manipulation.
- Abstract OS-specific logic behind Traits to allow cross-platform testing on any host.

## ERROR HANDLING & SAFETY

- No `unwrap()` or `expect()` in production code. Handle all errors gracefully using `Result` and propagation (`?`).
- `unwrap()` is permitted ONLY in test code (`#[cfg(test)]`).
- Use custom Error types (e.g., `thiserror`) to provide meaningful context for failures.

## TIDY FIRST DISCIPLINE

- Separate all changes into structural vs behavioral.
- Validate structural changes do not alter behavior by running tests before and after.

## COMMIT DISCIPLINE

- Only commit when all tests are passing.
- Only commit when all compiler/linter warnings are resolved.
- Only commit when the change is a single logical unit of work.
- Commit messages should start with `feat:` / `fix:` / `docs:` / `chore:` (Conventional Commits style).
- Commits must be performed or approved by a human.
