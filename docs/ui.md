# Control panel (UI)

The UI is [egui](https://github.com/emilk/egui), hosted by `bevy_egui`.

## Panel

`ui::panel::ui_panel` runs in `EguiPrimaryContextPass` and draws a single
floating panel pinned to the top-left. It reads and writes `UiState` — the same
state the sim boundary converts to parameters — so the panel is the user's only
handle on the app. It also shows the FPS and any solver error reported in
`SimState`.

The panel's width is responsive; `responsive_panel_width` (crate root) reserves
the horizontal space the 3D viewport is offset by, so the figure is never hidden
behind the panel.

## Theme and widgets

- `ui/theme.rs` — the palette (`ACCENT = #00D2BE`, dark surfaces) and the panel
  geometry (`PANEL_WIDTH = 328`, `PANEL_MARGIN = 24`, `PANEL_GAP = 16`), lifted
  from the design mock-up so the panel and the 3D stage match.
- `ui/widgets.rs` — the small custom controls (section labels, hairlines, the
  large numeric readout).
- `ui/fonts.rs` — embeds **DM Sans** (UI text) and **DM Mono** (numeric
  readouts), installed once in `PreUpdate` **before** egui's `begin_pass` so the
  custom families are bound from the first frame.

## State

`UiState` is the single source of truth. Its `Default` derives every physical
field from the sim module configs, so the UI and the solver cannot drift apart.

---

[← Previous: Rendering](rendering.md) · [Next: Decision records →](decisions/0001-two-gpu-contexts.md)
