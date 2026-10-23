# Third-party code and assets taken from Zed

Source: Zed 1.24.2, git tag `v1.24.2`, commit `fb69067729cb16a7f6135e2c1600c26a28c59659`.
Our app is GPL-3.0-or-later (see `LICENSE`), which is compatible with everything below.

## Used as dependencies (not copied)
Pinned by git revision in the workspace `Cargo.toml` files.

| Crate | Licence | Used by |
|-------|---------|---------|
| `gpui`, `gpui_platform` (and its platform and utility crates) | Apache-2.0 | `spikes/gpui-spike`, `spikes/zed-ui-spike` |
| `ui`, `theme`, `component`, `icons`, `menu`, `ui_macros`, `syntax_theme` | GPL-3.0-or-later | `spikes/zed-ui-spike` |

## Copied or adapted files
| Our file | Original | Licence | Change |
|----------|----------|---------|--------|
| `spikes/gpui-spike/src/text_input.rs` | `crates/gpui/examples/input.rs` | Apache-2.0 | Removed the example app; added constructor, dark styling, ctrl key bindings |
| `assets/fonts/ibm-plex-sans/*`, `assets/fonts/lilex/*` | `assets/fonts/*` | SIL OFL (licence files kept alongside) | None |
| `assets/icons/*.svg`, `assets/icons/LICENSES` | `assets/icons/*` | Lucide ISC and Zed's own (see `LICENSES`) | Subset only |

Before copying any other file, add a row here.

## Added during M1
Dependencies (git, same pinned revision): `menu` (GPL-3.0-or-later) for the menu action names used to bind keyboard navigation in Zed's `ContextMenu`.

Copied assets: 41 SVG icons from `assets/icons/` (Lucide, ISC, and Zed's own set; `assets/icons/LICENSES` is kept with them). The list is whatever is in `assets/icons/*.svg` at the top level; `assets/icons/jm/` holds icons drawn for this project.

Adapted code: `crates/app/src/fields.rs` is a rewrite of the same GPUI `input` example (Apache-2.0) on top of our own tested text buffer; `spikes/gpui-spike/src/text_input.rs` remains the unmodified-logic copy from the spike.
