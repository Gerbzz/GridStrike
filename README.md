# GridStrike Windows alpha

GridStrike is a local Rust spreadsheet prototype. Its native Windows preview edits typed cells and formulas, calculates references and common aggregate functions, saves/reopens a `.gstrike` workbook, supports undo/redo, and exports bounded displayed-value CSV. The verified checkpoint passed 10 integration tests, including guarded CSV text output and rejection of an archive whose current cells contradict its applied history.

From this standalone source root:

```powershell
cargo test --locked --features native-ui
cargo run --locked --features native-ui --bin gridstrike-native
```

The lockfile was derived from the tested Office lock without adding package versions. A new machine needs ordinary crates.io access for uncached dependencies. Windows is the tested platform.

Current limits: no XLSX import, charts, or proved interrupted-save recovery. The synthetic XLSX files in `fixtures/gridstrike/` define a future cache-boundary test; they do not make XLSX import a current feature. Desktop pointer, screen-reader, and installer acceptance remain open. No installer is supported by this source checkpoint.

The adapted A1 parser is attributed to pinned DesignCraft source in `src/designcraft_table_adapter.rs`. The selected, currently unlinked donor XLSX source and its original license files and notice are under `vendor/designcraft-selected/`. See [SOURCE_SCOPE.md](SOURCE_SCOPE.md) for ownership and exclusions. No first-party license grant is stated for GridStrike here.
