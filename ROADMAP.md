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
8. **Record what you decided.** When the implementation deviates from the *Design* block, add
   a *Decided while building* paragraph to the item (before *Touch points*) instead of
   rewriting the design; later items and the release notes rely on it. The *Verify* block
   lists what was actually checked; say in the report what was compile-checked only (e.g.
   editor fields below the visible area, which the editor script cannot scroll to).
9. **Scripted checks that hold up.** Park the autoplay mouse before navigating menus
   (`drag 2 2 2 2` as the first command), or the real pointer over the test window changes
   the selection. Start from a crafted `saves/quick.ron` (`press f9`) to skip arrival
   cut-scenes; a pending dialog blocks autosaves and queued actions (`press e` twice). Add a
   `status()` field for every new state (`attacking=`, `shake=`, `pad=`, …) rather than
   reading screenshots for it; use screenshots for looks. Never delete with a relative path
   after `cd`; use a fresh scratch folder instead.
10. **Dependencies and packages.** A new crate that links a system library (gilrs → libudev)
    means: the CI apt lines (`ci.yml`, `release.yml`), the README's build prerequisites, a
    check that the release game still needs glibc ≤ 2.34 (`objdump -T target/release/
    rust2d-game | grep GLIBC_`), the Debian 12 container (`ldd`), and the Flatpak permissions
    (`--device`, `--socket`) in `editor/src/package.rs` and `xtask/src/dist.rs`.
11. **Example content.** Regenerate with `cargo run -p rust2d-engine --example gen_example`
    and commit `example_project/` and `crates/editor/assets/starter.r2dpack` together; new
    entities go at the end of a map function; an example id overrides the starter library's
    (give new example characters their own ids); never touch `example_project/saves/`.
12. **Branches that touch the same files.** Start the second item's branch from the first one
    (as *Save thumbnails* did from *Screen shake*, both in `menu.rs`), open the pull requests
    in that order, and say so in the pull request text. One pull request per item.
13. **Documentation per item**, all before the report: README (feature bullet; controls, events
    and scripting summaries), `Documentation/scripting.md` for every script function (a test
    fails otherwise), the editor's `API` table, CHANGELOG under the matching `###` heading of
    *Unreleased* (Gameplay, Editor, Fixes, Maintenance), the roadmap mark.
14. **CI hiccups.** A job that fails within a minute with a download timeout
    (`static.rust-lang.org`) is the runner's network: `gh run rerun <id> --failed`.
15. **Cutting a release.** `release/X.Y.Z` from develop: version in `Cargo.toml` (the build
    updates `Cargo.lock`), `## Unreleased` → `## X.Y.Z — date`, README *Current version*,
    the version's roadmap section replaced by one line per item under *Shipped* (its design
    notes live on in the CHANGELOG and the code), `cargo xtask check-version release/X.Y.Z`,
    pull request into master, merge after the four checks, merge the back-merge pull request.

## 0.4.0 — The workbench (editor productivity and content pipeline)

Theme: faster map making, faster iteration on scripts, a graph of the game's events, and
bringing existing assets in.

### [x] Region copy, cut and paste — `feature/region-clipboard`

**Goal.** Select a rectangle of the map (all layers, collision, entities), Ctrl+C / Ctrl+X /
Ctrl+V, paste follows the mouse as a preview, undoable.

**Design.** `Selection::Region(rect)` set by dragging with the Select tool; the clipboard is a
`StructureDef` built with `StructureDef::capture` and pasted with `doc.place_structure`
(`Edit::Group`), preview drawn like the Structures tool. Editor script commands `region X Y W H`,
`copy`, `cut`, `paste X Y`. Ctrl+C/X/V are unbound today.

**Decided while building.** A region drag starts on empty space, or anywhere with Shift (big
zones would otherwise make most of a map unselectable); a plain click still deselects. Copy
captures with the ground layer, so a paste is an exact copy of the rectangle (empty upper cells
clear the target). Cut and Del empty every layer (the ground too: the map background shows),
reset collision to the tileset's and remove the entities standing inside, as one undo step
(`MapDoc::clear_region`). Ctrl+V arms the clipboard on the Select tool like an armed structure:
every click pastes, Esc / right-click / another tool stops; the pasted rectangle becomes the
selection. Zones are never copied (their names are link targets). Pasted named entities get a
free name (`guard` → `guard_2`, `guard_2` → `guard_3`). A selected entity alone can be copied
too (a 1×1 clipboard without tiles). The clipboard lives in the editor process, so it survives
map switches but not a restart, and the Asset Library window has its own. The region inspector
offers Copy, Cut, Paste, Clear and *Save as structure…* (the capture window); the Edit menu has
Cut, Copy, Paste. Extra script command `paste` without coordinates arms the clipboard (with
`hover` for the ghost), `unpaste` stops; `status` prints `sel=`, `clipboard=`, `pasting=`.
Code in `editor/src/clipboard.rs`.

