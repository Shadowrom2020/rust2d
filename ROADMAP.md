# rust2d roadmap

Planned features of rust2d, grouped into themed releases, with the design decisions and the
code hook points already worked out so that a session can start an item without re-exploring the
codebase. What is shipped is in [CHANGELOG.md](CHANGELOG.md); the rules for branches, versions
and releases are in [AGENTS.md](AGENTS.md); coding and verification rules in [CLAUDE.md](CLAUDE.md).

Status marks: `[ ]` open · `[~] feature/<branch>` in progress · `[x] X.Y.Z` shipped.

## How to work from this roadmap

Instructions for every session (people and coding agents) that implements roadmap items:

1. **Pick the item.** Unless the user names one, take the first open item of the lowest
   unreleased version, in the order listed (later items build on earlier ones). Read its whole
   block: *Goal*, *Design* (defaults are decided; keep them unless the user says otherwise),
   *Touch points*, *Example*, *Verify*. Line numbers are not given on purpose: find the named
   functions and structs with grep, they move.
2. **Branch.** `git checkout develop && git pull --ff-only && git checkout -b feature/<item-slug>`
   (the slug is given in the item's heading). One item per branch; a release's items are
   separate pull requests into `develop`.
3. **Mark it.** Change the item's mark to `[~] feature/<slug>` in the first commit of the branch
   and to `[x]` (version left empty until the release) in the last one. Keep the *Design* block
   true: if the implementation deviates, edit the block so the roadmap matches the code.
4. **Compatibility is a hard rule** (see CLAUDE.md): new `Action`, `Cond`, `TriggerOn`,
   `ZoneKind`, `OnUnlock` variants are appended; new fields get `#[serde(default)]`; new map
   object fields go into the RON `Meta` of `map/io.rs`; save games (`WorldState`) and
   definitions of every released version must still load. Add a test that parses an old RON
   without the new fields when you add fields to `WorldState` or a definition.
5. **A feature is done when** it works in the running binaries, is covered by the
   scripted checks in *Verify* (autoplay / editor script + screenshots, on a scratch copy of
   the example project), has tests where the engine logic allows, is documented
   (`README.md`, `Documentation/scripting.md` for every script function, the editor's `API`
   table, CHANGELOG *Unreleased*), and `cargo build / clippy -D warnings / test --workspace`
   pass. The example project (`crates/engine/examples/gen_example/`) shows the feature: add
   new entities at the end of a map function so uids stay stable, then regenerate.
6. **Commit, push and open pull requests only when the user asks.** When all items of a
   version are `[x]` (or the user decides to ship early), cut `release/X.Y.Z` as described in
   AGENTS.md, fill in the version in the marks, and move the version's section to *Shipped*
   at the end of this file (one line per item).
7. **Changing the roadmap** is fine: reorder, move items between versions, split or merge
   them, add new ideas to *Backlog*. Keep every item's block in the same shape. Don't delete
   design notes of open items; replace them.

## 0.4.0 — Editor productivity (creator tools)

Theme: faster map making and faster iteration on scripts.

### [ ] Region copy, cut and paste — `feature/region-clipboard`

**Goal.** Select a rectangle of the map (all layers, collision, entities), Ctrl+C / Ctrl+X /
Ctrl+V, paste follows the mouse as a preview, undoable.

**Design.** `Selection::Region(rect)` set by dragging with the Select tool; the clipboard is a
`StructureDef` built with `StructureDef::capture` and pasted with `doc.place_structure`
(`Edit::Group`), preview drawn like the Structures tool. Editor script commands `region X Y W H`,
`copy`, `cut`, `paste X Y`. Ctrl+C/X/V are unbound today.

**Verify.** Editor script copies a house region and pastes it elsewhere; `dump` / screenshot;
undo restores; cross-map paste after `open`.

### [ ] Script hot reload and in-game console — `feature/live-scripts`

**Goal.** Edit a script while the play-test runs and see it update; a console to poke the game.

**Design.** The editor starts the play-test with `--dev`; the game then polls the mtimes of
`scripts/*.rhai` every 0.5 s and calls `ScriptHost::load_dir` again (errors → toast + log). F4
toggles a console panel (text input like the creator's name field via `input::next_char`): the
autoplay vocabulary (`call FN`, `settime H`, `give ITEM N`, `flag NAME on|off`, `var NAME V`,
`teleport MAP SPAWN`, `hp N`, `xp N`, `effects`, `status`). Only with `--dev`, so exported games
don't expose it.

**Verify.** Autoplay with `--dev`: change a script file from the test harness mid-run, `call` it,
output changed; console screenshot; the released game ignores F4.

### [ ] Auto-tiling terrains — `feature/autotile`

**Goal.** Paint "grass" or "water" and the editor picks edge and corner tiles itself.

**Design.** `TilesetDef.terrains: Vec<Terrain { name, tiles: [u32; 16] }>` (4-bit blob: one
tile per combination of same-terrain neighbours N/E/S/W; a 47-tile corner variant can come
later). New tool *Terrain* (T): painting sets the centre tile and re-evaluates the eight
neighbours inside the stroke (undoable through `Edit::Tiles`). The Tilesets form gets a terrain
editor (pick the 16 tiles by clicking into the tileset, with a preview). The example generator
draws grass/dirt and grass/water edge tiles and defines the terrains; the starter library picks
them up (bump `STARTER_RULES`). This is the biggest item of the release because of the art.

**Verify.** Editor script: `tool terrain`, `terrain grass`, `drag …`, screenshot shows edges;
undo; a map painted with v0.3.0 looks unchanged.

## 0.5.0 — Living world

Theme: the world moves without the player: weather, routines, companions, crafting, portraits.

### [ ] Weather — `feature/weather`

**Design.** Per-map `weather: Weather { kind: None|Rain|Snow|Fog, intensity }` in the map
RON `Meta`; `Action::SetWeather` / script `set_weather(kind, intensity)` (saved override in
`WorldState.weather`). A particle layer drawn between the light pass and `set_default_camera`
(moves with the camera), darker ambient for rain, built-in rain loop sound. The editor's
lighting preview gets a weather toggle; the map inspector a weather row.

**Verify.** `set_weather` via `call`, screenshots day and night; save / load keeps it.

### [ ] NPC schedules — `feature/schedules`

**Design.** `EntityInstance.schedule: Vec<Stop { hour, x, y }>` (same map): at each hour the NPC
walks to the stop with the patrol machinery (`patrol_step`, A*) and waits there; the editor edits
stops on the map like patrol routes. Night lines use the existing `TimeBetween` condition.

**Verify.** `settime` jumps, `status` shows the NPC at the stop; screenshot.

### [ ] Companions — `feature/companions`

**Design.** `Behavior::Follow` for placed NPCs plus script `follow("name")` /
`stop_following("name")` and the matching actions; followers path to the tile behind the player,
are carried across maps (`WorldState.companions`: definition id, HP, gear), attack hostiles in
reach with their weapon (reuse `attack_at` with an attacker parameter) and are excluded from the
player's melee targeting (`attack_at` filters on `hostile`, which also stops hitting friendly
NPCs today). Enemies target the nearest of player and companions.

**Verify.** `call` a function that recruits the guard, walk through a transition, `status` lists
the companion on the new map; combat screenshot; save / load.

### [ ] Crafting — `feature/crafting`

**Design.** New definitions file `recipes.ron` (`RecipeDef { id, result, count, ingredients:
Vec<ItemStack>, station }`; `project.rs` DEFS, `release.rs`, `library.rs`), `Action::OpenCrafting
{ station }` / script `open_crafting("anvil")`, a crafting window modelled on `shop_ui.rs` (rows
= recipes, greyed when ingredients are missing). Editor: Database tab *Recipes*; `refactor.rs`
renames and `refs_of_*` for library export.

**Verify.** Autoplay opens the window at a bench prop, crafts, `status` inventory changed.

### [ ] Dialog portraits — `feature/portraits`

**Design.** Dialog boxes show a portrait of the speaker (paperdoll of the NPC's look, or the
player's) at the left; nothing to configure per character, one project option turns it off.
A proper node-graph dialog editor stays in the backlog.

**Verify.** Screenshot of a dialog with the elder; the player's own lines show the hero.

## 0.6.0 — Beyond Linux (reach)

Theme: games made with rust2d reach more players.

### [ ] Windows export — `feature/windows`

**Design.** Gate the `std::os::unix` uses (`editor/files.rs` copy permissions,
`packaging/lib.rs` symlink and executable bit, `release.rs` chmod); Windows paths for saves and
settings (`release::user_saves_dir`, `Settings::dir`: `%APPDATA%`); `.exe` handling in
`export::game_binary` and `release::export` (`<slug>-windows.zip`, README text per platform).
CI builds `rust2d-game.exe` with `x86_64-pc-windows-gnu` in the Packages and Release jobs and
the toolkit packages ship it next to the Linux runner, so the Linux editor exports Windows games
(*File → Export game* gains a *Windows* checkbox). The editor on Windows is a later item.

**Verify.** Export from the editor produces the zip; run the exe under Wine for a `status` run
if Wine is available, otherwise a CI smoke test on `windows-latest`.

### [ ] Web export — `feature/web`

**Design.** Game only (`wasm32-unknown-unknown`): async asset loading (`audio.rs` `poll_now`
assumes instant loading), saves and settings in `localStorage`, no `std::env` / `current_exe`,
`SystemTime::now` → `web-time`, the exported game as an asset tree plus a generated
`index.html` for itch.io. Plan in detail only after Windows ships.

### [ ] Localization — `feature/localization`

**Design.** Content texts (dialogs, choices, item / prop / quest / skill names and descriptions)
through `lang/<code>.ron` lookups keyed by the original string with `expand_text` as the
chokepoint; the ~170 hard-coded UI strings in the runtime move to one table behind `t("…")`;
Options row *Language*; the editor exports a translation template. Large but mechanical.

## Backlog (unscheduled ideas)

- Event / dialog graph editor (node view of triggers, choices and script calls).
- Achievements (quests, secrets, counters; a page on the title screen).
- Import of Tiled maps, LPC sprite sheets and Aseprite exports.
- 47-tile corner auto-tiling; parallax background layers.
- Fishing / lockpicking minigames; swimming and ladders.
- Editor on Windows; macOS builds.

## Shipped

See [CHANGELOG.md](CHANGELOG.md) for the details of every release.

### 0.3.0 — Heroes, foes and feel (2026-11-06)

- [x] Experience and levels — `feature/progression`
- [x] Status effects — `feature/status-effects`
- [x] Enemy AI — `feature/enemy-ai`
- [x] Bestiary — `feature/bestiary`
- [x] Dialog log — `feature/dialog-log`
- [x] Input actions and gamepad — `feature/gamepad`
- [x] Key rebinding — `feature/rebinding`
- [x] Screen shake and damage flash — `feature/juice`
- [x] Save thumbnails — `feature/save-thumbnails`

### 0.0.1 to 0.2.0

Editor, combat, quests, shops, skills and magic, lighting, cut-scenes, packaging, GitHub
workflow (before this roadmap existed).
