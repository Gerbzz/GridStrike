# GridStrike source preview

GridStrike is a local desktop spreadsheet editor adapted from the complete [GridCraft](https://github.com/storytold/gridcraft) Rust source at commit `3926d21aa9a79d272b9f71ae60d67d7d7e5bd2c2`. The desktop binary is `gridstrike`; internal `gridcraft-*` crate names and native workbook behavior remain. The workspace also retains donor CLI and web source as development/reference targets. No hosted service is required for the desktop workflow.

**Status: early source preview.** On Windows, the native desktop target compiled with the pinned lockfile after a CSV export safety fix. A self-authored workbook was edited, saved and reopened as XLSX with formula source and calculated value intact, edited with undo/redo, and exported to CSV. Chart/XLSX round-trip and a grid selection helper test passed. These are command/model fixtures, not a physical pointer, keyboard, screen reader or visual desktop acceptance test. Crash recovery, broad XLSX fidelity, malformed UI input, other platforms and binary packaging remain unverified. No release binary is provided.

## Earlier `.gstrike` projects

The earlier GridStrike archive is a different format. The explicit copy-based legacy import command uses the in-repository `legacy/gridstrike` reader; ordinary Open and file associations do not reinterpret `.gstrike`. The tested importer maps sheet order/names, active sheet, sparse values and formula source, recalculates formulas, and refuses unsupported or mismatched results rather than silently changing them. The old file and undo history remain untouched. Two importer integration tests passed on Windows. Keep the old file; review unsupported features and save the converted workbook to a new native path.

## Build from source

Use Rust 1.90 or newer with the lockfile and reviewed dependencies. The legacy reader is included inside this repository, so no sibling checkout is needed.

```powershell
cargo fetch --locked --target x86_64-pc-windows-msvc
cargo build --locked --offline -p gridcraft --bin gridstrike
cargo test --locked --offline -p gridcraft-engine --test legacy_import
```

The original GridCraft `LICENSE-MIT`, `LICENSE-APACHE` and `NOTICE` are preserved. [ADAPTATION.md](ADAPTATION.md) describes the current source changes, [ATTRIBUTION.md](ATTRIBUTION.md) distinguishes included assets from donor history, and [UPSTREAM_ATTRIBUTION.md](UPSTREAM_ATTRIBUTION.md) retains the donor inventory. A complete transitive dependency and binary notice audit remains before a release.
