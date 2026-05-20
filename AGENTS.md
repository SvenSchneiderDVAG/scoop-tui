# Repository Instructions

- Keep the design simple and pragmatic. Prefer YAGNI and KISS over speculative abstractions.
- When adding a feature, bump the project version semantically in `Cargo.toml`.
- When patching or fixing behavior, bump the patch version semantically in `Cargo.toml`.
- Use Conventional Commits for commit messages, such as `feat: add mouse support` or `fix: handle empty package lists`.
- Use `/opt/homebrew/bin/gh` for GitHub CLI commands because `gh` may not be on `PATH` in this environment.
- Keep mock/test-only data out of the production backend path.
- Do not add implementation-plan documents intended only for local planning to the repository.
