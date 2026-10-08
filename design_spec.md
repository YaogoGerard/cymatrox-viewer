# Design Spec: Modern, Functional UI for cymatrox-viewer (egui)

## 1) Visual Design Language

### Color palette (dark-first)
| Token | Hex | Usage |
|---|---|---|
| Background | `#0B0B14` / `#12121E` | App bg / panel base. Prefer `#0B0B14` for deeper look, keep `#12121E` as fallback. |
| Surface/Card | `#151522` | Group containers, cards. |
| Surface/hover | `#1B1B2B` | Hovered interactive elements. |
| Border | `#242438` | Card borders, separators. |
| Accent | `#00D2BE` | Active states, highlights, primary accent (keep current). |
| Accent-dim | `#008C82` | Focus rings/accents on hover. |
| Text/primary | `#F4F4F8` | Headings/body. |
| Text/secondary | `#B9B9C9` | Labels, units. |
| Text/tertiary | `#8C8CA0` | Subtitles, hints. |
| Success/streaming | `#00D2BE` | "STREAMING" dot/text. |
| Warning/starting | `#F2A65A` | "STARTING" state. |
| Error | `#D65454` | Errors/border/banner text. |
| Error-bg | `#461818` | Error banner background. |

Keep accent cyan to preserve identity. Slightly darker base improves contrast vs current `#12121E`.

### Spacing (4pt grid)
| Scale | px | Usage |
|---|---|---|
| xs | 4 | Tight gaps, icon-text. |
| sm | 8 | Item spacing between related controls. |
| md | 12 | Card inner padding. |
| lg | 16 | Section spacing between groups. |
| xl | 20 | Panel top/bottom padding. |
| 2xl | 24 | Major section breaks. |

Use `egui::Margin::same(md)` for cards, `item_spacing = vec2(sm, sm)`.

### Typography
| Style | Size | Weight | Color | Usage |
|---|---|---|---|---|
| H1 | 22–24 | Bold | Text/primary | "CYMATROX" title. |
| H2 | 13–14 | Bold/600 | Accent/Text-primary | Card headers (DRIVING/PHYSICS/STATUS...). |
| Body | 11–12 | Normal | Text/secondary | Labels, units. |
| Value | 12 | Medium | Text/primary | Slider values/display. |
| Caption | 10–11 | Normal | Text/tertiary | Subtitle, voice readouts. |

Use `egui::RichText` consistently.

### Radius & elevation
- Card radius: 8–10px (`corner_radius(10)`)
- Button radius: 6–8px
- Border: 1px solid `Border`
- Subtle elevation via background/hover (no heavy drop shadow needed in egui)

## 2) Layout Structure

Left sidebar panel (`egui::Panel::left`) with responsive width (`responsive_panel_width`). New order:

1. **Header** (H1 + subtitle)
2. **Module selector** (segmented tabs) — replaces 3 buttons in a row
3. **Error banner** (conditional, when `sim_state.error`)
4. **Driving** (Frequency + Amplitude with inline units)
5. **Physics** (contextual): Granular → SOLVER group; Fluid → LIQUID group; Acoustic → minimal or nothing extra. Keep collapsible or flat groups (flat preferred to reduce clicks).
6. **Audio** (Voice control + live readouts)
7. **Status** (FPS, state pill, module, frame stats)

Avoid deep nesting. Group cards have header + body.

## 3) Component Specs

### ModuleTabs
- Segmented control: 3 tabs [Granular, Fluid, Acoustic]
- Active: filled accent, text `#0B0B14` (or primary on accent)
- Inactive: surface/card bg, text secondary
- Min height 30–32px, equal width split
- On click sets `ui_state.active_module`

### GroupCard
- `egui::Frame`: fill `Surface/Card`, corner_radius 10, inner_margin md, stroke Border (alpha 0.4)
- Header row: left label (H2, accent or primary), optional right toggle/collapse
- Body: vertical stack with `item_spacing sm`

### SliderRow
Props: label, &mut value, range, unit_str, logarithmic(bool)
Layout:
- Top row: label (left, secondary) + value+unit (right, value style) — e.g. `440 Hz`
- Bottom row: slider full width
- Use `egui::Slider::new` with `logarithmic` for freq; show inline value via formatting
- Respect clamping done by sim (or keep UI clamped)

### StatusPill
State derived: if `error` → red pill "ERROR"; else if any frame present for active module → cyan "STREAMING"; else → amber "STARTING"
Dot + text, compact.

### ErrorBanner
Visible only if `sim_state.error`. Red-tinted bg/border, title "GPU / simulation error", body `sim_state.status` (wrap if long). Keep concise.

### VoiceReadout
Show `Freq: {:.0} Hz`, `RMS: {:.4}` in caption style when enabled.

## 4) Wireframe (ASCII)

Left panel width ~260–300px.