**Verify.** Done: unit tests for cut + paste + undo on a `MapDoc`, unique names and region
clipping. Editor script on a scratch copy: Select-tool drag over the village's west house gives
`sel=Region((16, 2, 10, 7))`; `copy` reports 10×7 tiles and 2 entities (the barrels); `paste` +
`hover` shows the ghost; a click pastes it centered on the cursor (barrels added, region
selected); `undo` removes it; `cut` empties the house and its barrels, `undo` restores them;
copying the elder and pasting gives a selectable `elder_2`; `open forest` keeps the clipboard and
`paste 3 3` puts the house among the trees. Screenshots of the region outline, ghost, paste, cut,
forest paste and the region inspector. Compile-checked only: the Ctrl+C / X / V keys (the editor
script cannot press keys), the Edit menu entries and the inspector buttons (egui clicks).

### [x] Script hot reload and in-game console — `feature/live-scripts`

**Goal.** Edit a script while the play-test runs and see it update; a console to poke the game.

**Design.** The editor starts the play-test with `--dev`; the game then polls the mtimes of
`scripts/*.rhai` every 0.5 s and calls `ScriptHost::load_dir` again (errors → toast + log). F4
toggles a console panel (text input like the creator's name field via `input::next_char`): the
autoplay vocabulary (`call FN`, `settime H`, `give ITEM N`, `flag NAME on|off`, `var NAME V`,
`teleport MAP SPAWN`, `hp N`, `xp N`, `effects`, `status`). Only with `--dev`, so exported games
don't expose it.

**Decided while building.** Reloading uses a new `ScriptHost::reload_dir`: it compiles every
file first and swaps the scripts only if all compile, so a half-typed edit never removes working
functions; the watch compares mtime and size, so new and deleted files count too. The console
pauses the game while open (like the character creator: typing must not move the player).
Commands that only change state run at once; actions that take time (dialogs, teleports,
walking, from `teleport` or a called script) go to the action runner and run once the console is
closed. Instant actions run through a separate `ActionRunner`, because the runner (not
`execute`) handles items, flags and variables. Any line that isn't a command is evaluated as Rhai
with the project's functions (`ScriptHost::eval`), and the value is printed. Script `print`
output is captured for the console (`take_printed`). Extra commands: `take`, `reload`, `clear`,
`help`. Up / Down recall earlier lines; Esc clears the line first, then closes; F4 closes. The
console uses the raw F4 key, not a rebindable action, so it never appears on the Controls page.
A released game (the bundle next to the executable) refuses `--dev` with a message. Autoplay
`console LINE` runs a line directly; `status` starts with `dev=off` or `dev=on console=…
reloads=N reload_error=B`. Code in `runtime/dev.rs`.

**Verify.** Done: unit tests for reload (a broken edit keeps the old function, a fix and a new
file load) and console evaluation. Autoplay with `--dev` while the test harness rewrote a script
three times: `call probe` printed v1, then v2 after the edit, then v2 again with
`reload_error=true` after a syntax error (error toast screenshot), then v3 after the fix.
Console driven by keys (`press f4`, `type`, `press enter`) from a quick save: `give potion 2`,
`flag`, a Rhai expression, `set_var(…)`, a syntax error, `xp 50`, `help`, history with Up all
work (screenshots); `teleport forest cave` happens after Esc closes the console; `call
guard_talk` shows its dialog after closing. A play-test started by the editor (new editor script command `play`, with `RUST2D_AUTOPLAY` inherited by the game) reports `dev=on`. Without `--dev` F4 does nothing; the exported game
started with `--dev` prints the refusal and reports `dev=off`. Not checked by a script: saving
from the editor's script editor during a real play-test (the same file write as the harness).

### [x] Auto-tiling terrains — `feature/autotile`

**Goal.** Paint "grass" or "water" and the editor picks edge and corner tiles itself.

**Design.** `TilesetDef.terrains: Vec<Terrain { name, tiles: [u32; 16] }>` (4-bit blob: one
tile per combination of same-terrain neighbours N/E/S/W; a 47-tile corner variant can come
later). New tool *Terrain* (T): painting sets the centre tile and re-evaluates the eight
neighbours inside the stroke (undoable through `Edit::Tiles`). The Tilesets form gets a terrain
editor (pick the 16 tiles by clicking into the tileset, with a preview). The example generator
draws grass/dirt and grass/water edge tiles and defines the terrains; the starter library picks
them up (bump `STARTER_RULES`). This is the biggest item of the release because of the art.

**Decided while building.** The tool's key is **A**: T was already the Transition tool.
`Terrain { name, tiles: [Option<u32>; 16], base: Option<u32>, joins: Vec<String> }` in the new
`engine/src/autotile.rs`; a missing tile falls back to the inside (`tiles[15]`). `base` is what
erasing (Shift) leaves, e.g. grass; if the base is another terrain's tile the cell is re-tiled
as that terrain. Which cells belong to a terrain is read from their tiles, so maps store nothing
new; a tile should belong to one terrain (the painted terrain wins for shared tiles). Only the
four orthogonal neighbours matter. Cells outside the map count as every terrain (terrains
continue past the edge). **Joins**: terrains lying on another count as it for its edges (Sand
joins Shore, Forest floor joins Snowfield / Forest path / Stream), and painting re-tiles every
neighbour with its own terrain; joins refer to names and follow renames. `autotile::paint`
(one cell, the editor through `MapDoc::paint`, one stroke = one undo step) and
`autotile::retile` (a whole layer with a membership function; the example generator's
`autotile_terrain`). The terrain editor: 4×4 slots in mask order with arm icons, click a slot
then a tile (the next slot follows), *Fill from a 4×4 block*, a base slot, *Joins* checkboxes, a
sample preview; right-click with the tool picks the terrain under the cursor.
Art and example (asked for after the first version: all starter tilesets and all example maps):
a shared `edge_block` builder (inside / surroundings drawers, wobbly or straight sides, rim
colours) draws 4×4 blocks below each sheet, two per band of four rows; the existing tiles keep
their pixels and indices (checked), so libraries take the grown images. Terrains: *terrain*
(Dirt path, Pond, Sand, Shore, Forest floor, Snowfield, Forest path, Stream, Flagstones, Rug;
rows 5–24), *dungeon* (Earth, Chasm), *desert* (Oasis, Hardpan), *mountains* (Snowfield,
Mountain path, Frozen lake), *city* (Canal, Plaza). Insides reuse the ordinary tile where it is
unique (so painted areas match the maps); Shore, Stream and Forest path have their own inside
tiles, the water ones animated with the water frames. Water and chasm edge tiles are solid.
Maps (tiles only, uids unchanged): village roads up to the doors, the sandy pond (Sand + Shore)
and a new duck pond with a footpath; forest floor, forest path (a dirt path on the clearing's
grass, extended two tiles into it), stream joined to the bridge, shrine flagstones; Wilds
lakes, sand, forests, snow; rugs in the house and shop; earth patches and a chasm in the mine.
Existing asset libraries: starter rule 5 gives a tileset with no terrains and the same (or an
upgraded) image the pack's terrains with their solid flags and animations.

**Verify.** Done: unit tests for masks, painting, erasing, neighbours, the map edge, other
tileset slots, joined terrains, whole-layer retiling, old tileset RON without the field, and the
library rule. After regenerating, the old part of all five tileset images is pixel-identical.
Editor script on scratch copies: `tool terrain`, right-click picks *Shore* on the sandy pond and
*Pond* on the duck pond; painting and erasing give the tile indices the masks predict, with the
joined sand re-tiled around shore water; undo restores the map. Screenshots of every new edge
block, all six maps in the editor, the Terrains section of the project and of the library's
mountains and city tilesets. In the game the player stops at the old pond's water and at the
stream, crosses the bridge, and stops at the new pond's bank. A scratch library with terrains
and terrain animations stripped and an old stamp gets them back. The map format is unchanged.
Compile-checked only: clicks in the Terrains section (egui clicks; `terrainedit` opens it for
the screenshot) and real Shift+drag erasing (`terrainpaint … erase` uses the same function).

### [x] Event / dialog graph editor — `feature/event-graph`

**Goal.** See a map's events as a graph: triggers, their conditions and actions as nodes,
dialog lines and choices readable at a glance, and the connections the data implies
(a choice's variable to the conditions that test it, *Emit* to the custom-event triggers that
listen, *Run script* / `then_call` to script functions, *Teleport* to the target map), editable
in place.

**Design.**
- New `Mode::Events` (app.rs `Mode`, `ui()` dispatch, a `ui::tab` in `menu_bar`, `mode events`
  in main.rs). Scope selector at the top: *this map* (map triggers, zones, placed entities) or
  *selected object*; a filter for script functions (*called from this map* / *all*).
- Canvas: `egui::Scene` (egui 0.31) for pan / zoom with `scene_rect` in editor state; nodes are
  `egui::Frame`s placed at computed positions; edges are `epaint::CubicBezierShape` drawn on the
  scene painter. Layout is automatic (columns: owners → triggers → script functions; rows by
  order) and recomputed on open; dragging a node only moves it for the session (nothing new is
  stored in map files).
- Node contents reuse the existing editors: make `action_fields` and the condition row of
  `conditions_ui` `pub` in `widgets.rs` and call them inside the node, so editing is identical
  to the inspector. Edits go through the same undo path as the inspector: snapshot
  `Objects::of(&map)` before, `doc.record_objects(before, "graph")` after (app.rs `inspector`).
  Definition triggers (prop / character / item `on_use`) are shown read-only with a *Open in
  Database* link.
- Script function nodes come from `script_refs::functions` + `string_args` filtered on `say`,
  `emit`, `then_call`, `choice` (text and var); extend the tokenizer pass so `choice`'s options
  array is listed (today `string_args` drops the array argument). Edges between functions from
  `used_functions`. A node links to `open_script(path, line)` (database.rs).
- Edges computed each frame from the data: `Choice.var` / `SetVar.name` → `Cond::Var { name }`
  and `SetFlag` → `Cond::Flag`; `Emit { name }` → `TriggerOn::Custom { name }` (routing as in
  `collect(.., Source::Broadcast)`: map, zones, active entities); `RunScript` / `Cutscene` /
  `Aim` / `Fly.function` / `Cond::Script` → function nodes; `Teleport { map }` → a map node
  (double-click opens that map).
- Editor script commands: `graph scope map|selection`, `graphnode N` (select), `graphshot`
  is just `shot`; `status` prints `graph=nodes,edges`.

**Decided while building.** One node per trigger rather than per action: it shows a compact
summary (conditions in one line, one row per action, choice options and their variable under
the choice) and the pencil expands it into the inspector's own `conditions_ui` / `actions_ui`
(already public, so `action_fields` stayed private); edits are recorded as a "graph" undo step
like the inspector's. Owners are the map, zones with triggers (and every portal / transition,
for its map edge) and entities with instance or definition triggers. Function nodes list `say`
lines, choices with options (`script_refs::list_args`, new), the answer variable, `emit`,
`then_call`, `set_flag` / `set_var` / `add_var`, teleports, quest and item calls; texts built at
run time show as "…". State edges also run from script writes to script reads (`flag()`,
`get_var()`), so Mira's choice reaches `mira_walk`. Focus mode follows edges forward from the
selected owner and adds the owners of reached triggers. Layout: owners with their triggers side
by side, functions and maps level with their callers (a function called by a function just
below it); same-column edges loop out to the right. The view has its own Ctrl+Z / Ctrl+Y /
Ctrl+S (the canvas only handles them in Map mode). Extra script commands `graph functions`,
`graphedit`, `graphtext`, `graphview`, `graphdump`.

