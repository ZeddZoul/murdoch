# Code Quality Check

Check the codebase for common issues:

## Run Tests

```bash
cargo test
```

## Run Clippy (Lint)

```bash
cargo clippy -- -W clippy::all
```

## Check for Dead Code

Search for:

- Unused imports
- Functions never called
- Files not imported anywhere
- Export statements with no consumers

## Check for AI-ish Patterns

Look for and remove:

- Redundant doc comments like "/// This function does X" (just say what X is)
- Obvious inline comments like "// Initialize the thing" before `let thing = Thing::new()`
- Formulaic patterns like "Create a new X with the given Y"

## Verify Deployment

```bash
flyctl logs -a murdoch-bot --no-tail
```
