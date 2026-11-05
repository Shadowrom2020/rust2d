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

## 0.3.0 — Heroes and foes (progression and combat)

Theme: the player grows, enemies get interesting. All three systems are optional per project
(disabled or absent → nothing changes for existing games).

### [x] Experience and levels — `feature/progression`

**Goal.** Defeated enemies and completed quests give XP; levels raise max HP, mana and skill
points; the HUD shows the level and an XP bar.

**Design.**
- `ProjectConfig.progression: Progression` (struct-level `#[serde(default)]`, like
  `SkillSettings`): `enabled` (false), `base_xp` 100, `growth` 1.5, `max_level` 20,
  `hp_per_level` 2, `mana_per_level` 2, `skill_points_per_level` 1. XP for level n → n+1 =
  `round(base_xp · growth^(n-1))`; helpers `xp_to_next(level)`, `level_for(xp)`.
- `CharacterDef.xp: u32` (given when defeated), `QuestDef.xp: u32` (given on completion),
  `WorldState.xp: u64`, `WorldState.level: u32` (default 1).
- `Game::give_xp(n)` (new `runtime/progression.rs`) adds XP and loops level-ups: max HP +
  `hp_per_level` (healed by that amount), skill points, mana; one toast per level
  (`"Level 3! +2 HP, +1 skill point"`), pickup sound. No toast per kill (the bar is enough).
- Max HP is currently read once in `PlayerCombat::new`; replace by `Game::player_max_hp()` =
  definition + `(level-1)·hp_per_level`, used by `heal`, `hp()`, `reset_world`, `load_game`,
  the hearts and `PlayerInfo.max_hp` (`runtime/fx.rs`). `skills::max_mana()` adds the level bonus.
- HUD (`combat.rs` `draw_hud`): `Lv N` + a thin XP bar under the mana bar; the panel grows.
  Character window stats (`equipment.rs` `Stats`): `Level N, XP a / b`. `status()`: `xp=`, `level=`.
- Events: `Action::GiveXp { amount }`, `Cond::Level { op, value }` (shape of `PlayerHp`).
  Scripts: queued `give_xp(n)`, immediate `xp()`, `level()`.
- Editor: character form *Combat* grid gets *Experience*; `quest_form` gets *Experience reward*;
  `project_form` gets an *Experience and levels* section next to *Skills & magic* (with the
  total XP of the first levels as a preview); `widgets.rs` fields for the action (*Give
  experience*) and the condition (*Player level*).
