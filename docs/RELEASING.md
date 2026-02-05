# Releasing

## Versioning

This project uses semantic versioning.

1. Update version in `Cargo.toml`.
2. Update `CHANGELOG.md`.
3. Commit with a release-oriented message.
4. Create and push a tag, for example `v2.1.0`.

## Suggested Commands

```bash
cargo check
cargo test
git tag vX.Y.Z
git push origin vX.Y.Z
```

## Post-Release

- Confirm release/tag metadata on GitHub.
- Validate CI and any release-related workflows.
