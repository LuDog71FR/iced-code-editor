# Publishing

## Update version & ChangeLog

Increase version number in Cargo.toml (check VERSIONING.md file).

### Update the CHANGELOG.md

#### Format : X.Y.Z

- **MAJOR (X)** : Changements incompatibles (breaking changes)
- **MINOR (Y)** : Nouvelles fonctionnalités compatibles
- **PATCH (Z)** : Corrections de bugs compatibles

#### Example

```markdown
## [0.3.0] - 2025-01-XX

### Added

- Delete key now deletes text selection when text is selected

### Changed

- Updated documentation to reflect new deletion behavior

### Fixed

- Doctest compilation error in CommandHistory::new (#1)

## [0.2.0] - 2024-12-XX

### Added

- Initial release on crates.io
- Canvas-based code editor widget
```

## Last checks

Last checks before commit, launch all tests, clippy & generate the doc:

```bash
cargo test --all
cargo clippy --all-targets -- -D warnings
cargo fmt --all
cargo doc --no-deps
```

## Commit & Push to GitHub

Commit and push the changes to GitHub:

```bash
git commit
git push
```

## Create the new release tag

```bash
git tag -a v0.2.3 -m "Release v0.2.3 - Fix README.md and example"
git push origin v0.2.3
```

## Create the GitHub Release

1. Go to the GitHub repository
2. Click on "Releases" > "Create a new release"
3. Select tag `v0.2.0`
4. Title : `v0.2.0 - Initial Release`
5. Description : Copy the key points of the CHANGELOG

## Publish on crates.io

As simple as:

```bash
cargo publish
```

### Verify

Go to https://crates.io/crates/iced-code-editor and check if everything is alright.

## docs.rs

Generate automatically when publishing to crates.io !

Wait 5-15 minutes after publishing the crate and go to https://docs.rs/iced-code-editor.
Check if the documentation is well displayed.

## Magic script

Now the cherry on the cake ;-)

All above steps (last checks, commit, publish ...) could be done in one go with the script below:

```bash
publish_release.sh <commit_message>
```

Example:

```bash
./publish_release.sh "fix: editor background overflow"
```
