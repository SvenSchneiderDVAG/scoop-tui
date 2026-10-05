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
- `PageUp`/`PageDown` move package selection by page
- `Home`/`End` or `g`/`G` jump to the first/last package
- `Shift+PageUp`/`Shift+PageDown` scroll terminal output by page
- `Shift+Home`/`Shift+End` jump terminal output to top/bottom
- `i` install selected package
- `u` update selected package
- `r` uninstall selected package
- `y` confirm a pending command
- `n` or `Esc` cancel a pending command
- `s` load package search results
- `l` toggle installed/search view
- `c` clear search
- `?` open or close help; help can be scrolled with `Up`/`Down`, `k`/`j`, `PageUp`/`PageDown`, or `g`
- `q` or `Ctrl+C` quit, even while a command is running

While Scoop commands run, navigation, filtering, help, and quitting stay available; only keys that start Scoop commands wait.

Mouse:

- Scroll wheel moves package selection
- Scroll wheel over the terminal panel scrolls command output
- Scroll wheel over the help modal scrolls help content
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
- Scrollable terminal output panel
- Header and terminal activity indicators with animated spinner state while commands run
- Fast startup: installed packages appear after `scoop list`, while `scoop update` and `scoop status` check for newer versions in the background
- Windows single-instance guard so duplicate launches exit before drawing another TUI
- Confirmation modal before install, update, or uninstall actions run
- Setup prompt for Windows systems where Scoop is not detected
- Scrollable help modal with grouped key bindings
- Mouse support for package selection, terminal/help scrolling, search mode, modal actions, footer actions, and homepage links
- Production backend selected on Windows
- Sample Scoop packages and buckets for tests and non-Windows development
- Scoop environment discovery for `SCOOP`, `SCOOP_GLOBAL`, bucket paths, app paths, and cache paths
- Scoop command discovery prefers PowerShell shims on Windows to avoid extra visible command windows where possible
- Short-lived package search cache so returning to recent searches does not rerun the backend or duplicate output

## License

`scoop-tui` is licensed under the MIT license. See [LICENSE](LICENSE).
