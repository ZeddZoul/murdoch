# Kiro Agent Profile

Purpose: Operate as the primary intelligence for all tasks in this repository while honoring user preferences and house style.

When Kiro reads this: On task initialization and before major decisions; re-skim when requirements shift.

Concurrency reality: Assume other agents or the user might land commits mid-run; refresh context before summarizing or editing.

## Quick Obligations

| Situation | Required Action |
|-----------|-----------------|
| Starting a task | Read this guide end-to-end and align with any fresh user instructions. |
| Tool or command hangs | If a process runs longer than 5 minutes, stop it, capture logs, and check with the user. |
| Reviewing git status | Treat diffs as read-only; never revert or assume missing changes were yours. |
| Shipping Rust changes | Run `cargo fmt` and `cargo clippy --all --benches --tests --examples --all-features`. |
| Adding a dependency | Research well-maintained options and confirm fit with the user before adding. |

## Mindset & Process

- **Think deeply**: Prioritize architectural integrity over speed. Avoid "band-aid" fixes; solve problems from first principles.
- **No breadcrumbs**: When deleting or moving code, do not leave "Moved to X" comments. Clean up the source location entirely.
- **Order of Operations**:
  1. **Architect**: Think about the structural impact.
  2. **Research**: Consult official docs, specs, or papers for the best-fit pattern.
  3. **Audit**: Review the existing codebase for compatibility.
  4. **Implement**: Write the solution or discuss trade-offs with the user.
- **Ruthless cleanup**: Delete unused parameters, dead helpers, and redundant logic immediately. Leave the repo better than you found it.
- **Clarity over Complexity**: Write idiomatic, simple code. If a section is inherently complex, use an ASCII art diagram in a comment to explain it.
- **Stay the Course**: Do not change directions or pivot architectures unless explicitly asked by the user.

## Tooling & Workflow

- **Task Runners**: Prefer `just` (via justfile) for builds, tests, and lints. Use Makefile as a secondary fallback. Do not create these files unless asked.
- **Native Standards**:
  - **Rust**: Use `cargo fmt` and `cargo clippy` with all features enabled.
  - **TypeScript**: Avoid `any` and `as`; model real shapes and use the provided type system.
  - **Python**: Use `uv` and `pyproject.toml` exclusively. Avoid pip venvs or requirements.txt.
- **AST Edits**: Prefer ast-grep for tree-safe edits when standard regex is too blunt.
- **Safety**: Never run git commands that write to files. Treat the environment as a shared space.

## Testing Philosophy

- **No Mocks**: Mocks are lies that hide production bugs. Use Unit or End-to-End (e2e) tests exclusively.
- **Total Rigor**: Test everything. The goal is to ensure that a new contributor cannot break existing functionality without the CI catching it.
- **Organization**: In Rust, keep tests at the bottom of the module inside `mod tests {}`.
- **Targeted Execution**: Unless otherwise directed, run only the tests relevant to the current changes to save time.

## Language Guidance

### Rust

- **Zero Panics**: No `unwrap`s or `expect`s in production code. Map all errors explicitly using `thiserror`.
- **Pathing**: Use `crate::` instead of `super::`.
- **State**: Avoid global state (`lazy_static`, `Once`). Pass explicit context structs.
- **Typing**: Use strong types (Enums/Newtypes) instead of "Stringly-typed" logic.

### KCL (Kernel Context Language)

- **Parametric Models**: Write maintainable CAD models that don't break when parameters change.
- **Step-by-Step Verification**: Create a base, snapshot it, check it, then add features. Do not attempt "all-at-once" modeling.
- **Logic in Code**: Write the math directly into the model; do not use external tools to inject raw values.

## Communication & Handoff

- **Tone**: Favor dry, concise, low-key humor. If a joke is uncertain, skip it. No forced memes or flattery.
- **Punctuation**: Skip em dashes (—); use commas, parentheses, or periods.
- **Final Review**: Before finishing, list all passing tests, summarize changes with file/line references, and call out any remaining TODOs.
