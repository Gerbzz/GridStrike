# GridStrike source and asset attribution

The editor foundation derives from [GridCraft](https://github.com/storytold/gridcraft) at `3926d21aa9a79d272b9f71ae60d67d7d7e5bd2c2`, by the ArtCraft Team and GridCraft contributors. The donor `LICENSE-MIT`, `LICENSE-APACHE` and `NOTICE` are included byte for byte. [UPSTREAM_ATTRIBUTION.md](UPSTREAM_ATTRIBUTION.md) preserves the donor's historical asset table; its brand, icon and screenshot rows describe upstream files that are absent here.

The earlier first-party GridStrike archive reader is included under `legacy/gridstrike`, with its own package metadata declaring MIT OR Apache-2.0. The native app's GridStrike identity, legacy bridge and CSV safety changes are local adaptations. Upstream contributor metadata is omitted, so the About contributor table uses the build script's empty fallback.

The only committed image assets are the three GridStrike adaptation icons below. They depict a plain spreadsheet glyph, have no donor or third-party mark, and were added to the local adaptation. Their separate authorship record should be checked before binary packaging.

| Included file | Source | Use |
|---|---|---|
| `assets/app-icon/gridstrike-1024.png` | GridStrike adaptation | window/icon source |
| `assets/app-icon/gridstrike-256.png` | GridStrike adaptation | window icon |
| `assets/app-icon/gridstrike.ico` | GridStrike adaptation | Windows resource |

No external font files, promotional screenshots, captured data or user workbook files are committed. Optional `CRAFT_FONTS_DIR` fonts are not part of this source boundary. A complete transitive dependency license/notice inventory remains a release task.