- Decided while building: with progression off, `give_xp` does nothing (XP isn't counted);
  *Defeat all enemies* gives no XP (the player didn't fight); negative XP never lowers the level;
  `WorldState.level` 0 (old saves, fresh worlds) reads as 1 via `WorldState::player_level()`;
  the engine-side settings and math live in `crates/engine/src/progression.rs`.

**Touch points.** `project.rs`, `defs.rs` (`CharacterDef` literals: gen_example `main.rs`,
`art/extra.rs`, editor `builder.rs`), `quest.rs`, `state.rs`, `runtime/{combat,journal,skills,
equipment,charwin,fx,mod}.rs`, `events/mod.rs`, `script.rs`, editor `script_editor.rs` API
table, `widgets.rs`, `database.rs`.

**Example.** Enable progression (base 60, growth 1.4); slimes 5 XP, goblins 12, bandits and
skeletons 20; quests 40–80 XP.

**Verify.** Autoplay: crafted `quick.ron` next to a slime, `press f9; clickenemy …; status`
shows `xp=`; a test script function with `give_xp` reaches a level-up; screenshot of the bar and
the toast; `status` shows `level=2`, raised `hp=`, +1 skill point; save, `press f9`, same status.
Editor script: project form and character form screenshots with the new fields. A v0.2.0 copy
of the example with its save loads with defaults and no log errors.

### [x] Status effects — `feature/status-effects`

**Goal.** Timed effects on the player and NPCs (poison, burning, regeneration, slow, haste,
stun, strength, weakness, shield) from potions, weapons, enemy touch, spells and scripts;
visible as badges in the HUD and tints on sprites; saved.

**Design.**
- `defs.rs`: `enum EffectKind { Poison, Burn, Regen, Slow, Haste, Stun, Strength, Weakness,
  Shield }` (built-in kinds, appended only; label table like `SkillEffect::LABELS`),
  `struct EffectSpec { kind, strength: f32, seconds: f32 }`. `strength` means: Poison / Burn
  damage per second (fractional, accumulated; ignores defense and invulnerability, no knockback),
  Regen HP per second, Slow / Haste / Strength / Weakness percent, Shield defense points,
  Stun unused.
- `Weapon.effect: Option<EffectSpec>` (applied to whoever the weapon hits, melee or projectile),
  `CharacterDef.touch_effect: Option<EffectSpec>` (applied by contact damage). Potions use the
  *Apply effect* action in their *On use* triggers; no new item fields.
- Runtime (`runtime/effects.rs`): `ActiveEffect { kind, strength, remaining, accum }`;
  `WorldState.effects: Vec<ActiveEffect>` (player, saved), `RtEntity.effects` (NPCs, not saved,
  like their HP). Re-applying a kind refreshes the duration and keeps the stronger strength.
  `tick_effects(dt)` runs next to `update_skills` (not while frozen); player damage via a new
  `effect_damage(n)` (writes `player_hp`, game over at 0, no invulnerability / knockback); NPC
  damage via `damage_entity` with a `direct` flag (no knockback, ignores `invuln`).
- Modifiers: `equipment.rs` `speed_factor()` (Slow / Haste), `attack_weapon()` (Strength /
  Weakness), `player_defense()` (Shield); Stun: `update_player` ignores movement and attacks.
  NPCs: speed factor in `chase`, `patrol_step` and wander; AI skipped while stunned.
- Drawing: sprite tint in `draw_entity` (Poison green, Burn orange, Stun grey). HUD: a row of
  badges under the hearts, one per effect: colored box, ASCII label (`PSN BRN REG SLW HST STN
  STR WK SHD`), seconds left. Enemies: the same badges above the HP bar in `draw_combat_world`.
  `status()`: `effects=`.
- Events: `Action::ApplyEffect { target, kind, strength, seconds }`, `Action::CureEffect {
  target, kind: Option<EffectKind> }` (None = all), `Cond::HasEffect { kind }`. Scripts:
  queued `apply_effect(target, "poison", 1.0, 6)`, `cure(target, "poison")`, `cure(target)`;
  immediate `has_effect("poison")`.
- Editor: `weapon_ui` gets an *On hit* effect row, the character form an *On touch* row,
  `widgets.rs` the action / condition fields (kind combo, strength, seconds, target).

- Decided while building: the kinds and math live in `crates/engine/src/effects.rs` (used by
  definitions and the save game alike), the runtime part in `runtime/effects.rs`; the actions
  carry a whole spec (`ApplyEffect { target, effect: EffectSpec }`); a character without a
  *touch_effect* passes on the effect of a melee weapon it holds; stun also blocks interacting;
  stunned characters neither move, shoot nor hurt by contact; effects don't run while the game
  is frozen (dialogs, windows); NPC markers are colored squares above the head (no text in world
  space); the skeleton's slow is its *touch* (skeletons are starter-library characters).

**Touch points.** `defs.rs`, `state.rs`, `runtime/{effects (new),combat,equipment,mod,
npc_items}.rs`, `events/mod.rs`, `script.rs`, editor `script_editor.rs`, `widgets.rs`,
`database.rs`, gen_example.

**Example.** Slime touch = Poison 0.5/s for 6 s; Tilda sells *Antidote* (cure poison),
*Elixir of Strength* (+30 % for 60 s) and *Swiftness potion* (Haste +30 % for 30 s); the
skeleton's blade applies Slow on hit.

**Verify.** Autoplay: `call` a test function that applies poison, `status` over a few seconds
shows `hp=` falling and `effects=`; Antidote from a quick-bar `slot` cures; Stun: `hold right`
then `status` tile unchanged; screenshots of the player badges and a slowed skeleton. Save with
active effects, `press f9`, same `effects=`.

### [x] Enemy AI — `feature/enemy-ai`

**Goal.** Enemies remember the player for a moment, give up beyond a leash and return home,
flee at low HP, ranged enemies keep their distance, and bosses react to HP thresholds.

**Design.**
- `Behavior::Chase { sight, leash: f32 (0 = none, tiles from `RtEntity.home`), flee_below: f32
  (fraction of max HP, 0 = never), keep_distance: f32 (tiles; 0 = today's 0.6 × range) }`,
  new fields with `#[serde(default)]`.
- `RtEntity`: `aggro: f32` (`AGGRO_MEMORY = 4.0` s after losing sight), `returning: bool`,
  `hp_fired: Vec<u32>`. `combat.rs` `chase()` becomes a small state machine: seen (within
  sight and `line_clear`) → chase and refresh `aggro`; aggro left but not seen → walk to the
  last known position; beyond the leash → return home with A* (`patrol::find_path`), no contact
  damage while returning, then wander; `hp < flee_below · max_hp` with the player in sight →
  move away (the free direction with the largest dot product away from the player), no attacks;
  ranged with `keep_distance` → back off when closer, close in when beyond range.
- `TriggerOn::HpBelow { percent }` (appended; characters only in the editor's event lists),
  fired from `damage_entity` when HP crosses the threshold, once per life. Script immediate
  `hp_of(target)` (-1 if none).
- Editor: the Chase radio in the character form gains *Leash*, *Flee below %*, *Keep distance*;
  the exhaustive `Behavior` matches (`database.rs`, `runtime/mod.rs`) are updated.
  `status()`: the `npcs=` list adds `aggro / returning / fleeing` marks.

- Decided while building: the state is `runtime/ai.rs` `Ai` (aggro, last seen, returning,
  fleeing, way home), `chase()` moved there; noticing needs a clear line only beyond half the
  sight (dense forests otherwise hide every enemy); blocked steps slide along obstacles, scored by
  progress towards the goal; `GameEvent::HpBelow { from, to }` matches thresholds that were
  crossed, so the trigger fires again after healing (tick *once* to avoid that) and no
  `hp_fired` list is needed; `hp_of` reads a per-frame (uid, name, HP) list in the script context.
  The goblins flee below 60 % (they have 2 HP: a quarter could never be reached). Found and fixed
  on the way: actions queued while the runner ran (triggers fired by an action) were dropped.

**Touch points.** `defs.rs`, `runtime/{combat,mod,patrol}.rs`, `events/mod.rs`, `script.rs`,
editor `database.rs`, `widgets.rs` (event list), gen_example.

**Example.** Goblins `leash 10`, `flee_below 0.25`, archers `keep_distance 4`. A *Bandit chief*
near the cave (added at the end of the map function) with an `HpBelow 50` trigger: a shout and
two hidden bandits are set active.

**Verify.** Crafted saves at the goblin camp: walk out of the leash, `status` shows the goblins
returning and back at home; hurt a goblin below 25 % → fleeing mark; the bandit chief's dialog
appears exactly once; archers stay about 4 tiles away in two screenshots.

### [ ] Bestiary — `feature/bestiary`

**Goal.** A monster book in the journal (J) that fills up with every character kind defeated
at least once: portrait (paperdoll of the definition), name, description, HP, damage, XP, loot.

**Design.** `CharacterDef.description: String` (new, also shown in the editor);
`WorldState.bestiary: BTreeMap<String, u32>` (definition id → defeated count), written in
`combat::defeat`. The journal gets a second tab *Bestiary* (same tab mechanics as the character
window); the list shows unlocked entries, the right side the portrait and stats via
`paperdoll`. Script immediate `defeated_count("slime")`; `Cond::Defeated { character, op, count }`
(appended) for "kill ten slimes" quests without script. `status()`: `bestiary=`.

**Verify.** Defeat a slime, `press j`, `wintab bestiary` (new autoplay tab name), screenshot;
`status` shows the count; save / load keeps it.

## 0.4.0 — Controls and feel (player comfort)

Theme: the game feels good in the hands, with keyboard, mouse or controller.

### [ ] Input actions and gamepad — `feature/gamepad`

**Goal.** One input layer of named actions, driven by the keyboard and by a gamepad.

**Design.** `runtime/input.rs` gets `enum GameAction { Up, Down, Left, Right, Run, Confirm,
Cancel, Attack, Interact, Inventory, Skills, Spells, Map, Journal, SwitchWeapon, HideUi,
Quick1..Quick8, QuickSave, QuickLoad, Debug }` with `input::act_down(a)` / `act_pressed(a)`;
the ~78 call sites in 11 runtime files (`menu`, `mod`, `worldmap`, `inventory_ui`, `charwin`,
`creator`, `shop_ui`, `skills`, `journal`, `ui`, `combat`) switch from `KeyCode` lists to actions.
Bindings live in `Settings.keys: BTreeMap<GameAction, Vec<String>>` (serde default = today's
keys; names as in `input::key_from_name`). Gamepad through the `gilrs` crate (macroquad has no
gamepad API; the only new dependency): left stick / d-pad move, A confirm and interact, B cancel,
X attack, Y inventory, bumpers switch weapon / quick-bar page, Start pause, Select map; the loot
window gets d-pad selection (today mouse-only). Autoplay `pad BUTTON` injects buttons.

**Verify.** Every autoplay script of the README still works (key names unchanged); `pad a`
walks a dialog; screenshots of inventory / shop / loot operated by pad commands.

### [ ] Key rebinding — `feature/rebinding`

**Goal.** Options → *Controls*: a page listing every action with its keys; Enter waits for a key,
Esc keeps, *Reset to defaults*. Conflicts are shown, not refused. The README controls table
documents the defaults. Depends on the input layer.

**Verify.** `opt` commands rebind *Attack* to another key, `press` the new key attacks; the
setting survives a restart (settings file in the scratch settings dir).

### [ ] Screen shake and damage flash — `feature/juice`

**Goal.** Hits feel like hits: a short camera shake and a red flash when the player is hurt,
shake on explosions; usable from scripts for scenes; switchable off (accessibility).

**Design.** `hurt_player` sets `shake = 0.25 s`, `flash = 0.15 s`; `Game::camera()` offsets the
target by a decaying random vector; the flash is a red rectangle drawn next to the fade overlay
in `draw()`. Options row *Screen shake* (on/off). Script `shake(seconds)` / `Action::Shake`
(appended), used by `effect()` with a `shake` option. `status()`: `shake=`.

**Verify.** Two screenshots 50 ms apart after a hit differ in camera offset; flash visible;
`opt` turns shake off and the offset stays 0.

### [ ] Save thumbnails — `feature/save-thumbnails`

**Goal.** The Load page shows a small screenshot, map name, play time and date per slot.

**Design.** `write_save` also writes `{slot}.png` (screen capture scaled to 160×90 with
`imageio`, taken before the menu is drawn); `menu.rs` save rows get a 56 px layout with the
thumbnail at the left (`MAX_ROWS` adapts); deleting a slot deletes the PNG; slots without one
show a placeholder. Play time and date are already stored.

**Verify.** Save, open Load, screenshot shows the thumbnail; delete removes both files.

## 0.5.0 — Editor productivity (creator tools)

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
undo; a map painted with v0.4.0 looks unchanged.

## 0.6.0 — Living world

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

## 0.7.0 — Beyond Linux (reach)

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

See [CHANGELOG.md](CHANGELOG.md) for 0.0.1 to 0.2.0 (editor, combat, quests, shops, skills and
magic, lighting, cut-scenes, packaging, GitHub workflow). Completed roadmap items are moved here
with their version when a release is cut.