**Touch points.** `editor/src/graph.rs` (new), `app.rs` (Mode, dispatch, menu tab),
`widgets.rs` (pub helpers), `main.rs` (commands), `engine/src/script_refs.rs` (choice options),
README editor section.

**Example.** Nothing new; the village (elder, Mira's choice, the shrine's `emit("shrine_lit")`)
is the showcase.

**Verify.** Done: a unit test builds the graph of a small map and script (owner, call,
event, state edges from a trigger choice and from a script choice to `get_var`, function to
function, teleport, unused functions only with *all*, focus mode, layout). Editor script on a
scratch copy of the example: `open village; mode events` gives `graph=29,21`; `graphdump` lists
the elder's choice, Mira's `mira_talk()` with *Sure* / *No thanks* and its edges to `mira_walk`
(call and variable) and `quest_accepted` between the elder's triggers; `graphtext` changed the
elder's first line, `undo` restored it; `select named elder` + `graph scope selection` gives 4
nodes; in the forest `shrine_awakening()`'s `emit shrine_lit` reaches the map trigger.
Screenshots of the village graph, Mira's function node, the elder's trigger in edit mode and the
forest's shrine functions with the slimes' definition triggers. Compile-checked only: mouse
dragging of nodes and the *Show on map*, *Open in Database*, *Edit in …* and *Open* buttons
(egui clicks).

