<p align="center">
  <img src="assets/branding/GridStrike.png" alt="Original GridStrike logo" width="160" height="160">
</p>

<h1 align="center">GridStrike</h1>

<p align="center">
  <a href="https://prowebdevsolutions.com"><picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/branding/PWDS-logo-orange.svg">
    <img src="assets/branding/PWDS-logo.png" alt="PWDS company logo" width="120">
  </picture></a>
</p>

<p align="center"><strong>Make room for the numbers.</strong></p>

GridStrike is a local spreadsheet editor for everyday workbook work. Open a sheet, edit cells and formulas, revisit changes with undo and redo, then save an XLSX workbook or export table data to CSV.

![GridStrike showing a quarterly sales workbook, formula totals, sparklines, and charts](assets/screenshots/gridstrike-sales-dashboard.png)

*A live GridStrike window displaying its original Quarterly Sales Dashboard sample.*

## Workbook workflows

The original sales sample combines a styled table, calculated totals, growth rates, sparklines, and two charts. In this bounded live run, GridStrike saved it as XLSX and reopened cell `G5` with formula `=SUM(C5:F5)` and value `$217,420` intact.

![GridStrike showing a household budget with conditional formatting and a comparison chart](assets/screenshots/gridstrike-budget-analysis.png)

*The separate Household Budget sample shows category totals, color cues, and a budget versus actual chart in the Insert workspace.*

<p align="center"><a href="#highlights">Highlights</a> · <a href="#build-from-source">Build from source</a> · <a href="assets/branding">Original artwork</a></p>

## Highlights

- Edit cells and formulas in a desktop grid.
- Save and reopen editable XLSX workbooks.
- Export tables to CSV for other tools.

## Current status

Early Windows source preview. Earlier bounded model tests covered edit, save, reopen, formula, and CSV workflows. The retained Windows desktop build also rendered both original sample workbooks in a live window; its sales workbook saved and reopened with formula source and calculated value intact. These checks do not establish broad XLSX fidelity, crash recovery, interactive accessibility, other platforms, or release packaging.

## Workbook formats

The bounded Windows check saved and reopened an XLSX workbook with its formula source and calculated value intact, then exported CSV for table exchange. XLSX remains the editable workbook in that tested path, while CSV carries table values for other tools. Earlier `.gstrike` archives use a separate format and require the explicit copy-based conversion below; ordinary Open does not silently reinterpret them.

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

The GridStrike app logo remains the original suite artwork. The PWDS company mark is the existing official logo from [PWDS-Website](https://github.com/Gerbzz/PWDS-Website/blob/main/public/prowebdevsolutions-logo.png); its orange variant is the [official site asset](https://prowebdevsolutions.com/images/prowebdevsolutions-logo-orange.svg). [Screenshot provenance](docs/SCREENSHOT_PROVENANCE.md) records the live capture and sample workflow.

## Source and verification notes

GridStrike adapts the complete [GridCraft](https://github.com/storytold/gridcraft) Rust source at `3926d21aa9a79d272b9f71ae60d67d7d7e5bd2c2`. The desktop binary is `gridstrike`; retained `gridcraft-*` crate names, CLI, and web sources remain development/reference targets. The Windows checkpoint compiled with the pinned lockfile after a CSV export safety fix. A self-authored workbook was edited, saved and reopened as XLSX with formula source and calculated value intact; chart/XLSX round-trip and a grid selection helper also passed bounded model tests. The screenshots above come from a separate live desktop run through the app's built-in control channel, not a full interactive acceptance pass.
