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

### [x] Companions — `feature/companions`

**Design.** `Behavior::Follow` for placed NPCs plus script `follow("name")` /
`stop_following("name")` and the matching actions; followers path to the tile behind the player,
are carried across maps (`WorldState.companions`: definition id, HP, gear), attack hostiles in
reach with their weapon (reuse `attack_at` with an attacker parameter) and are excluded from the
player's melee targeting (`attack_at` filters on `hostile`, which also stops hitting friendly
NPCs today). Enemies target the nearest of player and companions.

**Decided while building.** No `Behavior::Follow`: joining is the **Companion** action
(`Action::Companion { target, join }`, appended) and `follow` / `stop_following`, plus
`is_companion(name)` / `companion_count()`. `WorldState.companions: Vec<Companion { map, uid,
name, hp }>` points at where the character is placed: its look, items (`npc_items`, keyed by the
home map / uid) and triggers come from there, and the placed instance isn't built while it is
recruited. `runtime/companions.rs`: runtime uids `COMPANION_UID_BASE + k`, a `Link` on the
`RtEntity` (home, triggers, walker); companions aren't solid (the player walks through them),
follow to a tile behind the player (`walk_to_tile`, at the player's speed, faster when behind),
catch up beyond 14 tiles, fight the nearest hostile within 5 tiles of them and of the player
(own melee / ranged code rather than `attack_at`, which is the player's swing, ammo and
cooldown), regain 1 HP per 4 s when no enemy hunts. Enemies (`chase`, shooting, contact,
projectiles) go for the nearest of player and hurtable companions (max HP > 0). Knocked out
(`defeat` of a companion) = dismissed and gone home with full HP, no XP or loot. Player attacks
filter on `hostile` as planned (melee, projectiles, `damage_at` spells, mouse attack / aim).

**Verify (done).** Autoplay on a scratch copy: `call guard_talk` with the quest flag, choosing
"Come with me" → `companions=["guard:8/8@…"]`; walking left, he follows (screenshot);
`teleport` to the forest → he arrives beside the player; near the slime den he kills a slime
(`bestiary=["slimex1"]`, screenshot); poisoned to 5 HP, F5, back to the village (HP carried),
F9 → in the forest with 5/8; poisoned to 0 → `companions=[]`, no bestiary entry, and back in the
village he patrols again from his post; `call guard_talk` as companion, "Yes, thank you" →
dismissed on his own map, patrol resumes. Not checked by script: a companion with a ranged
weapon (the example has none), the editor's Companion action fields (compile-checked).

### [x] Crafting — `feature/crafting`

**Design.** New definitions file `recipes.ron` (`RecipeDef { id, result, count, ingredients:
Vec<ItemStack>, station }`; `project.rs` DEFS, `release.rs`, `library.rs`), `Action::OpenCrafting
{ station }` / script `open_crafting("anvil")`, a crafting window modelled on `shop_ui.rs` (rows
= recipes, greyed when ingredients are missing). Editor: Database tab *Recipes*; `refactor.rs`
renames and `refs_of_*` for library export.

**Decided while building.** `crafting.rs`: `RecipeDef` also has `skill` / `level` (like spells),
`missing()` (items short or skill too low), `craft()`, `fits()` (the result must fit once the
ingredients are gone). `Action::OpenCrafting { station, title }` (appended; "" title =
"Crafting"), `open_crafting(station[, title])`, `can_craft(recipe)` (the script host gets the
recipes via `set_recipes`; `IdKind::Recipe` in `API_IDS`). Recipes are a full asset kind
(`RefKind::Recipe`, appended): library export carries the result / ingredient items and the
skill, the library's Browse view shows them with the result's icon. `runtime/crafting_ui.rs`:
one list (icon, name x count, ingredient icons with have / need, *Make* button), Shift makes up
to ten, the footer shows the result's description and the last message; it shares the trade
window's slot (`shop_open()` covers both). Editor: *Recipes* tab (result + count with icon,
ingredients list, station with suggestions, skill + level), *Open crafting* action fields, API
table entries with recipe / station completion. Example art: Healing Herb and Red Cap icons
(appended to `items.png`), the alchemy table on a new row of `props.png` (both only grow, so
existing libraries take the new images).

