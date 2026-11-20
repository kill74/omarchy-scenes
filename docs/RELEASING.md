# Releasing

Releases are triggered by pushing an annotated `v*` tag. The
[release workflow](../.github/workflows/release.yml) builds the binaries,
creates a GitHub release with generated notes, and publishes the container to
`ghcr.io/kill74/omarchy-scenes`.

## Checklist

1. Update the version in `Cargo.toml`, `manifest.json` and `packaging/aur/PKGBUILD`.
2. Move unreleased items in `CHANGELOG.md` into a new dated version section.
3. Run the checks:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   omarchy plugin validate .
   ```
4. Commit and push to `main`.
5. Create and push the tag:
   ```bash
   git tag -a v0.1.0 -m "Omarchy Scenes v0.1.0"
   git push origin v0.1.0
   ```
6. Confirm the release and container package appear on GitHub.
7. If the marketplace listing needs updating, use its "Verify and publish a
   newer upstream commit" form with the exact 40-character SHA.

## Assets

Each GitHub release contains:

- `scenes` — CLI binary (Linux x86_64)
- `scenesd` — daemon binary (Linux x86_64)

Container tags: `ghcr.io/kill74/omarchy-scenes:0.1.2` (from tag `v0.1.2`) and `:latest`.

The AUR `PKGBUILD` lives in `packaging/aur/PKGBUILD`; publishing it to the AUR
requires a separate AUR account and is not done by CI.
