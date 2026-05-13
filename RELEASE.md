# Release Checklist

This checklist is for publishing a coordinated Rust crate and Python package
release.

## One-Time Repository Setup

- GitHub branch protection should require the `CI` workflow on `master`.
- Create a GitHub environment named `crates-io`.
- Add `CARGO_REGISTRY_TOKEN` to the `crates-io` environment secrets.
- Create a GitHub environment named `pypi`.
- Add `PYPI_API_TOKEN` to the `pypi` environment secrets.
- Confirm the PyPI project name is `citerra`.

The release workflow uses environment-scoped publishing tokens. Package
publication jobs set `deployment: false` so GitHub environment approvals do not
create repository deployment records.

## Pre-Release Local Gates

Run these from a clean worktree before tagging:

```sh
git status --short
cargo fmt --all -- --check
guix shell -m manifest.scm -- actionlint
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --all-features -- --ignored
cargo bench --all-features --no-run
cargo package --locked
cargo publish --dry-run --locked
```

Build and test the Python wheel locally:

```sh
rm -rf target/wheels target/python-test
guix shell -m manifest.scm -- maturin build --release --out target/wheels
python3 - <<'PY'
from pathlib import Path
from zipfile import ZipFile

wheel = sorted(Path("target/wheels").glob("citerra-*.whl"))[-1]
target = Path("target/python-test")
target.mkdir(parents=True, exist_ok=True)
with ZipFile(wheel) as archive:
    archive.extractall(target)
PY
guix shell -m manifest.scm -- env PYTHONPATH=target/python-test python3 -m pytest tests/python
```

Check package contents:

```sh
cargo package --locked --list
python3 - <<'PY'
from pathlib import Path
from zipfile import ZipFile

wheel = sorted(Path("target/wheels").glob("citerra-*.whl"))[-1]
with ZipFile(wheel) as archive:
    for name in archive.namelist():
        if name.endswith(("METADATA", "WHEEL")) or name.startswith("citerra/"):
            print(name)
PY
```

The crate package should not contain local caches, virtual environments, build
artifacts, editor files, or generated benchmark reports.

## Version And Changelog

- `Cargo.toml` and `pyproject.toml` must have the same version.
- `Cargo.lock` must reflect the package version.
- `CHANGELOG.md` must have an entry for the release.
- `README.md` and `RUST.md` install snippets must reference the intended minor version.
- The tag must be exactly `vX.Y.Z` for package version `X.Y.Z`.

## Release

After the final release-prep commit is on `master`:

```sh
git pull --ff-only origin master
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin master
git push origin vX.Y.Z
```

Pushing the tag runs `.github/workflows/release.yml`. The workflow:

- validates version metadata and release gates
- builds the Rust crate package
- builds the Python source distribution
- builds ABI3 wheels for Linux x86_64, Linux aarch64, macOS x86_64,
  macOS aarch64, and Windows x64
- creates a GitHub release with all Python artifacts
- publishes the Rust crate to crates.io
- publishes the Python distribution to PyPI

## Post-Release Verification

```sh
cargo search bibtex-parser
python3 -m venv /tmp/citerra-release-check
/tmp/citerra-release-check/bin/python -m pip install --upgrade pip
/tmp/citerra-release-check/bin/python -m pip install citerra==X.Y.Z
/tmp/citerra-release-check/bin/python - <<'PY'
import citerra

document = citerra.parse('@article{paper, title = "Example Paper"}')
assert document.entry("paper").get("title") == "Example Paper"
print("ok")
PY
```

Confirm:

- GitHub release exists and has wheels plus source distribution.
- crates.io shows the new crate version.
- docs.rs successfully builds the new docs.
- PyPI shows the new Python version.
- `pip install citerra` works on a fresh environment.