### [ ] Imports: Tiled, LPC, Aseprite — `feature/imports`

**Goal.** Bring existing assets in without rebuilding them: a Tiled map becomes a rust2d map
with its tilesets, an LPC character sheet becomes a body sheet, an Aseprite export becomes a
sprite sheet with named animations.

**Design.**
- One *File › Import…* dialog (and `import tiled|lpc|aseprite FILE [ID]` editor commands) via a
  new `files::Purpose::Import(kind)` and a branch in `Editor::handle_file`; results are reported
  in a message box (what was imported, what was dropped). Code in `editor/src/import/` (new):
  `tiled.rs`, `lpc.rs`, `aseprite.rs`.
- **Tiled** `.tmx` (XML, add `quick-xml` as a direct dependency; it is already in the lock file
  through rfd's Wayland stack) and `.tmj` (JSON through `rhai::Engine::parse_json`, no new
  crate). Tile data: CSV, base64 (small hand-written decoder), base64 + zlib / gzip (add `flate2`
  as a direct dependency; it is already compiled for `zip`); zstd is refused with a message.
  Each tileset image is copied into `tilesets/` (`files::import_asset`) and registered as a
  `TilesetDef`; 16 px tilesets are upscaled 2× with `Rgba::resized` (tiles are always 32 px;
  `migrate.rs` does the same). Tile layers map 1:1 to `TileLayer`s (name kept; `above_entities`
  when the layer name contains "over" or has a boolean property `above`); flipped / rotated
  tiles lose their flip (counted and reported). Collision: a tileset tile property `solid`, or
  a layer named `collision` whose tiles mark `CollisionOverride::Solid`. Object layers:
  rectangles become `Trigger` zones with the object's name; points named `spawn:*` become
  spawn zones. Tiled tile animations with equal frame durations become `TileAnim`s. The map is
  saved as `maps/<name>.map` and opened.
