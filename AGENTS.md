# Agent Instructions

These instructions apply to this repository.

## Release Hygiene

- Follow SemVer for every published Rust crate and Python package release.
- Do not move, delete, or recreate a tag after it has published artifacts.
- If a published release has a release-affecting defect, cut the next patch
  version. Example: publish `0.2.1` after `0.2.0`, not a replacement `0.2.0`.
- Keep `Cargo.toml`, `pyproject.toml`, and `Cargo.lock` on the same package
  version before tagging.
- Keep `CHANGELOG.md` current. Every release gets a dated entry with user-facing
  changes and release/process fixes.
- Release tags must be annotated and exactly match `vX.Y.Z`.
- The release commit must be on `master` before the tag is pushed.
- A tagged release should not be approved for publication while branch CI has
  known failures on the same commit or an already-required follow-up fix.
- If GitHub environment approvals are required, approve `crates-io` and `pypi`
  only after release validation and artifact-build jobs have passed. Package
  publication jobs should use `deployment: false` so releases do not create
  GitHub deployment records.

## Commit Hygiene

- Use Conventional Commits for repository commits:
  - `feat: ...` for new user-facing behavior.
  - `fix: ...` for bug fixes and compatibility fixes.
  - `perf: ...` for benchmark-backed speed or memory improvements.
  - `docs: ...` for documentation-only changes.
  - `test: ...` for test-only changes.
  - `ci: ...` for workflow and release automation.
  - `chore: ...` for maintenance that does not fit the above.
  - `release: X.Y.Z` for release-prep commits.
- Keep commits scoped. Do not bundle unrelated parser, docs, CI, and release
  work unless they are part of one release-prep change.
- Public docs and package metadata must use neutral technical wording. Avoid
  subjective claims, self-evaluation, and comparisons that are not backed by a
  reproducible benchmark.

## Pre-Commit And Pre-Release Gates

- Use the Guix manifest for local tooling:

  ```sh
  guix shell -m manifest.scm --
  ```

- Install the local pre-commit hooks when working in the repo:

  ```sh
  guix shell -m manifest.scm -- pre-commit install
  ```

- Before committing, run at least:

  ```sh
  guix shell -m manifest.scm -- pre-commit run --all-files
  guix shell -m manifest.scm -- cargo fmt --all -- --check
  guix shell -m manifest.scm -- actionlint
  guix shell -m manifest.scm -- env CC=gcc cargo clippy --locked --all-targets --all-features -- -D warnings
  guix shell -m manifest.scm -- env CC=gcc cargo test --locked --all-features
  ```

- Before a release, additionally run:

  ```sh
  guix shell -m manifest.scm -- env CC=gcc cargo test --locked --all-features -- --ignored
  guix shell -m manifest.scm -- env CC=gcc cargo bench --locked --all-features --no-run
  cargo package --locked
  cargo publish --dry-run --locked
  rm -rf target/wheels target/python-test
  guix shell -m manifest.scm -- env CC=gcc maturin build --release --out target/wheels
  python3 -c 'from pathlib import Path; from zipfile import ZipFile; wheel = sorted(Path("target/wheels").glob("citerra-*.whl"))[-1]; target = Path("target/python-test"); target.mkdir(parents=True, exist_ok=True); ZipFile(wheel).extractall(target)'
  guix shell -m manifest.scm -- env PYTHONPATH=target/python-test python3 -m pytest tests/python
  ```

- When a local toolchain cannot reproduce a CI-only gate, state that explicitly
  and use GitHub CI as the source of truth before release approval.

## Python Package Naming

- GitHub repository name: `citerra`.
- Rust crate name: `bibtex-parser`.
- Rust import path: `bibtex_parser`.
- Python distribution name: `citerra`.
- Python import name: `citerra`.
- Do not introduce `bibtex_parser` as a Python import or package name.
