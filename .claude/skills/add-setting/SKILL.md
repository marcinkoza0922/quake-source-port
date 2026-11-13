---
name: add-setting
description: Add or change a user-facing setting in this Quake port — a cvar, its options-menu row (toggle, slider, choice), tooltip, sub-page, master-toggle gating, and the docs — following the project's existing wiring. Use whenever a feature needs an on/off switch or tunable, the user asks for "an option", "a toggle", "a slider", "a menu entry", "a setting", or a new Graphics/Gameplay/Accessibility/HUD/Controller/Assists option, or when a cvar from another engine must be accepted so configs load quietly.
---

# Adding a setting

Settings are cvars. One registry is shared: the host's `CmdContext::cvars`
is `server.core.cvars`, so host, server and QC all see the same values. A
complete option touches five places. Missing one gives a row with no help
text, a value that isn't saved, or a sub-page that can't be reached.

## 1. Register the cvar

- **Host/client/render options**: add `("name", "default", flags)` to
  `HOST_CVARS` in `src/host/cvars.rs`, next to related ones. Use
  `CVAR_ARCHIVE` when the user's choice should persist in `config.cfg`
  (almost every menu option), and `0` for tuning or debug knobs.
- **Gameplay rules the server applies** (assists, autosave rules, `sv_*`):
  register them in the server crate, the way `crates/qserver/src/assists.rs`
  keeps its own table. That code runs in qbot and dedicated servers, where
  there is no host.
- **Cvars from other engines that we accept but ignore**: add them to
  `COMPAT_CVARS` in `src/host/cvars.rs`, so vkQuake/ironwail/mod configs load
  without "unknown command" noise. Only real cvars (like `gamecfg`) get
  behaviour.
- Naming: follow the existing prefixes, e.g. `r_` render, `r_debris_*`
  sub-options, `cl_` client, `v_` view, `scr_` screen/HUD, `hud_`, `joy_`
  gamepad, `in_` input, `a11y_` accessibility, `assist_` gameplay assists,
  `sv_` server, `ui_` menus. New optional effects default to **off**.
  Gameplay-changing options are disabled in deathmatch.

## 2. Read it

- In the host, `self.cv("name")` returns an `f32` (`src/host.rs`).
- The renderer and other crates never read cvars. The host copies values
  into plain settings structs each frame, e.g. `src/host/render.rs` fills
  `qrender::Settings` and `debris::Settings`. Add a field there, not a cvar
  lookup inside `qrender`.
- Clamp at the point of use (`.clamp(0.0, 4.0)`), since configs and the
  console can set anything.

## 3. Menu row (`src/menu/pages.rs`)

Add it to the page's `&[Item]` const (`GAMEPLAY`, `HUD`, `INPUT`, `MOUSE`,
`CONTROLLER`, `GRAPHICS`, `WATER`, `DEBRIS`, `AUDIO`, `ACCESSIBILITY`,
`ASSISTS`, `AUTOSAVE`, ...):

```rust
toggle("Kill Lift", "r_debris_killlift"),
slider("Push Strength", "r_debris_force", 0.0, 6.0, 0.25),
Item::Choice { label: "Ragdoll Start", cvar: "r_debris_ragdollstart", choices: RAGDOLL_START },
// with: pub(super) const RAGDOLL_START: &[(&str, f32)] = &[("after animation", 0.0), ("at death", 1.0), ("mixed", 2.0)];
```

`Item::Text` is for string cvars and `Item::Sub { label, page }` opens a
nested page. Labels are short and title case. Long pages scroll, but put
the important rows first.

## 4. Tooltip (`src/menu/page_info.rs`)

Add a `"cvar" => "One plain sentence on what it does"` arm to `tooltip()`.
Every slider, toggle and choice is expected to have one; it is shown under
the panel. Mention caveats there ("single player only", "not in deathmatch").

## 5. New sub-page (only if needed)

1. Add a variant to `pub enum Page` in `src/menu.rs`.
2. Add a `const NAME: &[Item]` in `pages.rs` and an
   `Item::Sub { label: "X Settings", page: Page::X }` row on the parent page.
3. In `page_info.rs`, add `page_items` and `page_name` arms. `parent_page`
   only searches Gameplay, Input, HUD, Accessibility and Graphics, so extend
   that list if the parent is another page.
4. If a master toggle controls the page, add it to `sub_disabled` (like
   Water/`r_enhancedwater` and Debris/`r_debris`). The row is then dimmed and
   refuses to open while the master is off.
5. Optionally add a name to the `menu_options` match in
   `src/host/commands.rs`, so `menu_options <name>` opens it directly (handy
   for screenshots).

## 6. Verify and document

- `cargo nextest run -p quake-source-port menu` runs the menu tests.
  `src/menu/tests.rs` has patterns for sub-page navigation (`options_subpages_open_and_return_to_parent_row`)
  and dropdowns. Add one when you add a page or unusual row behaviour.
- Look at the page in a headless screenshot (visual-check skill):
  `"+delay 2 menu_options graphics"`. Check that the label fits and the row
  doesn't collide with the hint row or scrollbar.
- Persistence comes from `CVAR_ARCHIVE` alone: `write_config` writes every
  archived cvar. Don't check it with a windowed run that exits normally,
  because that rewrites the user's real `config.cfg` with your overrides.
- `docs/DESIGN.md`: in the feature's section, give the cvar name, default,
  range, menu location and any gating, in the style of the existing
  "Accessibility" / "Ragdolls and debris" paragraphs.