- **LPC** (Universal LPC spritesheet, 64×64 frames, rows per animation): the importer takes the
  four walk rows (LPC order up, left, down, right), reorders them to the engine's Down, Left,
  Right, Up (`Facing` discriminants, `render/character.rs` `LAYOUT_FRAMES = 4`), picks 4 of the
  8 stepping frames, and writes a 64×64-frame sheet of the chosen `SheetKind` (Body, Hair,
  Outfit, …; the dialog asks). Frame size stays 64 (the engine allows any body frame size; the
  hitbox is set to 20×12). Other LPC animations are dropped and reported. Skin / hair recolor
  reference ramps are not applied (LPC colors stay as they are; noted in the result).
- **Aseprite**: a PNG plus the JSON exported by *File › Export Sprite Sheet* (array or hash
  `frames`, `meta.frameTags`). Frame size from the first frame; the sheet is copied to
  `sprites/`; tags become named frame ranges in a new `SheetDef.tags: Vec<SheetTag { name,
  first, last, fps }>` (`#[serde(default)]`; fps from the frames' durations), offered by the
  *Effect* / *Fly* action fields and `effect("sheet", x, y, #{ tag: "explode" })`.
- Imports never overwrite: `files::import_asset` already suffixes `_2`, `_3`; ids are made
  unique like the wizard's.

**Touch points.** `editor/src/import/` (new), `files.rs` (Purpose, filters), `app.rs`
(menu, handle_file), `main.rs` (commands), `engine/src/defs.rs` (`SheetDef.tags`, literals in
gen_example / library tests), `script.rs` (`tag` option), `widgets.rs` (tag picker),
`engine/Cargo.toml` (`flate2`), `editor/Cargo.toml` (`quick-xml`), README (new *Import*
section), CHANGELOG.

**Example.** Test fixtures only (a tiny `.tmx` with two tilesets and an object layer, a 64×64
LPC-layout sheet, an Aseprite JSON) under `crates/editor/tests/fixtures/`; the example project
stays generated.

**Verify.** Unit tests for each parser on the fixtures (layer count, tile ids, zones, dropped
flips; LPC row order; tag ranges). Editor script: `import tiled FIXTURE` then `open <name>`,
screenshot shows the map; `import lpc FIXTURE body_lpc`, `tab sheets`, screenshot of the sheet;
`import aseprite FIXTURE`, an *Effect* action offers the tag.

## 0.5.0 — Living world and adventure

Theme: the world moves without the player, and there is more to do in it: routines, weather,
companions, crafting, achievements, minigames, water and height.

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

### [ ] Achievements — `feature/achievements`

**Goal.** Creators define achievements; players unlock them by playing (quests, counters, flags,
secrets, levels), see a toast and browse them on a page of the title and pause menus. Unlocked
achievements belong to the player, not to a save game.

**Design.**
- New definitions file `defs/achievements.ron`: `AchievementDef { id, name, description, icon:
  SpriteRef, hidden: bool (shown as "???" until unlocked), conditions: Vec<Condition> }`.
  `project.rs` `DEFS` (11th entry; load / save), `release.rs` export list, `library.rs`
  `Database` literals, `refactor.rs` (conditions already renamed by `Renamer::conditions`),
  editor Database tab *Achievements* with `conditions_ui`.
- Unlocking: `Game::check_achievements()` runs every 0.5 s (and on `quest_changed`,
  `defeat_with`, `level_up`) over the locked definitions with `runner::conditions_met` (covers
  `Quest`, `Var`, `Flag`, `Defeated`, `Level`, `HasItem`, `Script`, …); plus the explicit
  `Action::UnlockAchievement { id }` (appended) and script `unlock_achievement(id)` /
  `achievement_unlocked(id)` (`IdKind::Achievement` for completion / renames). Nothing is
  checked on the title screen. Play-tests from the editor use the game's settings folder, so
  they unlock achievements like a real run; creators can set `RUST2D_SETTINGS_DIR` to keep a
  test profile apart, and the page's *Reset* row clears the file.
- Storage: `achievements.ron` next to `settings.ron` (`Settings::dir`), `{ unlocked: BTreeMap<id,
  unix seconds> }`, saved on every unlock; loaded in `Game::new`.
- Toast: a wider box with the icon, "Achievement unlocked", the name; its own list drawn after
  `draw_menu` so it shows over the pause menu; the pickup sound.
- Menu: `Row::AchievementsMenu` on the Title and Main pages (after *Options*; fix the hard-coded
  `sel` indices in `back()` and `open_title`), `Page::Achievements` with icon rows like the save
  rows (`row_h` 60): name, description or "???", unlock date; header "N of M". `status()`:
  `achievements=[ids]`.

**Touch points.** `engine/src/achievements.rs` (new: def + progress file),
`runtime/achievements.rs` (new), `project.rs`, `release.rs`, `library.rs`, `events/mod.rs`,
`script.rs`, `script_refs.rs`, `runtime/{menu,ui,mod,journal,combat,progression}.rs`,
`editor/src/{database,widgets,script_editor}.rs`, gen_example, README, scripting.md.

**Example.** Six achievements: *First Blood* (defeated ≥ 1 slime), *Pest Control* (quest
completed), *Lights in the Dark* (flag `shrine_lit`), *Seasoned* (level ≥ 5), *Naturalist*
(bestiary: `Defeated` ≥ 1 of four kinds), *Hidden Hoard* (hidden; unlocked by the mine chest's
Unlocked trigger via the action). Icons from `items.png` / `outfit_icons.png`.

**Verify.** Autoplay: defeat a slime → toast + `achievements=["first_blood"]`; the file in the
scratch settings dir lists it; restart, `press escape`, open the page, screenshot; a hidden one
shows "???"; `unlock_achievement` from a script; the editor tab with a definition.

### [ ] Minigames: lockpicking and fishing — `feature/minigames`

**Goal.** Picking a lock becomes a short skill game instead of an instant check, and water
spots can be fished for items; both are small, keyboard / gamepad playable, pausing the world.

**Design.**
- `runtime/minigame.rs` (new): `Game.minigame: Option<Minigame>` updated before `update_menu`
  like the creator (early return; `can_open` false; `modal()` true) and drawn after the HUD.
  Keys: `Act::Confirm`, `Act::Cancel`, `Act::Left/Right`; autoplay `press` works, plus
  `status()` `minigame=`.
- **Lockpicking**: three pins; per pin a marker sweeps a bar back and forth, Confirm when it is
  inside the sweet spot; width and speed come from `pick_difficulty` minus the player's
  `lockpick_level()` (skill 0 can still try the easiest locks: difficulty 1 only); three misses
  = the lock stays, a toast; success = `apply_unlock` + `GameEvent::Unlock` + `try_open_container`
  (today's `try_unlock` `picked` branch becomes "start the game"; the instant pick stays for a
  skill level ≥ difficulty + 2, so experts skip it). `LockSpec` unchanged. Also
  `Action::Lockpick { difficulty, var }` (appended) / script `lockpick(difficulty, "var")`
  for custom puzzles: the result (1 / 0) goes into the variable, the action blocks until done.
- **Fishing**: `ZoneKind::FishingSpot { rod: String }` (appended; `rod` = item needed, "" =
  none); the zone's existing `loot` table is the catch. Standing in the spot and pressing
  Confirm (nothing else to interact with) casts: a wait of 1–4 s with a bobber drawn in front
  of the player, a bite window of 0.6 s (Confirm inside it), then a tension bar (hold Confirm to
  reel, release when the bar turns red; 5 s); success rolls `roll_loot` and gives the catch
  (`item_given` toasts), failure a "It got away" toast. The editor's zone kind list gets
  *Fishing spot* with the rod field and the loot editor (`widgets::loot_ui`, already used for
  spawners).
- The player faces the water (`Facing` from the zone centre); fishing shows the rod `held`
  sheet if the rod item has one.

**Touch points.** `runtime/minigame.rs` (new), `runtime/{mod,locks,input}.rs`, `map/transition.rs`
(`ZoneKind`), `events/mod.rs`, `script.rs`, editor `widgets.rs` (zone kind UI),
`script_editor.rs`, gen_example (rod + fish items, icons, a pond spot), README, scripting.md.

**Example.** A *Fishing Rod* (Tilda, 20 coins; `held` sheet) and *Trout* / *Old Boot* items;
a fishing spot on the village pond's sand rim (x 5–15, y 24) with a loot table; the forest
chest (difficulty 1) and the mine chest (difficulty 2) use the lockpicking game.

**Verify.** Autoplay: at the forest chest with Lockpicking 1, `press e` opens the game
(`minigame=lockpick`), timed `press e` presses succeed (the marker speed is deterministic for
tests: `RUST2D_AUTOPLAY` seeds it), the chest opens; three misses leave it locked. Fishing:
stand in the spot, `press e`, wait, `press e` on the bite (status shows `bite`), hold / release,
`status` inventory shows a trout; without a rod a toast says what is needed. Screenshots of both.

### [ ] Swimming and ladders — `feature/swim-climb`

**Goal.** Water the player can swim across (slowly, no attacks) and ladder tiles the player
climbs (vertical only, slower, climbing pose), so maps gain height and water as terrain
instead of walls.

**Design.**
- `TilesetDef.water: Vec<u32>` and `TilesetDef.ladder: Vec<u32>` (`#[serde(default)]`, like
  `solid`); the Tilesets form's grid gets two more pick modes (today: solid, animation, light).
  `Loaded` gets `water` / `ladder` grids next to `solid` (`Assets::compute_solid` pattern). A
  water tile marked swimmable is **not** solid for the player only: `rect_blocked(.., swimmer:
  bool)`; NPCs, pathfinding, projectiles and sight keep treating it as today (solid if listed in
  `solid`). Creators mark a tile either solid (a wall of water) or water (swimmable).
- Swimming: when the player's feet are on water: speed × 0.5, no running, no attacking (toast
  once), no interacting with props; the paperdoll is drawn with the lower 40 % of the frame
  clipped (`draw_frame_colored_in` with a shortened source rect) and a 2 px bob, a blue tint;
  a splash `effect` on entering; `status()` `swimming=true`. Leaving water restores everything.