```
┌─────────────────────────────────────────┐
│ CYMATROX        (H1, accent)             │
│ Interactive cymatics laboratory (caption)│
├─────────────────────────────────────────┤
│ [Granular] [Fluid] [Acoustic] (tabs)     │
├─────────────────────────────────────────┤
│ [ERROR BANNER if error]                  │
├─────────────────────────────────────────┤
│ DRIVING (card)                            │
│ Frequency        440 Hz                  │
│ ├─────────────────────────────────────┤   │
│ Amplitude        0.10 ×1 mm              │
│ ├─────────────────────────────────────┤   │
├─────────────────────────────────────────┤
│ PHYSICS (card, contextual)                │
│ Granular: Drag / Restitution / Coupling   │
│ Fluid: Density / Surface tension / Depth /│
│        Damping                            │
│ (Acoustic: empty or hidden)               │
├─────────────────────────────────────────┤
│ AUDIO (card)                               │
│ [✓] Enable mic input                      │
│ Freq: 440 Hz  RMS: 0.0123                 │
├─────────────────────────────────────────┤
│ STATUS (card)                              │
│ ● STREAMING        60 FPS                 │
│ Active: Granular  ·  4096 grains          │
└─────────────────────────────────────────┘
```

States: streaming/starting/error reflected in pill + banner. Module tabs switch contextual PHYSICS block.

## 5) State Representation
- Active module: drives tabs + contextual block + status text.
- `sim_state.error`: banner + pill red.
- Data presence: any of granular/fluid/acoustic Some → streaming.
- Voice: checkbox toggles; readouts shown only when enabled.
- Debounce: unchanged (timing REBUILD_DEBOUNCE 300ms) — UI must not change that.

## 6) egui Implementation Guidance
- Theme application: set dark visuals in `ctx.style_mut_of(egui::Theme::Dark, |style| { ... })` with new palette values. Keep selection/accent consistent.
- Widgets: prefer small helpers `module_tabs`, `group_card`, `slider_row` returning bool/changes? `Slider` already returns `Response`/changes via `ui.is_rect_visible` not needed. Just read `value` mutation.
- Avoid allocations in hot path: build strings via `format!` sparingly; status updates are infrequent. No heap churn in `ui_panel` loop.
- Layout: use `ui.horizontal`/`ui.vertical` with `item_spacing`. `ScrollArea::vertical` already used — keep.
- Responsive width: keep `responsive_panel_width(ctx.viewport_rect().width())`.
- Viewport offset: must remain tied to panel width (camera code unchanged).
- Migration: extract to `src/ui/`:
  - `theme.rs` — palette + `apply_dark_theme(ctx)`
  - `widgets.rs` — `module_tabs`, `group_card`, `slider_row`, `status_pill`, `error_banner`
  - `panel.rs` — `ui_panel()` implementation (moved from main)
  - `mod.rs` — re-exports
Keep resources/enums in `main.rs` or `app.rs` (separate later) — minimal refactor first.

## 7) Migration Plan
1. Create `src/ui/` directory.
2. `theme.rs`: constants + `apply_dark_theme`.
3. `widgets.rs`: component helpers.
4. `panel.rs`: move `ui_panel`, `styled_card`, `slider_row`, `responsive_panel_width`? Keep `responsive_panel_width` public if used by camera? camera uses `responsive_panel_width` (already pub). Move to `ui::layout` or keep in main and import. Or put in `ui/mod.rs`.
5. Update `main.rs`: remove `ui_panel`, `styled_card`, `slider_row` (or re-export), add `mod ui; use ui::panel::ui_panel;`
6. Wire systems: `add_systems(EguiPrimaryContextPass, ui_panel)` unchanged.
7. Verify behavior: ranges, units, debounce, latest-wins, voice, viewport, errors.

## 8) Acceptance Checklist
- [ ] All controls present with same ranges/units and labels
- [ ] Module tabs switch correctly; contextual block updates
- [ ] Frequency log scale preserved
- [ ] Debounce 300ms unchanged (sim logic untouched)
- [ ] Latest-wins params preserved
- [ ] Voice mapping unchanged
- [ ] Viewport offset uses panel width (no camera regression)
- [ ] Error banner + pill reflect state
- [ ] Status shows FPS + frame stats per module
- [ ] No allocations in tight UI loop
- [ ] Compiles with clippy clean

## 9) Minimal Example Snippet
```rust
// ui/widgets.rs (sketch)
fn module_tabs(ui: &mut egui::Ui, active: &mut ActiveModule) -> egui::Response {
  let mut resp = egui::Response::default();
  ui.horizontal(|ui| {
    for m in [ActiveModule::Granular,ActiveModule::Fluid,ActiveModule::Acoustic] {
      let sel = *active==m; let label=m.label();
      let btn = egui::Button::new(label).selected(sel);
      if ui.add(btn).clicked() { *active=m; }
    }
  }).response
}
```

Implement minimal: tabs + driving card + status pill first, then expand.