**Verify (done).** Autoplay on a scratch copy: with 3 herbs and 1 mushroom, right-click the
alchemy table in the shop → `crafting=Some("alchemy [brew_potion brew_antidote brew_mana(missing)
brew_swiftness(missing)] …")` (screenshot); `craft 0`, `craft 1` → inventory potion 1, antidote
1, herbs and mushroom used up (screenshot); `craft 0` again → "Missing: Healing Herb 0/2";
`craft 3` → "Needs Arcana 1"; Esc closes (`crafting=None`). Editor: `tab recipes brew_antidote`
(screenshot of the form); a fresh asset library gets the four recipes from the starter pack
(`libfilter recipe`, screenshot); `forest` has the herbs and mushrooms (`dump`). Unit tests:
crafting, RON defaults, the script API, library export and item renames into recipes. Not
checked by script: typing into the Recipes form (compile-checked) and Shift-crafting ten.

### [x] Achievements — `feature/achievements`

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

**Decided while building.** Achievements aren't library assets (they belong to a game): the
bundle `Database` literals leave them out. No hooks in `quest_changed` / `defeat_with` /
`level_up`: the half-second check covers them. `Progress` (`engine/src/achievements.rs`) has
`load` / `save` / `reset` (the menu's *Reset* asks first: `Page::ConfirmResetAchievements`); the
script host gets the unlocked ids (`set_achievements`). Ids are edited like quest ids (a text
field, not renamed through the project). The menu rows that used hard-coded indices
(`back()`) now select by row (`select`). The Achievements row only appears when the game has
achievements; long descriptions are cut with "..." (`fit`).

**Touch points.** `engine/src/achievements.rs` (new: def + progress file),
`runtime/achievements.rs` (new), `project.rs`, `release.rs`, `library.rs`, `events/mod.rs`,
`script.rs`, `script_refs.rs`, `runtime/{menu,ui,mod,journal,combat,progression}.rs`,
`editor/src/{database,widgets,script_editor}.rs`, gen_example, README, scripting.md.

**Example.** Six achievements: *First Blood* (defeated ≥ 1 slime), *Pest Control* (quest
completed), *Lights in the Dark* (flag `shrine_lit`), *Seasoned* (level ≥ 5), *Naturalist*
(bestiary: `Defeated` ≥ 1 of four kinds), *Hidden Hoard* (hidden; unlocked by the mine chest's
Unlocked trigger via the action). Icons from `items.png` / `outfit_icons.png`.

**Verify (done).** Autoplay on a scratch copy: `defeat_enemies()` in the forest → toast (screenshot)
and `achievements=["first_blood"]`, `achievements.ron` in the scratch settings dir lists it;
`unlock_achievement("hidden_hoard")` from the console → unlocked (and written); restarted with
the title screen, the Achievements page shows First Blood unlocked and the hidden one as "???"
(screenshot); the pause menu's page with a toast drawn over it (screenshot); *Reset
achievements* → confirm → `achievements=[]`, the file is gone, Esc back to the pause menu. The
editor's Achievements tab (screenshot). Not checked by script: the mine chest's Unlock trigger
(needs Lockpicking 2) and the Unlock achievement action's editor fields (compile-checked).

### [x] Minigames: lockpicking and fishing — `feature/minigames`

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

**Decided while building.** Who plays: `pick_difficulty > 0`, skills on, no key at hand,
Lockpick level ≥ max(1, difficulty − 1) (`min_pick_level`: no picking without the skill, the user's call; the roadmap's "skill 0 tries difficulty 1" was dropped); level ≥ difficulty + 2 picks at once (games without the skill
system can't pick, as before; a player who has the key uses it). The level gap −1…3 sets the
spot's half width (0.17 … 0.05 of the bar) and the marker speed (0.55 … 1.95 bars/s). The
minigame runs right after the creator in `update` (early return, so Esc gives up instead of
opening the menu); `blocked()` waits for it (the *Lockpick* action). Winning a lock fires
Unlock, Interact and opens the container. Fishing: a `FishingSpot` zone covers the water (not the shore): Interact casts when one of
the three tiles straight ahead belongs to it (`fishing_target`; facing grass does nothing), the
bobber lands two tiles out or on the farthest spot tile closer than that; the player keeps
their facing; fish shadows (`render::fish::draw_shadows`, a pure function of the time: 1–4
fish per spot, each showing for part of a 7–11 s cycle at a new place) mark spots in the game
and the editor;
the bobber lands 2 tiles ahead. A new **Tool** equipment slot (`EquipSlot::Tool`, appended;
`ItemDef.tool` = the tool kind, `Equipment.tool`) holds the rod: a fishing spot's item is taken
from the Tool slot (moved there from the bag if needed), and while fishing the player is drawn
with the tool in the right hand instead of the weapon (`player_equipment_drawn`), the line
leaving the rod's tip (`rod_tip`: fixed offsets of the idle frame in the 32 x 48 layout that
the example's `held_rod` follows); the catch is one weighted entry of the zone's loot table
(`pick_catch`). Timings: bite after 1–4 s, 0.6 s to strike, reeling 0.3 / s while the tension
stays under 0.9, the fish pulls in waves, the line snaps at full tension, 10 s to land it. A
fixed seed under `RUST2D_AUTOPLAY` keeps the spots and waits the same in tests, and the
autoplay `minigame hit|miss|bite` command presses at the right (or wrong) moment.

**Touch points.** `runtime/minigame.rs` (new), `runtime/{mod,locks,input}.rs`, `map/transition.rs`
(`ZoneKind`), `events/mod.rs`, `script.rs`, editor `widgets.rs` (zone kind UI),
`script_editor.rs`, gen_example (rod + fish items, icons, a pond spot), README, scripting.md.

**Example.** A *Fishing Rod* (Tilda, 20 coins; `held` sheet) and *Trout* / *Old Boot* items;
a fishing spot on the village pond's sand rim (x 5–15, y 24) with a loot table; the forest
chest (difficulty 1) and the mine chest (difficulty 2) use the lockpicking game.

**Verify (done).** Autoplay on a scratch copy with crafted saves: at the forest chest with
Lockpicking 1, `press e` → `minigame=lockpick:pin=0/3…` (screenshot), `minigame hit` ×3 (one
`miss` in between) → done(true), the chest opens with its loot window (screenshot); three
`minigame miss` → done(false), the lock holds; with Lockpicking 0 the chest only shows its
locked text (screenshot). At the pond's rim facing up or left (grass): nothing
happens; facing the water without a rod: "You need a Fishing Rod to fish here."; with one,
facing down: `fishing:wait` (bobber screenshot), `minigame bite` →
`reel`, holding E in bursts lands it: `inv` gains a trout; the rod in the bag moves into the
Tool slot ("Equipped Fishing Rod", `equip=[…, "fishing_rod"]`) and is drawn in the hand in all
four facings with the line from its tip (screenshots), the character sheet shows the Tool slot; the bobber lands on the water
(screenshot); fish shadows in six screenshots a second apart and in the editor; holding E for 3 s snaps the line (no
catch). `lockpick(2, "vault")` from the console → three hits → `vault=1`; `lockpick(1, "door")`
and Esc → `door=0`, no pause menu. Unit test: weighted catches. Not checked by script: the mine
chest (difficulty 2), the editor's Fishing spot fields and catch table, the Lockpicking game
action fields (compile-checked).

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
- **Schedules across maps and with furniture** (improves *NPC schedules* of 0.5.0): a scheduled
  character can go to another map (into its house, the shop, the tavern) and use furniture
  there: sleep in a bed, sit on a chair or bench, stand at a counter. Ideas so far:
  - *Stops on other maps*: `ScheduleStop.map: String` (`#[serde(default)]`, "" = the
    character's own map). To get there the character walks to a portal / transition zone of
    its map leading to that map (the nearest one, found by `target_map`), then leaves: on its
    home map it is `away` (as *inside* stops today). Where everyone is lives in the save game,
    e.g. `WorldState.npc_whereabouts: BTreeMap<String /* home map */, BTreeMap<u32 /* uid */,
    String /* map */>>` (serde default), so the other map knows its visitors.
  - *Visitors*: building a map's entities adds the characters whose current stop is on this map
    (their instance comes from the home map: load it through the map cache, or keep a copy of
    the instance in the whereabouts entry). They get a runtime uid range of their own like
    dropped items (`DROPPED_UID_BASE`), their triggers, look and items come along, and state
    changes are saved under the home map's uid. Arriving while the player is there: they appear
    at the portal's arrival spawn and walk to the stop; leaving: they walk to the portal of their
    next stop and disappear. Maps the player isn't on aren't simulated (as today: placed at the
    stop of the hour on entering).
  - *Furniture*: `PropDef.use_spot: Option<UseSpot { offset: (f32, f32), facing, pose: Sit |
    Lie | Stand }>` (serde default; editor: Database › Props, with a preview of the spot), and a
    stop that names a prop instead of a tile (`ScheduleStop.prop: String`, an entity name on
    the stop's map; the tile is then the spot in front of it). Arrived, the character snaps to
    the spot and takes the pose: *Sit* draws the paperdoll a few pixels lower with the legs
    clipped (like the swimming item's clipped frame), *Lie* draws it rotated 90° on the bed with
    the bed's optional "occupied" state on top (a blanket overlay, so the prop decides what
    covers the sleeper), *Stand* only faces the furniture (counters, anvils, shelves). The prop
    isn't solid for its user while in use; a used prop is skipped by others (first come).
  - *Interaction*: talking to a sleeping character runs its triggers as usual, with a new
    condition `Cond::Activity { target, pose }` (appended; "is asleep") for "Zzz..." lines, or
    a default "is asleep" toast when no trigger matches; waking it is up to the creator
    (`stop_schedule`, a dialog). `status` lists `pose=` per scheduled character.
  - *Example*: Tomas sleeps in a bed in the elder's house instead of vanishing at the door;
    Tilda sits behind her counter by day and goes upstairs (or home) at night; a bench on the
    village green for the afternoon. Needs a bed (with an "occupied" state), a chair and a
    bench prop in the generator.
  - *Verify*: autoplay at 20:30 follows Tomas through the door (`status` on the house map
    shows him walking in, then `pose=lie`), save / load at night keeps him in bed, entering the
    house at noon shows nobody; screenshots of sitting and sleeping.
- **Healing companions** (improves *Companions* of 0.5.0): spells, scrolls and potions can heal
  a companion, not only the player.
  - *Target choice*: a *Heal* spell (`SpellEffect::Heal`) and consumable items whose *on use*
    heals (`Action::Heal`) ask whom to heal while companions are around: the aiming mode of
    bolts (`update_aim`) with friendly targets: click / select the player or a companion
    (keyboard: Left / Right cycles, Confirm heals, Cancel aborts; with no companion on the map
    it heals the player at once, as today). Items get `ItemDef.use_on: UseTarget { Player, Ally,
    Choose }` (serde default `Player`) so a potion can be drunk by the player only or offered to
    a companion; scrolls are consumable items with a spell-like use (`Action::Heal` + `Choose`).
  - *Actions and scripts*: `Action::Heal` gains `target: TargetName` (serde default "player";
    old maps keep healing the player), so `heal_npc("guard", 4)` and *Heal* actions on a named
    character work; `heal_area` already heals friendly characters (companions included) and
    stays. Companion HP is written back with `sync_companion_hp`.
  - *HUD*: small companion portraits with HP bars under the player's (head of the paperdoll,
    like the save thumbnails' heads), so it is clear who needs healing; clicking one targets it
    while healing.
  - *Verify*: a potion used with the guard at 3/8 offers both targets, choosing him → 7/8, the
    player's HP unchanged; the Heal spell the same; screenshot of the HUD portraits.
- **Knocked-down companions stay down until healed** (changes the knock-out rule of
  *Companions*; builds on *Healing companions*): at 0 HP a companion falls (drawn lying, the
  *Lie* pose of the furniture idea or a 90° rotation, greyed), stays where it fell, isn't hunted
  or hit, and doesn't follow. Healing it (potion, spell, scroll, `heal_npc`) gets it up with the
  healed HP; a project setting decides what happens otherwise: *stays down* (default), *gets up
  after N seconds with 1 HP*, or *goes home* (today's rule). Leaving the map with a downed
  companion asks "Leave the guard behind?" (yes = it goes home, as dismissed); a save keeps it
  down (`Companion.down: bool`, serde default). When the player is defeated, downed companions
  don't matter (the defeat menu as today). `status` prints `down` per companion.
  - *Verify*: poison the guard to 0 → `down`, enemies ignore him, he stays when the player walks
    away; a potion on him → up with its HP and following; save / load while down.
- **Commands for companions** (improves *Companions*): the player tells companions how to
  behave.
  - *Orders*: *Attack* (today's behaviour: fight hostiles near the player), *Defend* (only fight
    what attacks the player or the companion, stay close), *Passive* (never fight, keep out of
    reach of enemies: flee behind the player), *Stay* (wait here, fight what comes close; *Follow*
    ends it). Stored per companion (`Companion.order: Order`, serde default `Attack`), saved.
  - *Giving them*: a small command wheel / menu on a key (e.g. `G`, pad `RB`; `Act::Command`),
    with the companion to address when there are several (or *All*); talking to a companion can
    offer the same choices in its dialog via script: `order("guard", "stay")`,
    `order_of("guard")`, and `Action::CompanionOrder { target, order }` (appended).
  - *Behaviour*: `companion_fight` filters targets by the order (Defend: hostiles hunting the
    player or this companion, `ai.aggro` and its target; Passive: none, and `step_towards_dir`
    away from the nearest hostile within 3 tiles), *Stay* skips following and keeps a `post`
    position (not carried to other maps: leaving the map with a staying companion asks like a
    downed one, or it follows automatically: a project setting). HUD portraits show the order
    as an icon (ASCII letter in game UI: A / D / P / S).
  - *Verify*: autoplay `press g` + choices → `status` `companions=["guard:8/8:stay@…"]`; with
    *Passive* the guard doesn't attack a slime next to him; *Defend* ignores a slime that hasn't
    noticed the player; *Stay* keeps him put while the player walks off; save / load keeps the
    orders.
- **Companion inventory** (improves *Companions*; pairs well with *Healing companions*): the
  player equips companions with gear and hands them items they use on their own.
  - *Groundwork that exists*: NPCs already wear equipment and carry items (`RtEntity.equipment`
    / `inventory`, `npc_equip` / `npc_unequip` / `give_item_to`, saved per placed character in
    `WorldState.npc_items`, which companions already use with their home map / uid), and their
    weapon and defense follow their gear (`npc_items::apply_gear`).
  - *Window*: a *Companion* tab in the character window (Tab cycles; one tab per companion, or a
    companion picker in the tab) showing the companion's paperdoll with its equipment slots
    and its carried items next to the player's inventory grid, like the container window: drag
    or click items across (`slotdrag` / `slot` autoplay commands extend to it). Talking to a
    companion can offer "Let me see your gear" (script `open_companion("guard")`). Gear that
    doesn't fit the companion's body sheet still works (the paperdoll draws what its layers
    allow, as for NPCs today). Some definitions may refuse gear (`CharacterDef.gear_locked`,
    serde default false: a wolf companion wears nothing).
  - *Using items on their own*: consumables in a companion's bag are used when they help —
    `Heal` potions below 40 % HP (one at a time, a cooldown), antidotes / cures when poisoned or
    burning (`CureEffect`), buff potions at the start of a fight (Strength, Haste: a hostile
    hunts within 5 tiles), heal scrolls like potions. Which item does what comes from its
    `on_use` actions (heal amount, effect cured or applied), evaluated for the companion instead
    of the player (`execute` with the companion as target: `Action::Heal` gains a target as in
    *Healing companions*). A toast or floating text shows it ("Guard drinks a Red Potion").
  - *Ammo*: ranged companions use ammo from their bag (`Ranged.ammo`, as the player does) and
    stop shooting (closing in to melee, or staying back with *Passive*) when out of it; the
    HUD portrait of *Healing companions* shows low ammo.
  - *Spells*: optional, later: companions with *Magic* learn spells from scrolls / books given
    to them (`Companion.spells`, mana pool from the definition) and cast heal / bolt spells by
    the same rules as items.
  - *Rules*: given items belong to the companion; dismissed companions keep their gear
    (they come back with it), knocked-out ones too (no drop); a project setting could make
    companions return lent gear when dismissed.
  - *Verify*: give the guard a potion and arrows via the window (autoplay `slot` / `slotdrag`
    into the companion tab), poison him below 40 % → he drinks it (`companions=` HP rises,
    inventory shrinks, toast); equip a bow on him → he shoots and his arrows run down; save /
    load keeps his bag; screenshot of the tab.
- **NPC routines: visible activities** (only visual; builds on *NPC schedules*, the Tool slot of
  *Minigames* and the held sprites): characters are seen fishing at the pond, hoeing a field,
  hammering at an anvil, chopping wood, sweeping, reading — without producing anything.
  - *Data*: `Activity { kind: ActivityKind, tool: String /* item id, "" = the kind's default or
    none */, facing: Option<Facing> }` with `ActivityKind { Fish, Farm, Craft, Chop, Mine, Sweep,
    Read, Custom { sheet: String } }` (appended later as needed); on a schedule stop
    (`ScheduleStop.activity: Option<Activity>`, serde default) and on a placed character that
    does it all day (`EntityInstance.activity`, in the map's RON `Meta` like the schedule).
    The editor gets an *Activity* combo (+ tool item, facing) in the Daily schedule rows and in
    the inspector of a placed character.
  - *Looks*: while doing it, the character stands at its spot facing the activity and is drawn
    with the tool from its Tool slot (or the activity's item) in the right hand, like the
    player's rod (`player_equipment_drawn` generalised to any character). A work cycle replaces
    the idle frame: the walk frames played slowly in place for arm movement, plus small
    per-activity effects drawn by a pure function of time like the fish shadows — *Fish*: line
    and bobber on the water in front (the same drawing as the player's, with an occasional
    splash and a fish lifted out now and then); *Farm*: a hoe swing every ~1.5 s with soil
    specks; *Craft*: a hammer stroke with sparks and the anvil `hit` sound when the player is
    near; *Chop* / *Mine*: chips of wood / stone; *Sweep*: dust puffs; *Read*: a book held,
    pages turning; *Custom*: a character-layout sheet of the creator's played instead (its own
    4 frames per facing). Sounds only within a few tiles and at the effects volume.
  - *Behaviour*: talking to a working character pauses the activity (it turns to the player,
    then goes back to work); a schedule moves it on at the next stop as today; nothing is
    saved (it's derived from the stop or instance). Optional later: activities that do produce
    something (the fishing NPC sells fresh trout at the market stall the next morning) via a
    trigger on the activity's end.
  - *Example*: Tomas fishes at the big pond in the afternoon instead of standing there,
    a farmer hoes the garden behind Tilda's shop (with *Farming*: the plots), the blacksmith of
    *Blacksmithing* hammers at the anvil by day, the elder reads by the well.
  - *Verify*: `settime 16` → Tomas at his stop with `activity=fish` in `schedule=` status and
    the rod in his hand (screenshot), the bobber on the water; talking to him turns him to the
    player and back; the editor rows show and change the activity (screenshot).
- **Gathering and trades** (four ideas building on *Crafting*; suggested order: mining →
  blacksmithing, farming → cooking). Shared groundwork, done with the first of them:
  - *Tools*: `ItemDef.tool: String` (serde default; "pickaxe", "hammer", "hoe", "knife", …)
    and `RecipeDef.tools: Vec<String>` (needed, not used up: in the inventory or held). The
    crafting window shows missing tools like missing ingredients (`Missing::Tool`).
  - *Timed prop states*: `PropDef.regrow: Option<Regrow { from: state, to: state, hours: f32 }>`
    and a world clock that counts days (`WorldState.day: u32`, serde default, advanced when the
    hour wraps; `total_hours()`), so a prop state can change after in-game time passes, also
    while the player is on another map (the state change is computed when the map is entered:
    `EntityState.changed_at`). Used for depleted ore veins and growing crops.
  - *Gathering nodes*: a prop's *Interact* (or a hit with the right tool) gives items from a
    loot table and switches it to a state ("depleted", "harvested"); `PropDef.gather:
    Option<Gather { tool, hits, loot, then_state, skill }>` with a progress bar over the prop
    while hitting.
- **Mining**: ore veins (copper, iron, silver; gem veins rarely) as gathering-node props that
  need a *pickaxe* (a new item, also a weak weapon). A few hits (fewer with the *Mining* skill:
  `SkillEffect::Gather { tool: "pickaxe", percent }`) break off ore and maybe stone or a gem,
  the vein turns *depleted* and regrows after some in-game days. Ore and gems sell at merchants
  and feed *Blacksmithing*. Example: veins in the old mine (the dark corridors, so a torch
  helps), a pickaxe in the mine's entrance chest, a *Miner* skill in the example's skill list.
  Verify: autoplay with a pickaxe hits a vein three times → `inv` has iron ore, the prop state
  is `depleted`; `settime` past the regrow time (or a new day) and re-entering the map → full
  again; without a pickaxe a toast says what is needed.
- **Blacksmithing**: two stations: a *forge* smelts ore into ingots (copper / iron / silver ore
  plus coal → ingot), an *anvil* forges ingots and leather into weapons, armor, arrowheads and
  tools; anvil recipes need a *hammer* (`RecipeDef.tools`). A *Smithing* skill unlocks better
  recipes (recipe `skill` / `level`) and could raise the made item's stats: an optional
  `RecipeDef.quality_skill` that adds a "+1" variant (`ItemDef.upgrade_of`, a separate item id
  per quality, so items stay plain definitions). Optional idea: *upgrading* an equipped weapon at
  the anvil (sword + 2 iron ingots → fine sword). Example: a smithy next to Tilda's shop with a
  blacksmith NPC (Daily schedule: works the anvil by day), forge and anvil props (the forge's
  "lit" state glows: a light source), recipes for an iron sword, iron helmet, chainmail, arrows
  (iron ingot + wood) and the pickaxe itself. Verify: smelt and forge in autoplay, `status`
  inventory; the forge's light at night (screenshot).
- **Cooking**: a *cooking fire* or *stove* station (a campfire prop; recipes may require the
  station's prop to be in a state: `RecipeDef.station_state`, e.g. "lit"). Raw food (meat from
  animals' loot, fish from *Minigames: fishing*, mushrooms, apples, crops from *Farming*)
  becomes cooked food that heals more and gives timed buffs through the existing effects
  (*Regen*, *Strength*, *Haste*): stew = Regen for a minute, roasted fish = heal 3 hearts. A
  *Cooking* skill unlocks recipes and could make food heal more (`SkillEffect::FoodBonus
  { percent }` applied in `Action::Heal` from consumables). Optional: a *Hunger* system
  (project setting, off by default) where an empty stomach slowly stops HP regeneration — only
  if a game wants survival elements. Example: a campfire in the Wilds and a stove in the
  elder's house, four recipes, raw meat from the Wilds' animals. Verify: cook a stew at the
  campfire, eat it → `effects=` shows Regen; the "lit" requirement refuses an unlit fire.
- **Farming**: tilled soil plots (a prop with states *empty* → *planted* → *growing* → *ripe*)
  where the player plants seeds (using a seed item on the plot: the existing "use item on the
  prop in front" path, `TriggerOn::UseItem` on props, or a plot-specific rule `PropDef.crop:
  Option<CropSpec { seeds: Vec<(seed item, crop id)> }>`), crops grow over in-game days (timed
  prop states; rain on the map counts as watering and speeds it up, an optional *watering can*
  tool does the same by hand), and ripe plots are harvested for the crop (and sometimes seeds
  back) with a *hoe* or by hand, returning to *empty*. Crops: `CropDef { id, name, seed, stages:
  Vec<(sprite, hours)>, harvest: loot }` in a new `defs/crops.ron` (or a section of
  `recipes.ron`) so one plot prop grows any crop; the plot draws the crop's stage sprite on top
  of the soil. Saves keep the planting time per plot (`EntityState.planted: Option<(crop,
  total_hours)>`). Example: the fenced garden behind Tilda's shop becomes six plots, seeds
  (wheat, carrots, pumpkins) at Tilda's, crops sell to her and feed *Cooking*. Verify: plant,
  `settime` forward by days (`settime` gains a `+H` form that also advances `day`), re-enter,
  `status` shows the plot ripe, harvest → inventory; a rainy map grows faster; save / load
  keeps the growth.
- **The player uses furniture** (builds on the `UseSpot` of the idea above; either can come
  first): sit on chairs and benches, sleep in beds.
  - *Using*: interacting with a prop that has a use spot (E / right-click, nothing else on the
    tile in front) puts the player on the spot in its pose, drawn like a character's (sit: lower
    with clipped legs, lie: rotated with the bed's "occupied" overlay). A movement key, Cancel or
    an attack stands the player up at the tile in front of the prop. The prop's own *Interact*
    triggers run first; the spot is used only when none of them ran (so a bed can still have a
    "Sleep here?" dialog). Taken spots (an NPC sits there) refuse with a toast. Not saved: a
    loaded game starts standing. `status` prints `pose=`; autoplay `press e` at a chair works.
  - *Sitting*: cosmetic plus a small rest bonus (HP / mana regenerate faster while sitting, a
    project setting, default ×2), no attacks; the camera stays.
  - *Sleeping*: a bed asks "Sleep until morning?" (or *for N hours*: `UseSpot.sleep_until`
    hour, default 7). Yes: fade out, the clock jumps, HP and mana are restored, timed effects
    run out, spawners with *Respawn after* timers advance, scheduled characters are placed at
    their new stops (`place_on_schedule` for every one), the weather and lighting follow the
    hour; fade in, the player gets up beside the bed. Refused while hostile characters are in
    sight ("You can't sleep with enemies nearby") and in cut-scenes. Optional conditions on the
    spot (`UseSpot.conditions`, the condition editor; e.g. a paid inn room: flag `room_paid`)
    with a refusal text.
  - *Events and scripts*: `TriggerOn::Sleep` (appended; map and prop triggers, after waking: a
    dream, an ambush, a quest stage), `Action::Sleep { hours }` / script `sleep(hours)` to rest
    without a bed (camping, an inn keeper's "Rest" choice), `is_sitting()` / `is_sleeping()` in
    conditions via `Cond::Script` or a `Cond::Activity { target: "player", pose }` shared with
    the NPC idea. An autosave after waking (like map changes).
  - *Example*: the bed in the elder's house (sleep until 7:00, free), Tilda's inn-style "Rest
    (10 coins)" choice using `sleep(8)`, benches and chairs to sit on in the village and the
    shop.
  - *Verify*: autoplay `press e` at a chair → `pose=sit`, `hold left` stands up; at the bed at
    22:00 with 5 HP: the choice, `press e` → `time=07:00`, full HP, Tomas at his 07:00 stop;
    with a slime in sight the toast and no time jump; screenshots of sitting and sleeping.

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