- Ladders: on a ladder tile only Up / Down move (× 0.6 speed), the player faces Up (climbing
  pose = the Up row with the walk cycle), interact is allowed (so a ladder top can be a portal
  zone, which is how "climbing down into the mine" works). `status()` `climbing=true`.
- Save games don't change (position is enough). Old projects: no water / ladder tiles → nothing
  changes.

**Touch points.** `defs.rs`, `render/mod.rs` (`TilesetTex` flags), `runtime/{mod,paperdoll}.rs`,
`editor/src/database.rs` (tileset grid modes) and `canvas.rs` (overlay colours), gen_example
(mark the village pond and the Wilds' lakes swimmable, keep the forest stream solid; a ladder
tile in the dungeon tileset and a ladder in the mine leading to a lower shaft via portals),
README.

**Example.** The village pond becomes swimmable (the fishing spot stays on the rim); the mine
gets a ladder down to a small lower shaft with the pick-only chest moved there.

**Verify.** Autoplay: walk into the pond, `status` `swimming=true` and the tile is water, speed
halved (tiles per second from two statuses), `press x` doesn't attack, screenshot of the
half-submerged hero; the forest stream still blocks; in the mine `hold up` on the ladder
climbs (`climbing=true`), `hold left` doesn't move; an NPC never enters water (`ai=` positions).

