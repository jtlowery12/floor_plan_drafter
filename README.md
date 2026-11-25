# Drafting Table

A layman's architectural drafting table. Enter a square footage, get a to-scale
skeleton footprint, then draw your ideal home with rooms, walls, doors, windows
and furniture. Export a dimensioned PDF/PNG/SVG to hand to an architect.

Everything is stored locally as one JSON file per plan. No account, no network.

- Data folder: `DRAFTING_TABLE_DIR` if set, otherwise your OS app-data folder (`DraftingTable`).
- Units: feet & inches or metric, switchable at any time.
- Tools: rectangle, ellipse, triangle, custom polygon, wall, door, sliding door, window, furniture (17 symbols).
- Multiple floors, undo/redo, snap grid, live area meter against your target.
- Shortcuts: V select, H pan, R/E/T/P shapes, W wall, D/S/N openings, F furniture, G grid, Ctrl+Z/Y, Ctrl+D duplicate, Ctrl+R rotate, Ctrl+0 fit, Delete.

**Area note:** the meter adds up each room's area, so overlapping rooms count twice. Keep rooms side by side.

## Build

    cargo run --release

Linux needs the usual GTK/X11/Wayland development packages (see the CI workflow).

## Licenses of bundled assets
Inter and Playfair Display (SIL OFL 1.1), Material Icons (Apache 2.0). See `assets/fonts/`.
