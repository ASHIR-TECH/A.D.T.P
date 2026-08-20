# Pull Request

## Summary
Briefly list what you changed and why.

## Scope

- [ ] Code?
- [ ] Tests?
- [ ] Documentation?
- [ ] Dependency update?
- [ ] Security-sensitive behavior?

## Phase

- [ ] Phase undefined  

## Validation

Before opening a pull request paste commands run for:

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo audit
```

## Risk Notes

Please Describe any risk to protocol compatibility, or audit integrity, system cryptography, config, or file writes.

## Checklist

- [ ] Changes are scoped and reviewable.
- [ ] Docs are updated.
- [ ] Tests cover the behavior.
- [ ] No secrets or generated build artifacts are committed.
