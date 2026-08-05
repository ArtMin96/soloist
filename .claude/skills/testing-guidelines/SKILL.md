---
name: testing-guidelines
description: Guide for writing tests. Use when adding new functionality, fixing bugs, or when tests are needed. Emphasizes integration tests, real-world fixtures, and regression coverage.
version: "1.0.0"
---

# Testing Guidelines

Follow these principles when writing tests for this codebase.

## Core Principles

### 1. Mock External Services, Use Real Fixtures

**ALWAYS** mock third-party network services and external processes. **ALWAYS** use fixtures based on real-world data.

- Fixtures must be scrubbed of PII (use dummy data like `foo@example.com`, `user-123`)
- Capture real outputs (API responses, CLI output such as `git --porcelain=v2` or `gh --json`), then sanitize them
- Never make actual network calls in tests
- In Rust, mocking happens at the port boundary: use the shared fakes in `core::testing` (e.g. `MockClock`, fake repos), never re-roll a fake per crate

### 2. Prefer Integration Tests Over Unit Tests

Focus on **end-to-end style tests** that validate inputs and outputs, not implementation details.

- Test the public interface, not internal methods
- Unit tests are valuable for edge cases in pure functions, but integration tests are the priority
- If refactoring breaks tests but behavior is unchanged, the tests were too coupled to implementation

### 3. Minimize Edge Case Testing

Don't test every variant of a problem.

- Cover the **common path** thoroughly
- Skip exhaustive input permutations
- Skip unlikely edge cases that add maintenance burden without value
- One representative test per category of input is usually sufficient

### 4. Always Add Regression Tests for Bugs

When a **bug** is identified, **ALWAYS** add a test that would have caught it.

- The test should fail before the fix and pass after
- Name it descriptively to document the bug
- This prevents the same bug from recurring

**Note:** Regression tests are for unintentional broken behavior (bugs), not intentional changes. Intentional feature removals, deprecations, or breaking changes do NOT need regression tests—these are design decisions, not defects.

### 5. Cover Every User Entry Point

**ALWAYS** have at least one basic test for each customer/user entry point.

- Tauri commands, MCP tools, HTTP/CLI endpoints, public/exported functions
- Test the common/happy path first
- This proves the entry point works at all

**Note:** "Entry point" means the public interface—`Facade` methods, Tauri commands, MCP tools, HTTP routes, exported functions. Internal/private functions are NOT entry points, even if they handle user-facing flags or options. Test entry points; internal functions get coverage through those tests.

### 6. Tests Validate Before Manual QA

Tests are how we validate **ANY** functionality works before manual testing.

- Write tests first or alongside code, not as an afterthought
- If you can't test it, reconsider the design
- Passing tests should give confidence to ship

## Technical Guidelines

### File Organization

**Frontend (TypeScript/Vitest):**
- Test files use `*.test.ts` / `*.test.tsx` extension
- Co-locate tests with source: `foo.ts` → `foo.test.ts`

**Rust:**
- Unit tests live in a **separate sibling file**, wired as a child module so they reach private items:
  ```rust
  // at the bottom of foo.rs
  #[cfg(test)]
  #[path = "foo_tests.rs"]
  mod tests;
  ```
- Adapter/integration tests live in the crate's `tests/` directory
- Inline `mod tests` blocks only when there is no other way

### Test Isolation

Every test must:
- Run independently without affecting other tests
- Use temporary directories for file operations
- Clean up resources deterministically

**TypeScript (Vitest):**

```typescript
import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

describe("my feature", () => {
  let tempDir: string;

  beforeEach(() => {
    tempDir = join(tmpdir(), `soloist-test-${Date.now()}`);
    mkdirSync(tempDir, { recursive: true });
  });

  afterEach(() => {
    rmSync(tempDir, { recursive: true, force: true });
  });

  it("does something with files", () => {
    writeFileSync(join(tempDir, "test.ts"), "content");
    // ... test code
  });
});
```

**Rust (`tempfile` — cleanup happens on `Drop`, no manual teardown):**

```rust
use tempfile::TempDir;

#[test]
fn does_something_with_files() {
    let dir = TempDir::new().expect("tempdir");
    std::fs::write(dir.path().join("solo.yml"), "processes: {}").expect("write");
    // ... test code; dir is removed when it goes out of scope
}
```

- Time-dependent logic is tested with `MockClock` from `core::testing` — never with real sleeps
- Tests that spawn processes must reap them; a test run leaves no orphans behind

### Pure Function Tests

For pure functions without side effects, no special setup is needed:

```typescript
import { describe, it, expect } from "vitest";
import { tokenizeArgs } from "./tokenizeArgs";

describe("tokenizeArgs", () => {
  it("splits quoted arguments", () => {
    expect(tokenizeArgs('--name "hello world"')).toEqual(["--name", "hello world"]);
  });
});
```

```rust
#[test]
fn debounce_coalesces_burst_into_one() {
    let clock = MockClock::new();
    // ... exercise the pure logic, assert the observable outcome
}
```

## Running Tests

```bash
just test                      # cargo test --workspace + vitest (run once)
cargo test --workspace         # Rust only
pnpm -C crates/app/ui test     # frontend only
just lint                      # fmt, clippy -D warnings, tsc, ESLint, dependency guards
```

## Project-Specific Requirements (CLAUDE.md §15)

These are house rules that every test in this repo must also satisfy:

- **Assert the observable outcome, never the call shape.** A test pinning the arguments a function was called with defends the implementation, including its bugs.
- **A new or changed test must be shown to fail against the unfixed behavior** — break the fix, watch it redden, restore. A test never observed failing is unproven.
- Never `#[ignore]`, skip, or comment out a test to dodge a red — fix the cause or report it red.
- No placeholder/empty tests, no tautological asserts. If a module has nothing meaningful to test yet, it has no test yet.

## Checklist Before Submitting

- [ ] New entry points have at least one happy-path test
- [ ] Bug fixes (not intentional changes) include a regression test
- [ ] External services/processes are mocked with sanitized fixtures
- [ ] Tests validate behavior, not implementation
- [ ] No shared state between tests
- [ ] New/changed tests were observed failing against the unfixed behavior
