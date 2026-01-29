# Add Feature

When adding a new feature to Murdoch:

## 1. Create Spec

Create a new spec directory under `.kiro/specs/{feature-name}/`:

- `requirements.md` - What the feature does
- `design.md` - How it works
- `tasks.md` - Implementation checklist

## 2. Write Tests First

Add property tests using proptest:

```rust
proptest! {
    #[test]
    fn test_feature_invariant(input in arb_input()) {
        // Test the invariant holds
    }
}
```

## 3. Implement

- Add new module to `src/`
- Export from `lib.rs`
- Wire into relevant pipeline stage

## 4. Integration Test

```bash
cargo test --lib
```

## 5. Update DEVLOG

Document what was done in `DEVLOG.md` with date.

## 6. Deploy

See `deploy.md` for deployment steps.
