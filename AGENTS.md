# Agent Development Guidelines

This document provides instructions for automated agents working on
this repository. Follow these guidelines when creating commits and pull
requests or modifying the codebase.

## Commit Message Standards
- Use **Conventional Commits** (<https://www.conventionalcommits.org>)
  for all commit messages.
- Begin the summary line with a type such as `feat`, `fix`, `docs`,
  `chore`, etc.
- Keep the summary line under 72 characters.
- Provide a concise body when necessary to explain the change.

## Commit Sequencing
- Group related changes into separate atomic commits.
- Run pre-commit checks (see below) before each commit.

## Pull Request Guidelines
- Title should be a short summary of the change.
- Include a description with the following sections:
  - **Summary** – what was changed and why.
  - **Testing** – commands run and their results.
  - **Notes** – any additional context (optional).
- Ensure CI passes before requesting review.

## Pre-commit Checks
Run the following commands before committing:
```
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test --all-features
```

## Versioning
- Adhere to [Semantic Versioning](https://semver.org/).
- Increment:
  - **MAJOR** for breaking API changes,
  - **MINOR** for new functionality in a backwards-compatible manner,
  - **PATCH** for backwards-compatible bug fixes.
- Update `CHANGELOG.md` for user facing changes.

## CHANGELOG Maintenance
- Use Keep a Changelog format with sections: `Added`, `Changed`,
  `Deprecated`, `Removed`, `Fixed`, `Security`.
- Add entries under the `Unreleased` heading in chronological order.

## Testing Standards
- Prefer unit tests in the `tests/` directory.
- Ensure test names are descriptive.
- Maintain high coverage for new features.

## Documentation
- Update `README.md` when public APIs or workflows change.
- Keep inline code comments clear and concise.

