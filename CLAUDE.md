# CLAUDE.md

Instructions for Claude Code sessions on **rust2d**, a 2D top-down RPG engine with an editor
(Rust, macroquad + egui, Linux). The shared rules for everyone — branching concept, checks
before merging, commit messages — are in AGENTS.md and apply in full:

@AGENTS.md

## Working with the user

- The user asks for features in one or two sentences and expects them **working in the
  running binaries**, not just compiling. Verify every feature end to end (scripted runs +
  screenshots, see *Verification*) and report what was tested; say plainly what wasn't.
- Pick sensible defaults instead of asking, and mention them in the summary. Ask only when a
  decision really is the user's (e.g. which of two incompatible behaviours they want).
- **Branches:** follow AGENTS.md. Before any change: `git branch --show-current` and
  `git status`. Work on `feature/<name>` created from an up-to-date `develop`; never commit to
  `master` or `develop` directly. The user also commits and opens pull requests themselves, so
  fetch first and expect branches to have moved.
- **Commit, merge, push, tag only when asked.** Finishing a release = merge `release/x.y.z`
  into `master` (`--no-ff`), tag `vX.Y.Z`, merge back into `develop` — only on request.
- The user's own data must not be changed without a backup: their projects (e.g. `~/Test`),
  released games (`~/releases`), the asset library (`~/.local/share/rust2d/library`) and
  `example_project/saves/`. Test on copies in the session scratchpad instead.
- Keep `README.md` up to date with every user-visible feature (controls table, editor
  section, scripting API), and `Documentation/scripting.md` with every script API change
  (tests fail when an engine script function is missing there or in the editor's `API` table,
  and the document's code examples must compile).

## Workspace map

```
crates/engine/src/      library: everything shared by game and editor
  defs.rs               Database: tilesets, sheets, items, props (+states, container, lock),
                        characters (+shop), sounds, quests, structures; serde formats of defs/*.ron
  skills.rs             SkillSettings (project), SkillDef / SkillEffect, SpellDef / SpellEffect
  structure.rs          StructureDef::capture / place (multi-tile assets; editor-only, placing
                        writes ordinary tiles + entities)
  project.rs            ProjectConfig (project.ron), defs loading/saving (DEFS list),
                        TILE_SIZE = 32, CharacterCreation / SexChoice
  settings.rs           player options (Options menu): ~/.config/rust2d/<game>/settings.ron,
                        RUST2D_SETTINGS_DIR overrides the folder (set it to a scratch dir in tests)
  render/character.rs   layered characters: every character is an Appearance (body sheet, skin,
                        hair, beard; monsters are bodies without hair); layers() in drawing
                        order (held behind, body, clothing, body armor, hair, beard, head armor,
                        held in front; outfits have a female cut, Armor::overlay_f, worn by
                        female body sheets); render/recolor.rs = skin (magenta) / hair (cyan)
                        reference ramps recolored to the chosen colors. SheetDef.playable = body offered
                        by the creator (Database::playable_bodies). Old projects: a
                        character's legacy `sheet` becomes its body (Database::upgrade_characters)
  fog.rs                fog of war: Explored (tiles per map, saved as a compact string), REVEAL_RADIUS
  state.rs              WorldState = save game (flags, vars, inventory, quests, shops,
                        player name/sex/look), Sex, expand_text() placeholders
  map/                  Map, tile layers, zones (spawn, portal, trigger, spawner, transition),
                        io.rs = map file format (postcard tiles + RON `Meta`)
  events/               GameEvent, TriggerOn, Cond, Action (mod.rs); ActionRunner (runner.rs)
  script.rs             Rhai engine and the script API
  script_refs.rs        ids inside scripts: string arguments of API calls (API_IDS), used for
                        renames (rename_in_script) and the editor's script check
  runtime/              Game: mod.rs (loop, actions, drawing), charwin (the tabbed character window:
                        Character = inventory_ui, Skills / Spells = skills; C/I, K, L, Tab), cutscene (Play
                        cutscene: HUD off, bars, walk_to / approach / place), combat, equipment, locks,
                        inventory_ui, shop_ui, journal, creator, patrol, lights (light sources,
                        clock), skills (skill bonuses, mana, spells, aiming, K screen), fx (Fly /
                        Effect actions: scripted flying sprites and one-shot animations), menu (title/pause/options),
                        worldmap (M map screen), paperdoll (drawing layered characters, player model),
                        npc_items (NPC equipment / carried items: combat, drops, saved changes), mouse,
                        spawner, ui (dialogs, toasts), input (+ injection for autoplay)
  quest.rs, inventory.rs, entity.rs, path.rs (A*), render/ (Assets, chunked tile renderer;
                        lighting.rs = day/night ambient, LightPass light map, tile_lights)
  library.rs            asset library + .r2dpack bundles; refactor.rs = id renames / refs_of_*
  migrate.rs            converts old 16 px projects; release.rs = standalone game export
  examples/gen_example/ generates example_project/ and the editor's starter.r2dpack
crates/game/src/main.rs game binary: CLI args, window, RUST2D_AUTOPLAY script runner
crates/editor/src/      editor binary
  main.rs               loop + RUST2D_EDITOR_SCRIPT command runner
  app.rs                Editor state, menus, inspector, Pending/close-project logic, play-test
  canvas.rs (map view + tools), doc.rs (map doc + undo; Edit::Group), structures.rs (Structures
  tool, capture, thumbnails), database.rs (Database tabs),
  widgets.rs (sprite pickers, trigger/condition/action editors, shop/container/lock UIs),
  maps.rs (map switching, transitions), builder.rs (character builder: cards + preview, the
  New character / Customize window, `char…` script commands), models.rs (template chooser:
  copy a project / library character's look),
  script_editor.rs (Rhai editor: highlighting, completion, API table with params/docs),
  ui.rs (icon font, ribbon / tool-button helpers),
  new_project.rs (wizard), library_ui.rs (library Browse view, "Add from library" dialog,
  thumbnail grid), library_window.rs (`--library`: the editor in library mode on the asset
  library, opened as its own process), pixel.rs (pixel editor), wizard.rs (new asset),
  export.rs (release export dialog), files.rs (file dialogs, create_project, recent list)
example_project/        generated by gen_example — edit the generator, not these files
```

## Coding guide

- Match the surrounding style: long lines (~160 chars) are normal here, `///` doc comments on
  items, short comments that explain *why*. Rust 2021; clippy must pass with `-D warnings`.
- Errors: `anyhow::Result` with `.context(...)` in IO code; the game never panics on bad
  data — log with `eprintln!` (prefix like `[game]`, `[events]`) and carry on.
- Don't add dependencies without a good reason. Pinned stack: macroquad 0.4.16 (with
  `features = ["audio"]`), egui-macroquad 0.17.3 (egui 0.31), postcard + lz4_flex, ron, rhai,
  rfd, zip 9, png 0.17.
