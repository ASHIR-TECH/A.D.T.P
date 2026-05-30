# Pull Request

## Summary

Explain what changed and why.

## Scope

- [ ] Code
- [ ] Tests
- [ ] Documentation
- [ ] Dependency update
- [ ] Security-sensitive behavior

## Phase

- [ ] Phase 0
- [ ] Phase 1
- [ ] Future phase scaffold only

## Validation

Paste commands run:

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo audit
```

## Risk Notes

Describe any risk to protocol compatibility, audit integrity, cryptography, config, or file writes.

## Checklist

- [ ] Changes are scoped and reviewable.
- [ ] Docs are updated.
- [ ] Tests cover the behavior.
- [ ] No secrets or generated build artifacts are committed.
