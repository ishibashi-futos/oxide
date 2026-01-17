# ABSOLUTE RULES

## PROCESS

- Always ask for consent and provide `cargo add` commands before adding dependencies. Use latest versions unless justified.
- When updating documentation, prioritize integrating changes into existing sections to prevent fragmentation and improve maintainability.
- Always follow the TDD cycle: Red → Green → Refactor.
- Write the simplest failing test first.
- Implement the minimum code needed to make tests pass.
- Refactor only after tests are passing.
- Execute tasks reliably and sequentially; do not parallelize them.

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

- **No `unwrap()` or `expect()` in production code.** Handle all errors gracefully using `Result` and propagation (`?`).
- `unwrap()` is permitted ONLY in test code (`#[cfg(test)]`).
- Use custom Error types (e.g., `thiserror`) to provide meaningful context for failures.

## TIDY FIRST DISCIPLINE

- Separate all changes into structural vs behavioral.
- Never mix structural and behavioral changes in the same commit.
- Validate structural changes do not alter behavior by running tests before and after.

## COMMIT DISCIPLINE

- Only commit when all tests are passing.
- Only commit when all compiler/linter warnings are resolved.
- Only commit when the change is a single logical unit of work.
- Commit messages must clearly state `[Structural]` or `[Behavioral]`.
- Commits must be performed or approved by a human.
