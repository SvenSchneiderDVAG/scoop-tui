# scoop-tui

An unofficial Ratatui-based terminal UI for [Scoop](https://github.com/ScoopInstaller/Scoop), the Windows command-line installer.

> `scoop-tui` is not affiliated with, maintained by, or endorsed by ScoopInstaller. Scoop is licensed separately under the Unlicense or MIT license.

The current version keeps a mock backend for non-Windows development and tests. On Windows, the app selects a production backend that discovers the Scoop environment and shells out to `scoop`/PowerShell commands.

## Run

```bash
cargo run
```

Keys:

- `/` enter search mode
- `Esc` or `Enter` leave search mode
- `Up`/`Down` or `k`/`j` move selection
- `i` mock install selected package
- `u` mock update selected package
- `r` mock uninstall selected package
- `y` confirm a pending command
- `n` or `Esc` cancel a pending command
- `s` load package search results
- `l` toggle installed/search view
- `c` clear search
- `?` open or close help
- `q` quit

## Current Scope

- Ratatui package table inspired by terminal package managers
- Search/filter state
- Details panel for the selected package
- Integrated terminal panel showing confirmed command output
- Header and terminal activity indicators with spinner-ready state
- Confirmation modal before install, update, or uninstall actions run
- Setup prompt for Windows systems where Scoop is not detected
- Help modal with grouped key bindings
- Production backend selected on Windows
- Mock backend with sample Scoop packages and buckets for tests and non-Windows development
- Scoop environment discovery for `SCOOP`, `SCOOP_GLOBAL`, bucket paths, app paths, and cache paths
- Short-lived package search cache so returning to recent searches does not rerun the backend or duplicate output

## License

`scoop-tui` is licensed under the MIT license. See [LICENSE](LICENSE).