### [ ] Dialog portraits — `feature/portraits`

**Design.** Dialog boxes show a portrait of the speaker (paperdoll of the NPC's look, or the
player's) at the left; nothing to configure per character, one project option turns it off.
A proper node-graph dialog editor stays in the backlog.

**Verify.** Screenshot of a dialog with the elder; the player's own lines show the hero.

## 0.6.0 — Everywhere (platforms and reach)

Theme: rust2d and the games made with it run where the players and creators are: Windows,
macOS, the web, and in their language.

### [ ] Windows export — `feature/windows`

**Design.** Gate the `std::os::unix` uses (`editor/files.rs` copy permissions,
`packaging/lib.rs` symlink and executable bit, `release.rs` chmod); Windows paths for saves and
settings (`release::user_saves_dir`, `Settings::dir`: `%APPDATA%`); `.exe` handling in
`export::game_binary` and `release::export` (`<slug>-windows.zip`, README text per platform).
CI builds `rust2d-game.exe` with `x86_64-pc-windows-gnu` in the Packages and Release jobs and
the toolkit packages ship it next to the Linux runner, so the Linux editor exports Windows games
(*File → Export game* gains a *Windows* checkbox). Followed by *Editor on Windows* and *macOS
builds* below.

**Verify.** Export from the editor produces the zip; run the exe under Wine for a `status` run
if Wine is available, otherwise a CI smoke test on `windows-latest`.

### [ ] Editor on Windows — `feature/editor-windows` (follows *Windows export*)

**Goal.** The editor itself runs on Windows, with play-test, export and the asset library, so
Windows creators get the whole toolkit; the release ships a Windows zip of editor + game +
example.

**Design.**
- Gate the Unix-only calls: `editor/files.rs` `copy_tree` permissions, `packaging/lib.rs`
  `symlink` (Flatpak) and `set_executable` (AppImage) behind `#[cfg(unix)]` with Windows
  returning "not available on this platform" errors; the export dialog hides AppImage / Flatpak
  there. Paths: a `dirs`-style helper in `engine/src/paths.rs` (new, no crate): config
  `%APPDATA%\rust2d`, data `%LOCALAPPDATA%\rust2d`, cache `%LOCALAPPDATA%\rust2d\cache`, home
  `%USERPROFILE%`; used by `Settings::dir`, `release::user_saves_dir`, `library::library_dir`,
  `files::config_dir`, `example_copy`, the file dialog start folder, `new_project.rs`.
- Executables: `std::env::consts::EXE_SUFFIX` in `play_with`, `export::game_binary`,
  `library_window.rs`, `release::export` (`<slug>.exe`, `<slug>-windows.zip`, README per
  platform); `bundled_example` accepts the zip layout (`example_project/` next to the exes).
  `#![windows_subsystem = "windows"]` on both binaries, so the `--engine-build` probe writes
  its answer to a temp file instead of stdout (or the editor reads `ENGINE_BUILD` from the
  game's resources: simplest = a `--engine-build FILE` argument).
- Input: mouse-wheel units per platform in `editor/main.rs` (Windows reports 120 per notch:
  scale by 1/120; macOS pixels), `modifiers.command` instead of `ctrl` in `canvas.rs`,
  `pixel.rs`, `database.rs` (Ctrl stays Ctrl on Windows, Cmd on macOS). Window icon via
  `Conf::icon` for both binaries (the emblem from `packaging::emblem`).
- Repo hygiene: `.gitattributes` (`* text=auto eol=lf`) so `include_str!(LICENSE)` and the
  `ENGINE_BUILD` hash (which hashes path strings; make it use `/` always) are stable; tests that
  assume ELF (`packaging` `glibc_scan`, `release` `export_bundle` exe name) get `cfg` gates or
  platform branches.
- CI: a `windows-latest` job (clippy + test `--locked`; MSVC compiles the `vorbis_rs` C code for
  gen_example) on PRs into master; the Packages and Release jobs add a Windows build
  (`cargo build --release` on `windows-latest`, `cargo xtask dist --windows` zips editor, game,
  example, LICENSE, README) and upload `rust2d-<version>-windows-x86_64.zip`.
- Known gaps noted in the README: no code signing (SmartScreen warning), gamepads through
  Windows.Gaming.Input (gilrs default), file dialogs native.

**Touch points.** `engine/src/{paths (new),settings,release,library,build.rs}`, `editor/src/
{files,app,export,library_window,main,canvas,pixel,database}.rs`, `packaging/src/lib.rs`,
`game/src/main.rs`, `xtask/src/dist.rs`, `.github/workflows/*`, `.gitattributes`, README.

**Verify.** CI green on `windows-latest`; the zip from CI runs on a Windows machine or VM: the
editor opens the bundled example, play-tests it, exports a Windows game; `cargo test` on
Windows. From Linux: Wine run of the exes for a `status` smoke test if Wine is installed.

### [ ] macOS builds — `feature/macos`

**Goal.** Game and editor build and run on macOS (Apple Silicon and Intel); the release ships a
`.app` bundle for the editor (game runner and example inside) and games can be exported for
macOS from any platform's editor.

**Design.**
- Paths: `~/Library/Application Support/rust2d` (config and data), `~/Library/Caches/rust2d`
  in `paths.rs`. Bundle layout: `rust2d.app/Contents/{MacOS/rust2d-editor,
  MacOS/rust2d-game, Resources/example_project, Info.plist, rust2d.icns}`; `bundled_example`
  and `bundled_root` look in `../Resources`. Exported games: `<slug>.app` with the data in
  `Contents/Resources` (`release::bundled_root` checks there first on macOS) zipped as
  `<slug>-macos.zip`.
- Input: Cmd for shortcuts (`modifiers.command`), trackpad scrolling (pixel deltas, momentum:
  the canvas zoom accumulates deltas instead of zooming per event), `rfd` on the main thread
  (`run_on_main` needs NSApplication; the blocking dialog on a worker thread must become
  `AsyncFileDialog` polled from the frame loop, which also helps Windows).
- Rendering: miniquad's macOS backend (Metal or OpenGL per feature); keep OpenGL (default) and
  note the deprecation; high-DPI already on.
- CI: a `macos-latest` job (clippy + test) on PRs into master; Packages / Release build universal
  binaries (`aarch64-apple-darwin` + `x86_64-apple-darwin`, `lipo`), assemble the `.app` with
  `cargo xtask dist --macos` (plist generated, icon from the emblem via `iconutil` on the
  runner), zip it; unsigned (Gatekeeper: right-click › Open; documented). Notarization is out of
  scope.
- Gamepads: gilrs uses IOKit on macOS; no extra permissions.

**Touch points.** `engine/src/{paths,release}.rs`, `editor/src/{files,app,export,canvas,
main}.rs`, `xtask/src/dist.rs`, `.github/workflows/*`, README.

**Verify.** CI green on `macos-latest`; the `.app` from CI starts on a Mac (user's or a CI
smoke test with `RUST2D_AUTOPLAY` under the runner's window server), opens the example and
exports a game; `cargo test` on macOS.

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

- 47-tile corner auto-tiling (the 16-tile sets of 0.4.0 first); parallax background layers.
- Code signing and notarization of the Windows and macOS packages (today: unsigned, documented).
- Gamepad button glyphs in the HUD and hints (Steam Deck); rebinding gamepad buttons.

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
