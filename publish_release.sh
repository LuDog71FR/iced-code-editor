#!/bin/bash
set -e

# === CONSTANTS ===
CARGO_TOML="Cargo.toml"
CHANGELOG="CHANGELOG.md"

# === FUNCTIONS ===

# Prompts user for confirmation
confirm() {
    local message=$1
    read -r -p "$message [y/N] " response
    if [[ ! "$response" =~ ^[yY]$ ]]; then
        echo "Aborted."
        exit 1
    fi
}

# Extracts version from Cargo.toml
extract_version() {
    grep '^version = "' "$CARGO_TOML" | head -1 | sed 's/.*"\(.*\)".*/\1/'
}

# Extracts changelog section for a given version
extract_changelog_section() {
    local version=$1
    # Extract everything between "## [$version]" and the next "## [" (or EOF)
    # Remove leading and trailing blank lines
    sed -n "/^## \[$version\]/,/^## \[/{
        /^## \[$version\]/d
        /^## \[/d
        p
    }" "$CHANGELOG" | sed -e '/./,$!d' -e :a -e '/^\n*$/{$d;N;ba' -e '}'
}

# === MAIN SCRIPT ===

echo "=========================================="
echo "       Release Publishing Script"
echo "=========================================="
echo ""

# 1. Validate argument
if [ -z "$1" ]; then
    echo "Usage: ./publish_release.sh \"Commit message\""
    echo ""
    echo "Example: ./publish_release.sh \"fix: editor background overflow\""
    exit 1
fi
COMMIT_MSG="$1"

# 2. Verify we're on main branch
BRANCH=$(git branch --show-current)
if [ "$BRANCH" != "main" ]; then
    echo "Error: Must be on 'main' branch (currently on '$BRANCH')"
    exit 1
fi
echo "Branch: $BRANCH"

# 3. Extract version
VERSION=$(extract_version)
if [ -z "$VERSION" ]; then
    echo "Error: Could not extract version from $CARGO_TOML"
    exit 1
fi
echo "Version from Cargo.toml: $VERSION"

# 4. Get last tag
LAST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "v0.0.0")
LAST_VERSION=${LAST_TAG#v}
echo "Last tag: $LAST_TAG"

# 5. Verify version increment
if [ "$VERSION" == "$LAST_VERSION" ]; then
    echo ""
    echo "Error: Version $VERSION is same as last tag."
    echo "Please increment version in $CARGO_TOML"
    exit 1
fi
echo "Version increment OK: $LAST_VERSION -> $VERSION"

# 6. Verify changelog
if ! grep -q "^## \[$VERSION\]" "$CHANGELOG"; then
    echo ""
    echo "Error: $CHANGELOG does not contain section for version $VERSION"
    echo "Expected line starting with: ## [$VERSION]"
    exit 1
fi
echo "Changelog section found for $VERSION"

# 7. Extract changelog content
CHANGELOG_CONTENT=$(extract_changelog_section "$VERSION")
if [ -z "$CHANGELOG_CONTENT" ]; then
    echo ""
    echo "Warning: Changelog section for $VERSION appears to be empty"
    confirm "Continue anyway?"
fi

# 8-11. Pre-commit checks
echo ""
echo "=== Running pre-commit checks ==="
echo ""

echo "Running tests..."
cargo test --verbose --workspace --all-features

echo ""
echo "Running clippy..."
cargo clippy --verbose --workspace --all-targets --all-features -- -D warnings

echo ""
echo "Formatting code..."
cargo fmt --all

echo ""
echo "Generating documentation..."
rm -rf target/doc && cargo doc --no-deps

echo ""
echo "All checks passed!"

# 12-15. Git commit
echo ""
echo "=== Git Commit ==="
echo ""
git status
echo ""

# Check if there are changes to commit
if [ -z "$(git status --porcelain)" ]; then
    echo "No changes to commit, skipping commit step."
else
    confirm "Proceed with 'git add .' ?"
    git add .

    echo ""
    echo "Committing with message: $COMMIT_MSG"
    git commit -m "$COMMIT_MSG"
fi

# 16-17. Git push
echo ""
echo "Pushing to GitHub"
git push

# 18-20. Create tag
TAG="v$VERSION"
TAG_MSG="Release $TAG - $COMMIT_MSG"
echo ""
echo "=== Creating Tag ==="
echo "Tag: $TAG"
echo "Tag message: $TAG_MSG"
echo ""

git tag -a "$TAG" -m "$TAG_MSG"
git push origin "$TAG"

# 21-22. GitHub Release
RELEASE_TITLE="$TAG - $COMMIT_MSG"
echo ""
echo "=== Creating GitHub Release ==="
echo "Title: $RELEASE_TITLE"
echo ""
echo "Release notes:"
echo "---"
echo "$CHANGELOG_CONTENT"
echo "---"
echo ""

confirm "Create GitHub release ?"
gh release create "$TAG" --title "$RELEASE_TITLE" --notes "$CHANGELOG_CONTENT"

# 23-24. Publish to crates.io
echo ""
echo "=== Publishing to crates.io ==="
echo "Package: iced-code-editor $VERSION"
echo ""

confirm "Publish iced-code-editor $VERSION to crates.io ?"
cd iced-code-editor && cargo publish
cd ..

# 25. Success
echo ""
echo "=========================================="
echo "   Successfully released $TAG!"
echo "=========================================="
echo ""
echo "Summary:"
echo "  - Commit: $COMMIT_MSG"
echo "  - Tag: $TAG"
echo "  - GitHub Release: $RELEASE_TITLE"
echo "  - crates.io: iced-code-editor $VERSION"
echo ""
echo "Done!"
