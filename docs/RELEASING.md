# Releasing

bcp uses [`cargo-release`](https://github.com/crate-ci/cargo-release) to bump
the version, tag, and push in one step. Install it once:

```sh
cargo install cargo-release
```

Cut a release:

```sh
cargo release patch    # 0.1.0 -> 0.1.1
cargo release minor    # 0.1.0 -> 0.2.0
cargo release major    # 0.1.0 -> 1.0.0
```

Add `--execute` to actually perform the bump (cargo-release dry-runs by
default). This commits the bumped `Cargo.toml`, creates a `vX.Y.Z` tag, and
pushes both. The push triggers `.github/workflows/release.yml`, which builds
release binaries for macOS (arm64) and Linux (x86_64) and attaches them to a
new GitHub Release.
