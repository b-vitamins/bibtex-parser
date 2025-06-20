# Agents

This document describes the AI assistant environment and tools available in this repository.

## Codex CLI
- **Role**: AI-driven coding assistant operating inside a git-backed, sandboxed workspace at `/home/b/projects/bibtex-parser`.
- **Capabilities**:
  - Explore files and run shell commands via the `shell` function.
  - Apply code modifications using `apply_patch`.
  - Run tests, benchmarks, and other commands (e.g., `cargo test`, `rg`, `cargo bench`).
  - Follow strict coding and style guidelines, logging telemetry for traceability.
- **Workflow**:
  1. Receive natural language instructions from the user.
  2. Inspect the codebase using shell commands.
  3. Propose and apply patches to files.
  4. Run tests or commands to verify changes.
  5. Report back with summaries and results.

## Tools
- **shell**: Execute arbitrary shell commands in the workspace. Example:
  ```json
  { "name": "functions.shell", "arguments": { "command": ["bash", "-lc", "rg --files ."] } }
  ```
- **apply_patch**: Apply unified diff patches to files programmatically.

## Coding Guidelines
- Fix issues at the root cause, not with surface-level workarounds.
- Keep changes minimal, focused, and consistent with existing style.
- Remove inline comments before finalizing patches.
- Use `pre-commit` to verify formatting and lint rules where applicable.

## Roles
- **User**: Developer issuing instructions.
- **Codex CLI**: AI assistant performing code exploration, modifications, and verification.

## Reference Documents
- **ROADMAP.md**: Comprehensive design & implementation document with project overview, architecture, roadmap, technical specifications, API design, testing strategy, and future enhancements. Future agents and maintainers should refer to this file for detailed guidance.