# Legacy GridStrike reader

This package is the earlier first-party GridStrike implementation copied into the native workspace as a read-only dependency for explicit `.gstrike` import. Its source, manifest and synthetic tests are preserved from the tested local reader. The package metadata declares MIT OR Apache-2.0. It is not the current desktop executable; native `gridstrike` is built from `apps/gridcraft`.

The importer copies compatible workbook content into a new GridCraft session. It does not overwrite the original archive, merge undo history, or enable an old-format file association.
