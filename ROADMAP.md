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

## 0.5.0 — Living world and adventure

Theme: the world moves without the player, and there is more to do in it: routines, weather,
companions, crafting, achievements, minigames, water and height.

### [x] Weather — `feature/weather`

**Design.** Per-map `weather: Weather { kind: None|Rain|Snow|Fog, intensity }` in the map
RON `Meta`; `Action::SetWeather` / script `set_weather(kind, intensity)` (saved override in
`WorldState.weather`). A particle layer drawn between the light pass and `set_default_camera`
(moves with the camera), darker ambient for rain, built-in rain loop sound. The editor's
lighting preview gets a weather toggle; the map inspector a weather row.

**Decided while building.** `WorldState.weather` is per map (`BTreeMap<map, Weather>`), so rain
in the village doesn't follow the player indoors; `Action::SetWeather { map: "" = current,
weather: None = back to the map's own }`. Scripts: `set_weather(kind, [intensity])` for the
current map and `set_map_weather(map, kind, intensity)` for another one (one name per argument
layout: the script check reads a call's first argument as a map), `weather()` /
`weather_intensity()` read it (the game passes the weather with `PlayerInfo`); `"default"` goes
back to the map's weather, unknown kinds are logged. Clear weather isn't written into map files
(maps without weather stay byte-identical). Particles (`render/weather.rs`, shared with the
editor) live in world coordinates and wrap around the camera view; density per area,
intensity-scaled, capped. Fog = a grey veil plus slow soft clouds (a generated radial texture).
The weather tints the ambient light (rain darker and bluer, fog and snow a little); when the map
isn't dark otherwise (daylight, *Always bright*) the tint is applied without lights, so lanterns
don't glow by day. Particles dim with the scene but stay visible at night. The rain loop is
synthesized once at the mixer's 44.1 kHz: about 6 s of filtered noise with drop ticks, exactly 65
mixer buffers of 4096 frames long (quad-snd 0.2.8 rewinds a looped sound that ends inside a
buffer but writes the rest from the buffer's start again, leaving a short silence at every loop;
sounds start on a buffer boundary, so whole buffers never hit it), with the tail faded into the
start by an equal-power crossfade (a linear one dipped in loudness). It plays at 0.1–0.3 by
intensity times the player's new *Options › Ambience volume* (environmental sounds, default
70 %, `Settings.ambience_volume`); muted with `RUST2D_MUTE`. Music files loop through the same
quad-snd code, so their loop point can have the same short gap (see Backlog). Particles and sound pause with the world
(menus). The editor's *Weather preview* is a separate View toggle (cloud icon) next to the
lighting preview; the map settings have a *Weather* row; the undo snapshot includes it.
Example: fog (45 %) over Whisperwood, lifted by `shrine_awakening()` (`set_weather("none")`),
rain (50 %) in the Wilds.

**Verify.** Done: unit tests for the map file round trip, old saves without weather, ambient
tint, particle counts / wrapping / kind change, the script functions (incl. default and an
unknown kind) and the rain loop seam. Game (quick save, `--dev` console): the forest reports
`weather=fog:0.45(map)`; `set_weather("rain", 0.8)` → `rain:0.80(set)`; F5, `default` → fog,
F9 → rain again (kept in the save); `call shrine_awakening` lifts the fog (`weather=none`,
flag `shrine_lit`). Screenshots: forest fog by day and at night, rain by day and at night (the
guard's lantern lights the rain), snow in the village (no daylight glow), the Wilds' rain.
Editor: weather preview of the forest's fog, `mapweather snow 70` and `undo` (back to Fog 45 %),
the map settings' *Weather* row, the *Set weather* action editor. After the user's reports (rain
too loud, the loop stopping and starting, then a remaining gap): the loop's loudness is
unit-tested over two passes (no window below 80 % of the average; the old loop dipped to about
72 %); a test replays quad-snd's mixing loop (bug included): the rain loops bit-exactly, a loop
ending inside a buffer gets over 1000 silent frames; a silent run on the real device (master
volume 0) showed no ALSA underruns; the Options page shows
*Ambience volume* (screenshot) and saves `ambience_volume`; older settings files get the
default. Not checked by ear: the rain sound itself (tests run muted).

### [x] NPC schedules — `feature/schedules`

**Design.** `EntityInstance.schedule: Vec<Stop { hour, x, y }>` (same map): at each hour the NPC
walks to the stop with the patrol machinery (`patrol_step`, A*) and waits there; the editor edits
stops on the map like patrol routes. Night lines use the existing `TimeBetween` condition.

**Decided while building.** `ScheduleStop { hour, x, y, facing: Option<Facing>, inside: bool }`
(`entity.rs`, `current_stop`: the latest started stop, the last of the day carries over
midnight). `runtime/schedule.rs`: the walking part of `patrol_step` became `walk_to_tile` with a
`Walker` (path + stuck time) shared by both. At its stop the definition's behaviour applies
(Wander wanders around the stop; a patrol route is ignored while a schedule is followed).
*Inside* stops make the character inactive with a runtime `away` flag (saved as active, so the
schedule brings it out again; *Set active* clears it). Entering a map (and loading a save)
places scheduled characters at their current stop. `Action::Schedule { target, active }`
(appended) and `stop_schedule` / `follow_schedule` pause and resume it
(`EntityState.schedule_off`). Path planning for patrols and schedules now also avoids tiles
under solid props (`solid_entity_tiles`, shared with click-to-walk): the village statue trapped
Tomas. Editor: `RouteKind` makes the route click editing work for both; stop rows with a time
field (typed `7:30` or dragged in quarter hours), facing and *inside*.

**Verify (done).** Autoplay on a scratch copy with crafted saves: at 09:00 Tomas stands at the
07:00 stop; `settime 12` → `schedule=["tomas:12:00(walking)…"]`, at the duck pond after ~20 s
(screenshot); at 20:30 he walks around the statue to the elder's door and goes inside
(`(inside)`, screenshots); F5 at night, `settime 9` (he comes out), F9 → inside again; at 19:00
talking to him gives the night line (screenshot); `console stop_schedule("tomas")` →
`(stopped)`, kept through F5 / F9, `follow_schedule` → walking. Editor: `scheduleedit on`, a
`click` adds a 22:00 stop, `stop 4 hour 22.5` / `inside on` / `facing left`, `drag` moves it,
right-click removes it, `undo`; screenshots of the inspector rows and the dashed loop with time
labels. Not checked by script: typing a time into the egui field and the facing combo (the
`stop` command sets the same fields).

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
- Gapless music loops: quad-snd (macroquad's mixer) leaves a short silence where a looped sound
  ends inside a mix buffer. The rain avoids it with a length of whole buffers; music files would
  need padding to whole buffers when loaded (decode, pad, re-encode) or a fixed quad-snd.

## Shipped

See [CHANGELOG.md](CHANGELOG.md) for the details of every release.

### 0.4.0 — The workbench (2026-11-06)

- [x] Region copy, cut and paste — `feature/region-clipboard`
- [x] Script hot reload and in-game console — `feature/live-scripts`
- [x] Auto-tiling terrains (all starter tilesets, all example maps) — `feature/autotile`
- [x] Event / dialog graph editor — `feature/event-graph`
- [x] Imports: Tiled, LPC, Aseprite — `feature/imports`

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