- **Compatibility is a hard rule** (maps, defs and save games of older versions must load):
  - New variants of `Action`, `TriggerOn`, `Cond`, `ZoneKind`, `OnUnlock` are **appended**
    at the end (map files store variant indices).
  - New fields get `#[serde(default)]` (or `default = "fn"`). New map object fields go in the
    RON `Meta` in `map/io.rs`; never edit the frozen `EntityV1` there.
  - Save games are `WorldState` RON: every new field needs a serde default.
- Tiles are always 32 px; sizes in code are in pixels at that size (`Game::u()` = ts/16 scales
  effects). Rhai reserves `var`, hence `get_var` / `set_var`.
- Game UI (macroquad's default font) is ASCII only: no `∞`, `▲`, `✔` in `draw_text`. The editor
  uses the Phosphor icon font (`crates/editor/src/ui.rs`: `ph::NAME` constants, `ui::t(icon,
  text)`, `tool_button` / `icon_button` / `toggle` with name + shortcut + how-to tooltips,
  `group` for ribbon groups, `kind_icon`). Don't put emoji or `→` in egui strings — most render
  as boxes; use a `ph::` icon (or `ph::CARET_RIGHT` for arrows in prose).
- egui closures: take what you need out of `self` first (or use thread_local request hooks as
  in `maps.rs`, `models.rs`, `widgets::request_chooser`) to avoid borrow conflicts.

### Where a new feature usually has to touch

- **New `Action` / `Cond` variant:** `events/mod.rs` (variant at the end, `label()`,
  `templates()`), execution in `runtime/mod.rs` (`execute`) or `Condition::eval` /
  `runner::conditions_met`, editor fields in `widgets.rs` (action fields / `conditions_ui`),
  `refactor.rs` if it references ids, and a Rhai function in `script.rs` if useful.
- **New script function with an id argument** (item, map, sound, quest, …): add it to
  `script_refs::API_IDS` so renames and the script check cover it, and to the `API` table in
  `editor/src/script_editor.rs` (parameters, argument kinds, description) for completion. Note `go` and a few
  other words are reserved in Rhai.
- **New definition fields:** update every struct literal (`gen_example/main.rs`,
  `gen_example/art/extra.rs`, `editor/database.rs`, `editor/wizard.rs`, tests in
  `library.rs` / `refactor.rs`), the editor form in `database.rs`, renames in `refactor.rs`,
  and dependencies in `refs_of_*` so library export carries what it needs.
- **New defs file:** `project.rs` (`DEFS`, load, save), `release.rs` (exported files),
  `library.rs` (`Database` literals).
- **Player-facing text:** run it through `WorldState::expand_text` (`{player}`,
  `{sir|madam}`, `{var}`) like dialogs, choices and quest texts.
- **Example content:** change `crates/engine/examples/gen_example/` and regenerate
  (`cargo run --example gen_example`); keep entity uids of existing maps stable by adding new
  entities at the end of a map function (saves reference uids).
- **Testing hooks:** add autoplay / editor-script commands and extend `Game::status()` so the
  feature can be verified from a script.

## Verification

```sh
cargo build --workspace            # ALWAYS after engine changes: the editor's play-test and
                                   # scripted game runs use target/debug/rust2d-game
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release --workspace  # keep release binaries current too (export, user runs)
```

Use the session scratchpad for everything temporary: copy `example_project` (minus `saves/`)
there and test on the copy. Set `RUST2D_MUTE=1` and `RUST2D_SETTINGS_DIR=<scratch>/settings` (the game's Options menu saves there), and for the editor
`RUST2D_EDITOR_CONFIG=<scratch>/cfg RUST2D_LIBRARY=<scratch>/lib`.

- **Game:** `RUST2D_AUTOPLAY="wait 1; press e; hold right 0.5; rclick 6 3; type Name; shop buy 0; close; status; shot /path/a.png; quit" target/debug/rust2d-game <project> [--no-title] [--map M --spawn S | --pos X,Y]` (`--pos` is in pixels: tile × 32 + 16).
  Commands: `wait`, `press` (`press shift+g` holds Shift), `hold`, `click`/`rclick X Y` (tiles), `clickenemy`, `slot`,
  `slotdrag` (`slotdrag player 1 out` drags a stack out of the window), `shop buy|sell N`, `type`, `close` (window close button), `settime H`, `skillpoint|learn|cast|call ID` (call runs a script function), `opt ROW prev|next|row` (Options page arrows), `hud window|map` (the character window button / map), `wintab character|skills|spells` (click a tab of the open character window; keys `c`, `i`, `k`, `l`, `tab` too), `press h` (hide / show the HUD; status prints `ui_hidden=`; `cutscene=` the playing cut-scene; start with `--cutscene FN` to preview one without the map's Map enter triggers), `mapbtn close|zoomin|zoomout|center`, `drag X1 Y1 X2 Y2` / `wheel DY` (screen pixels), `status`,
  `spawners`, `shot`, `quit`. `status` prints player, creator, quests, shop, map, tile, hp,
  vars, inventory, flags. Load a crafted save with `press f9` from `<project>/saves/quick.ron`
  (a minimal `WorldState` RON) to start in a specific situation; loading never fires *Map enter*
  triggers, so a crafted save starts quietly.
- **Editor:** `RUST2D_EDITOR_SCRIPT="wait 1; open village; select named tilda; mode db; tab quests 1; shot /path/b.png; quit" target/debug/rust2d-editor <project>`.
  Many commands exist (`grep '=> {' crates/editor/src/main.rs`): tools, placing, zones,
  transitions (`linkarea`, `trdraw`…), library (`libadd`, `tolib`, `libdialog KIND`, `libpick`,
  `libdialogadd`, `libwindow`, `libedit`…; scripted assets: `tolib` / `projexport KIND ID PATH` open the *Script functions* dialog → `scriptpick FN on|off`, `scriptpickok`, `scriptpickcancel`; `libimport` / `projimport PATH` import packages, `libfilter KIND`), structures (`structure ID`, `stplace X Y`,
  `stcapture X Y W H ID`, `stinfo`; structure editor: `stnew`, `stsize W H`, `stlayer N`,
  `sttool stamp|keep|clear|solid|walkable|auto|prop`, `ststamp TS X Y W H`, `stprop ID`,
  `stpaint X Y`), `hover X Y` (tool previews), `lighting on|off [HOUR]` (lighting preview), `mapfog on|off` (map's fog of war), `addaction LABEL` (append an action template to the selected entity's trigger), `entequip ITEM` / `entskin RRGGBB` (selected NPC's own equipment / look), pixel editor (`px…`),
  character builder (`charnew [ID NAME]`, `charcustom` = selected NPC, `charset body|hair|beard|skin|haircolor|clothing|head|armor|weapon|offhand|carry|id|name VALUE` on the open builder else the selected character, `charrandom`, `chartemplate ID` (project or library character), `charcreate` / `charapply`, `charcancel`, `charreset`, `playable SHEET on|off`; `status` prints `builder=`),
  template chooser (`model def|inst|builder`, `modelpick`, `modeluse`), wizard (`newwizard`, `wiz`,
  `wizcreate`), `scriptsel`, `probe X Y` (egui layer at a point), `status`, `dump`.
- **Asset Library window:** run `rust2d-editor --library` (with `RUST2D_LIBRARY=<scratch>/lib`)
  and a script directly; `libwindow` from the main editor passes `RUST2D_LIBRARY_SCRIPT` to the
  child as its script.
- The editor script cannot click egui widgets; script commands call the same functions
  instead. For **real egui clicks/typing**, build a throwaway copy of the workspace in the
  scratchpad with a patched egui-macroquad (`[patch.crates-io]`) that exposes
  `inject_mouse` / `inject_text`, plus `eclick X Y` / `etype` commands — never patch the real
  repository. Add `ekey NAME [ctrl]` (egui key events) the same way for keyboard tests; after
  a Ctrl combination send a Ctrl key-up, or egui keeps ignoring typed text. There is no xdotool / Xvfb on this machine, and XTest on XWayland may prompt the
  user, so don't use it.
- Look at screenshots with the Read tool; crop with PIL when details matter. Lighting:
  `RUST2D_LIGHTMAP_DEBUG=1` draws the raw light map instead of multiplying it.
- Dialog text types out over time: wait ~2.5 s before reading a screenshot, and the first `E`
  only completes the text, the second closes it.

## Pitfalls seen before

- A game binary older than the engine can't read new map data (`Unexpected variant …`);
  the editor warns when `rust2d-game --engine-build` differs. Rebuild the workspace.
- `example_project/saves/` may contain the user's saves — never delete, regenerate over or
  commit it (it is git-ignored).
- The starter pack merges library-only extras by id; the example project's version wins.
  Existing user libraries only gain *new* ids (`Bundle::add_missing`), except that an existing
  image the new pack only extends (more frames, old pixels unchanged) is replaced. The starter
  is applied when `starter.stamp` (rules version + pack hash) changes: bump `STARTER_RULES` in
  `library.rs` when the way starter sets are applied changes, so existing libraries rerun it.
- macroquad `Image::export_png` flips vertically (the generator pre-flips).
- Generated characters are drawn by `art.rs::draw_part` (Full = whole figure, used for skeleton
  bodies; Body / Hair / Beard / Outfit = layers): bodies use `SKIN_REF`, hair `HAIR_REF`; overlays
  get an outline only outside the body (`outline_outside`). `Look.female` gives the feminine
  body shape and the fitted outfit cut (`outfit_cuts` writes `outfit_X` and `outfit_X_f`);
  `HairStyle` / `BeardStyle` pick the hair / beard sheets. Held items use the walk animation's
  hand positions (`hand()`). Sheet ids: `body_male` / `body_female` / `body_goblin` /
  `body_goblin_female`, `hair_short|long|ponytail`, `beard_full|goatee`.
- Interaction: E checks the tile in front of the player (also across a counter / solid
  tile for NPCs); right-click walks there first. Place test players accordingly.
