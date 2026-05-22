# scoop-tui

An unofficial Ratatui-based terminal UI for [Scoop](https://github.com/ScoopInstaller/Scoop), the Windows command-line installer.

> `scoop-tui` is not affiliated with, maintained by, or endorsed by ScoopInstaller. Scoop is licensed separately under the Unlicense or MIT license.

On Windows, the app discovers the Scoop environment and shells out to `scoop` commands. Non-Windows development and tests use sample package data.

## Run

```bash
cargo run
```

Keys:

- `/` enter search mode
- `Esc` or `Enter` leave search mode
- `Up`/`Down` or `k`/`j` move selection
- `i` install selected package
- `u` update selected package
- `r` uninstall selected package
- `y` confirm a pending command
- `n` or `Esc` cancel a pending command
- `s` load package search results
- `l` toggle installed/search view
- `c` clear search
- `?` open or close help
- `q` quit

Mouse:

- Scroll wheel moves package selection
- Click a package row to select it
- Click the search field to enter search mode
- Click modal confirm/cancel actions
- Click footer actions for help, view, search, install, update, and uninstall
- Click the selected package homepage in Details to open it in the default browser

## Current Scope

- Ratatui package table inspired by terminal package managers
- Search/filter state
- Details panel for the selected package
- Integrated terminal panel showing confirmed command output
- Header and terminal activity indicators with spinner-ready state
- Confirmation modal before install, update, or uninstall actions run
- Setup prompt for Windows systems where Scoop is not detected
- Help modal with grouped key bindings
- Mouse support for package selection, search mode, modal actions, footer actions, scrolling, and homepage links
- Production backend selected on Windows
- Sample Scoop packages and buckets for tests and non-Windows development
- Scoop environment discovery for `SCOOP`, `SCOOP_GLOBAL`, bucket paths, app paths, and cache paths
- Short-lived package search cache so returning to recent searches does not rerun the backend or duplicate output

## License

`scoop-tui` is licensed under the MIT license. See [LICENSE](LICENSE).
