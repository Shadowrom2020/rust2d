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
companions, crafting, achievements, minigames, furniture and rest.

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
quad-snd code, so their loop point can have the same short gap (*Gapless music loops*, 0.8.0). Particles and sound pause with the world
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

### [x] The player uses furniture — `feature/furniture`

**Goal.** The player sits on chairs and benches and sleeps in beds: sitting is a short rest,
sleeping skips the night (restored, the world moved on). Props become furniture with a *use
spot*; NPC furniture (*Schedules across maps and with furniture*, 0.7.0) reuses it later.

**Design.**
- *Use spots*: `PropDef.use_spot: Option<UseSpot { offset: (f32, f32), facing: Facing, pose:
  Pose, sleep_until: f32, conditions: Vec<Condition>, refuse_text: String }>` with `Pose { Sit,
  Lie, Stand }` (serde defaults; `sleep_until` default 7.0, only for beds). The spot is where the
  user's feet go, relative to the prop's feet; `facing` the way the user looks. Editor:
  *Database › Props › Use spot* (pose, offset with a preview of a character on the prop at that
  spot, facing, the bed's wake-up hour, conditions with the condition editor and a refusal text).
- *Using*: interacting with a prop that has a use spot (E / right-click, nothing else on the
  tile in front) puts the player on the spot in its pose. *Sit* draws the paperdoll a few
  pixels lower with the legs clipped (a shortened source rect, like the dialog portraits'
  `draw_frame_part_in`); *Lie* draws it rotated 90° over the bed, with the bed's optional
  "occupied" state on top (a blanket overlay, so the prop decides what covers the sleeper);
  *Stand* only faces the furniture (counters, anvils). A movement key, Cancel or an attack
  stands the player up at the tile in front of the prop. The prop's own *Interact* triggers run
  first; the spot is used only when none of them started anything (a bed can still have its own
  dialog). Not saved: a loaded game starts standing. `status` prints `pose=`.
- *Sitting*: cosmetic plus a small rest bonus (HP / mana regenerate faster while sitting; a
  project setting, default ×2), no attacks.
- *Sleeping*: a bed asks "Sleep until morning?" (or until its `sleep_until` hour). Yes: fade
  out, the clock jumps (a new day when it passes midnight), HP and mana are restored, timed
  effects run out, spawners with *Respawn after* timers advance, scheduled characters are placed
  at their new stops (`place_on_schedule` for every one), the weather and lighting follow the
  hour; fade in, the player gets up beside the bed. Refused while hostile characters are in
  sight ("You can't sleep with enemies nearby") and during cut-scenes; the spot's conditions
  can refuse too (a paid inn room: flag `room_paid`). An autosave after waking.
- *Events and scripts*: `TriggerOn::Sleep` (appended; map and prop triggers, after waking: a
  dream, an ambush, a quest stage), `Action::Sleep { hours }` (appended) / script `sleep(hours)`
  to rest without a bed (camping, an inn keeper's "Rest"), `is_sitting()` / `is_sleeping()`.

**Decided while building.** `UseSpot` also has `cover: SpriteRef` (drawn over the user at
the prop's feet: the bed's quilt) instead of an "occupied" prop state. In a bed, `facing` is the
side the head is on: Up = an upright bed (the front idle frame, head on the pillow), Left /
Right = a sideways bed (the frame turned ±90° around the body, `draw_frame_colored_rotated`),
Down = upside down; chairs come one prop per facing (the example: `chair`, `chair_up` with its
backrest as the cover, `chair_left`, `chair_right`; `bed` and `bed_side`); *Sit* draws the top 80 % of the frame
6 px lower, so the chair's posts and legs show beside the sitter (the example chair is drawn
wide for that). The player sorts after a bed (`player_sort_y`) so the cover goes on top.
Sitting: 1 HP per 4 s and mana at twice the rate (no project setting). Standing up: a movement
key held, an attack or E; the player returns to where they stood (`Seat::back`, also what a
save stores while seated). The bed's question is a Choice into `_sleep_answer` followed by an
*If* with `Action::Sleep` (so it works with the runner, dialogs and the dev console); the bed
the player chose is remembered for that Sleep action (`Game::bed`). Sleeping: 0.8 s fade, 1.4 s
dark with "Z z z", 0.8 s back; spawner respawn timers are set to zero
(`spawners_catch_up`), there is no day counter yet. `sleep(hours)` is registered for integers
and floats because Rhai's own `sleep(seconds)` would win over a Dynamic parameter. Not done:
NPCs on furniture (their own item in 0.7.0), Tilda's paid "Rest" (the docs show `sleep(8)`).

**Touch points.** `defs.rs` (`UseSpot`, `Pose`), `runtime/furniture.rs` (new: using, standing
up, sleeping), `runtime/{mod,paperdoll,lights,spawner,schedule}.rs`, `events/mod.rs`,
`script.rs`, `render/mod.rs` (rotated / clipped frames), editor `database.rs` (prop form) and
`widgets.rs` (action fields), `script_editor.rs`, gen_example (a bed, chair and bench prop with
art, placements), README, scripting.md.

**Example.** A bed in the elder's house (sleep until 7:00, free), Tilda's "Rest (10 coins)"
choice using `sleep(8)`, benches on the village green and chairs in the shop; the user asked for
chairs in every direction around a table (the elder's house) and sideways beds too.

**Verify (done).** Autoplay on a scratch copy with crafted saves: at Tilda's chair `press e`
→ `pose=sit#…` (screenshot: posts and legs beside the hero), `hold left` stands up; at the
elder's bed at 22:00 with 5 HP: the question (screenshot), `press e` → lying under the quilt
(screenshot), `Z z z`, then `time=07:00`, `hp=12`, `autosave=saved` and "You feel rested."; the
village bench (screenshot); `sleep(8)` from the console 10:01 → 18:01; near the slime den
"You can't sleep with enemies nearby." and no time jump; the editor's Furniture section of the
bed (screenshot); the elder's table with a chair on each side, sitting on each of the
four (screenshots: the up-facing chair's backrest over the sitter), sleeping in the sideways
guest bed (screenshot); unit tests for the RON defaults and the script functions. Not checked by
script: the Woke up triggers (no example uses one) and the spot's conditions.

### [x] Dialog portraits — `feature/portraits`

**Design.** Dialog boxes show a portrait of the speaker (paperdoll of the NPC's look, or the
player's) at the left; nothing to configure per character, one project option turns it off.
A proper node-graph dialog editor stays in the backlog.

**Decided while building.** `runtime/portraits.rs`: `find_speaker(raw, shown, owner)` →
`Speaker::{Player, Entity(uid), Def(id)}` in this order: none without a speaker; `{self}` /
`{uid:N}` = that entity; `{player}` or the player's name = the hero; the trigger's owner if
its entity / definition name fits; the nearest active character on the map whose name fits;
a character definition whose name fits (`names_match`: case-insensitive, all words of the
shorter name in the longer, so "Elder" speaks as "Elder Rowan"). The portrait is the idle
front frame's head and shoulders (`BUST`, the top 56 % of a 32 x 48 frame; other frame sizes
the top 60 %) without held items, drawn by `Assets::draw_frame_part_in`; the dialog box makes
room for a 120 px square (`Ui::with_portrait`, `portrait_rect`). `ProjectConfig::
dialog_portraits` (serde default true) with a checkbox in *Database › Project*. The example's
intro cut-scene got a line of the hero's.

**Verify (done).** Autoplay on a scratch copy: talking to the elder shows "Elder Rowan" with
the elder's portrait (screenshot); the intro shows the guard's and the hero's lines with their
portraits (screenshot); a unit test for the name matching. Not checked by script: the project
option off (compile-checked; `find_speaker` returns none first thing).

## 0.6.0 — The party (companions and faces)

Theme: companions become real party members (the player heals them, picks them up when they
fall, gives them orders and gear), and characters can have drawn portraits. Builds on
*Companions* and *Dialog portraits* of 0.5.0.

### [ ] Healing companions — `feature/heal-companions`

**Goal.** Spells, scrolls and potions can heal a companion, not only the player, and the HUD
shows how the companions are doing.

**Design.**
- *Target choice*: a *Heal* spell (`SpellEffect::Heal`) and consumable items whose *on use*
  heals (`Action::Heal`) ask whom to heal while a companion is on the map: the aiming mode of
  bolts (`update_aim` in `runtime/skills.rs`) with friendly targets: click / select the player or
  a companion (keyboard and pad: Left / Right cycles, Confirm heals, Cancel aborts and refunds
  nothing used yet). With no companion on the map it heals the player at once, as today. Items
  get `ItemDef.use_on: UseTarget { Player, Ally, Choose }` (serde default `Player`) so a potion
  can be the player's only or offered to companions; heal scrolls are consumable items with
  `Choose`.
- *Actions and scripts*: `Action::Heal` gains `target: String` (`#[serde(default)]`, "" = the
  player; old maps keep healing the player; the variant keeps its index, the field is new), so
  *Heal* actions on a named character work; script `heal_npc("guard", 4)`. `heal_area`
  (`runtime/fx.rs`) already heals friendly characters and stays. Companion HP is written back
  with `sync_companion_hp`.
- *HUD*: under the player's hearts, one small row per companion: the head from the portrait
  bust (`portraits.rs` `BUST`, `draw_frame_part_in`) and an HP bar; while choosing a heal target
  the chosen row is highlighted and rows are clickable. Hidden with the HUD (`H`).
- `status()`: `companions=` already has HP; add `aim=heal:<target>` while choosing.

**Touch points.** `defs.rs` (`UseTarget`, `ItemDef.use_on`), `events/mod.rs` (`Heal.target`),
`runtime/{skills,companions,combat,mod,inventory_ui}.rs`, a HUD part in `runtime/ui.rs` or
`companions.rs`, `script.rs` (`heal_npc`), editor `widgets.rs` (Heal target field) and
`database.rs` (item *Use on*), `script_editor.rs`, README, scripting.md.

**Example.** The guard companion; Tilda sells a *Healing Scroll* (`Choose`); the red potion
becomes `Choose`.

**Verify.** Autoplay from a crafted save with the guard at 3/8: use a potion → target choice
(`aim=heal:player`), Right, Confirm → `companions=` guard 7/8, the player's HP unchanged; the Heal
spell the same; without a companion the potion heals at once; screenshot of the HUD rows and
the choice. Unit test: an old map's `Heal { amount }` loads with an empty target.

### [ ] Knocked-down companions — `feature/downed-companions` (follows *Healing companions*)

**Goal.** At 0 HP a companion falls and stays down until healed, instead of limping home.

**Design.**
- Down: drawn lying (the furniture's rotated drawing, `draw_frame_colored_rotated`, facing
  Left) and greyed; it stays where it fell, isn't hunted or hit (`companion_targets` and enemy
  AI skip it), doesn't follow or fight. `Companion.down: bool` (serde default) keeps it in save
  games; `status()` prints `down` in `companions=`.
- Getting up: any heal on it (potion / scroll / spell via *Healing companions*, `heal_npc`,
  `heal_area`) gets it up with the healed HP. A project setting `ProjectConfig.companion_down:
  DownRule { StaysDown, GetsUp { seconds }, GoesHome }` (serde default `StaysDown`; `GoesHome`
  = today's `dismiss(i, true)`), in *Project settings*.
- Leaving the map with a downed companion asks "Leave {name} behind?" (yes = it goes home as
  dismissed, no = stay); a defeat of the player ignores downed companions (the defeat menu as
  today). Talking to a downed companion shows "{name} is out cold." unless a trigger matches.

**Touch points.** `state.rs` (`Companion.down`), `project.rs` (`DownRule`),
`runtime/{companions,combat,mod,maps,furniture}.rs` (the rotated drawing made reusable for
characters), editor *Project settings*, README.

**Example.** Nothing new beyond the guard; the setting stays at *stays down*.

**Verify.** Autoplay: `call` a script that damages the guard to 0 → `down`, a slime next to him
doesn't attack him, he stays when the player walks away; a potion on him → up with its HP and
following; save / load while down keeps him down at his place; leaving the map asks; screenshot
of him lying. Unit test: old saves without `down`.

### [ ] Commands for companions — `feature/companion-orders`

**Goal.** The player tells companions how to behave: attack, defend, stay passive or wait.

**Design.**
- *Orders*: *Attack* (today: fight hostiles near the player), *Defend* (only fight what attacks
  the player or this companion, stay close), *Passive* (never fight, keep out of reach: move
  away from the nearest hostile within 3 tiles, behind the player), *Stay* (wait here, fight
  what comes within 2 tiles; *Follow* ends it). `Companion.order: Order` (serde default
  `Attack`), saved.
- *Giving them*: `Act::Command` (key `G`, pad `RB`; rebindable, in the Controls page) opens a
  small menu (game UI, ASCII) listing the orders, with the companion to address first when
  there are several (or *All*). Script `order("guard", "stay")`, `order_of("guard")`;
  `Action::CompanionOrder { target, order }` (appended) so a companion's dialog can offer them.
- *Behaviour*: `companion_fight` filters targets by the order (Defend: hostiles whose AI target
  is the player or this companion; Passive: none). *Stay* skips following and keeps a `post`
  position; leaving the map with a staying companion: it comes along (project setting
  `stay_follows_across_maps`, default true) — no dialog. The HUD row of *Healing companions*
  shows the order as a letter (A / D / P / S).

**Touch points.** `state.rs` (`Order`), `runtime/{companions,input,menu or ui}.rs`,
`events/mod.rs`, `script.rs`, `script_refs.rs` (no ids: the name is a character name like
`follow`), editor `widgets.rs`, `script_editor.rs`, README (controls table), scripting.md.

**Example.** The guard's dialog after joining offers "Wait here" / "Follow me" / "Stay out of
fights" via `order`.

**Verify.** Autoplay: `press g` + menu choices → `status` `companions=["guard:8/8:stay@…"]`;
*Passive*: a slime next to him isn't attacked (its HP unchanged after 5 s); *Defend* ignores a
slime that hasn't noticed the player; *Stay* keeps him put while the player walks off; save /
load keeps the orders; screenshot of the menu.

### [ ] Companion inventory — `feature/companion-inventory`

**Goal.** The player equips companions with gear and hands them items they use on their own.

**Design.**
- *Groundwork that exists*: NPCs wear equipment and carry items (`RtEntity.equipment` /
  `inventory`, `npc_equip` / `npc_unequip` / `give_item_to`, saved per placed character in
  `WorldState.npc_items`, which companions already use with their home map / uid), and their
  weapon and defense follow their gear (`npc_items::apply_gear`).
- *Window*: a *Companion* tab in the character window (`charwin.rs`; Tab cycles; a companion
  picker in the tab when there are several) showing the companion's paperdoll with its slots
  and its carried items next to the player's inventory grid, like the container window: drag or
  click items across (`slot` / `slotdrag` autoplay commands extend to it, `wintab companion`).
  Script `open_companion("guard")` for a "Let me see your gear" dialog choice. Gear that the
  body sheet can't show still counts. `CharacterDef.gear_locked` (serde default false) refuses
  gear (a wolf companion).
- *Using items on their own*: consumables in a companion's bag are used when they help: heal
  items below 40 % HP (one at a time, 5 s cooldown), cures when poisoned or burning, buff items
  when a hostile hunts within 5 tiles. What an item does comes from its `on_use` actions,
  executed with the companion as the target (`Action::Heal.target` from *Healing companions*;
  effects get a target the same way). A toast: "Guard drinks a Red Potion".
- *Ammo*: ranged companions use ammo from their bag (`Ranged.ammo`, as the player does) and
  close in to melee (or stay back with *Passive*) when out of it; the HUD row shows low ammo.
- *Rules*: given items belong to the companion; dismissed or downed companions keep them.
- Later (not in this item): companions learning spells from scrolls.

**Touch points.** `runtime/{charwin,inventory_ui,companions,npc_items,combat}.rs`, `defs.rs`
(`gear_locked`), `script.rs`, game `main.rs` (autoplay), README, scripting.md.

**Example.** The guard can be given the bow and arrows; a second potion in the guard's quest
reward to try it.

**Verify.** Autoplay: give the guard a potion and arrows via the tab (`slot` / `slotdrag`),
damage him below 40 % → he drinks it (`companions=` HP rises, his bag shrinks, toast); equip a
bow on him → he shoots and his arrows run down; save / load keeps his bag; screenshot of the
tab.

### [ ] Portrait images — `feature/portrait-images`

**Goal.** Creators give characters a drawn portrait; without one the automatic head-and-shoulders
rendering of *Dialog portraits* stays.

**Design.**
- *Data*: `CharacterDef.portrait: SpriteRef` and the placed character's own in the map's RON
  `Meta` (empty = automatic), serde defaults; *moods*: `CharacterDef.moods: BTreeMap<String,
  SpriteRef>` picked by a dialog's `mood` (`say("Elder Rowan", "...", #{ mood: "worried" })`;
  `Action::Dialog` gains `mood` with a serde default). The player: `ProjectConfig.player_portrait`
  or, with the character creator, one per playable body (`SheetDef.portrait`).
- *Drawing*: `find_speaker` stays; `draw_dialog_portrait` draws the image (any frame size,
  scaled to fit with nearest-neighbour filtering) when there is one, else the bust. The HUD
  rows of *Healing companions* use it too.
- *Editor*: *Portrait* field (sprite chooser with a preview at dialog size and *Clear* =
  automatic) in *Database › Characters* and the placed-character inspector; *New blank portrait*
  (a 64 x 64 frame on a `portraits` sheet, opened in the pixel editor); import of a PNG as a
  portrait. Library export carries the sheet (`refs_of_character`), renames follow (`refactor.rs`).

**Touch points.** `defs.rs`, `map/io.rs` (Meta), `events/mod.rs` (`mood`), `runtime/portraits.rs`,
`script.rs` (`say` options), `refactor.rs`, editor `database.rs`, `widgets.rs`, `pixel.rs`,
gen_example (portrait art), README, scripting.md.

**Example.** Drawn portraits for the elder (with a *worried* mood in the intro) and Tilda; the
others keep the automatic one, to show both.

**Verify.** The elder's dialog shows his image, the guard's the bust (screenshots); the intro's
worried line shows the mood image; clearing the field falls back; library export of the elder
includes the portrait sheet (unit test in `library.rs`).

## 0.7.0 — Trades and town life

Theme: the player gathers, smiths, farms and cooks, and the townsfolk visibly live their days
across maps, using furniture and working at their trades. Suggested order: the gathering
groundwork with mining → blacksmithing; farming → cooking; then the townsfolk items, which show
the trades in their routines.

### [ ] Mining and gathering — `feature/mining`

**Goal.** Ore veins the player breaks with a pickaxe, plus the groundwork the other trades use:
tools in recipes, gathering props and prop states that change over in-game days.

**Design.**
- *Tools* (partly there: `ItemDef.tool` names a tool kind and the Tool slot holds one):
  `RecipeDef.tools: Vec<String>` (tool kinds needed, not used up: equipped in the Tool slot or
  in the inventory); the crafting window lists a missing tool like a missing ingredient
  (`Missing::Tool`).
- *Days*: `WorldState.day: u32` (serde default 0), advanced when the hour wraps (also by
  sleeping, `pass_time`); `total_hours() = day * 24 + hour`. Autoplay `settime +H` advances
  across days.
- *Timed prop states*: `PropDef.regrow: Option<Regrow { from, to, hours }>`;
  `EntityState.changed_at: Option<f64>` (total hours, serde default) is set on every state change,
  and the state is moved on when the map is entered or the time passes it on the current map,
  so it also works while the player is elsewhere.
- *Gathering nodes*: `PropDef.gather: Option<Gather { tool, hits, loot, then_state, skill }>`:
  with the tool equipped (or in the bag), attacking or E on the prop counts a hit (a progress
  bar over the prop, chips as an effect); after `hits` the loot table rolls (`roll_loot`), the
  prop switches to `then_state` ("depleted"). `SkillEffect::Gather { tool, percent }` (appended)
  lowers the hits.
- *Mining*: copper, iron and silver veins (gem veins rarely) need a *pickaxe* (tool kind
  "pickaxe", also a weak weapon); depleted veins regrow after 3 in-game days.

**Touch points.** `crafting.rs`, `runtime/crafting_ui.rs`, `state.rs` (`day`, `changed_at`),
`defs.rs` (`Regrow`, `Gather`), `skills.rs`, `runtime/{mod,lights,furniture,combat}.rs`, new
`runtime/gather.rs`, editor `database.rs` (prop form), game `main.rs` (`settime +H`), README.

**Example.** Veins in the old mine's dark corridors, a pickaxe in the mine's entrance chest, a
*Mining* skill; ore sells at Tilda's.

**Verify.** Autoplay with a pickaxe: hit a vein three times → `inv` has iron ore, the prop state
is `depleted`; `settime +72`, re-enter → full again; without a pickaxe a toast names it; a
recipe with a tool shows it missing. Unit tests: old saves without `day`, `regrow` arithmetic.

### [ ] Blacksmithing — `feature/blacksmithing` (follows *Mining*)

**Goal.** A forge smelts ore into ingots and an anvil forges them into weapons, armor and tools.

**Design.**
- Two crafting stations (props with `Open crafting`): the *forge* (ore + coal → ingot) and the
  *anvil* (ingots, leather, wood → weapons, armor, arrowheads, tools; recipes need a *hammer*
  via `RecipeDef.tools`).
- A *Smithing* skill unlocks better recipes (recipe `skill` / `level`). Quality: optional
  `RecipeDef.quality: Vec<(level, item)>` makes a better item id at higher skill (a *Fine Iron
  Sword* is its own definition, so items stay plain).
- Optional upgrade recipes: an equipped item as an ingredient (sword + 2 iron ingots → fine
  sword), using the existing ingredient check.
- The forge's "lit" state is a light source (`LightSpec` on the state) and needs coal to light.

**Touch points.** `crafting.rs` (`quality`), `runtime/crafting_ui.rs`, gen_example (smithy map
part, forge / anvil / ingot / coal art, recipes), README.

**Example.** A smithy next to Tilda's shop with a blacksmith NPC (by day at the anvil, see *NPC
routines*), forge and anvil, recipes for an iron sword, helmet, chainmail, arrows and the
pickaxe itself.

**Verify.** Autoplay: smelt iron ore, forge a sword (`inv`), the Smithing level gives the fine
sword; the forge's light at night (screenshot); without a hammer the anvil recipe shows it
missing.

### [ ] Farming — `feature/farming` (follows *Mining*: days, timed states)

**Goal.** The player plants seeds in soil plots, crops grow over in-game days and are harvested.

**Design.**
- Crops: `CropDef { id, name, seed, stages: Vec<(SpriteRef, hours)>, harvest: loot }` in a new
  `defs/crops.ron` (`project.rs` `DEFS`, `release.rs`, `library.rs`), so one plot prop grows
  any crop; Database tab *Crops*.
- Plots: `PropDef.plot: bool` (serde default); using a seed item on a plot (the existing "use
  item on the prop in front" path) plants it; `EntityState.planted: Option<(crop, total_hours)>`
  (serde default) is saved; the plot draws the crop's stage sprite on top of itself. Rain on
  the map counts as watering (growth × 1.5 while it rains on the plot's map); a *watering can*
  tool does the same for a day. Ripe plots are harvested with E (a *hoe* optional: more yield),
  and return to empty.
- Growth is computed from total hours, so it continues while the player is away.

**Touch points.** new `crops.rs` (def), `project.rs`, `release.rs`, `library.rs`, `refactor.rs`,
`state.rs`, `runtime/{mod,gather}.rs`, editor `database.rs` (Crops tab, prop *Plot*), gen_example
(plots, seeds, crop stages), README.

**Example.** The fenced garden behind Tilda's shop becomes six plots; wheat, carrot and pumpkin
seeds at Tilda's; crops sell to her and feed *Cooking*.

**Verify.** Autoplay: plant carrots, `settime +48`, re-enter → `status` shows the plot ripe,
harvest → `inv`; a rainy map grows faster; save / load keeps the growth; screenshots of the
stages.

### [ ] Cooking — `feature/cooking` (follows *Farming*)

**Goal.** Raw food becomes cooked food that heals more and gives timed buffs.

**Design.**
- A cooking fire or stove is a crafting station; `RecipeDef.station_state: String` (serde
  default "") requires the station prop's state ("lit").
- Raw food (meat from animals' loot, fish from fishing, mushrooms, apples, crops) → cooked food
  whose *on use* heals and applies the existing effects (*Regen*, *Strength*, *Haste*).
- A *Cooking* skill unlocks recipes; `SkillEffect::FoodBonus { percent }` (appended) raises
  healing from items tagged `ItemDef.food: bool`.
- No hunger system (left out on purpose; a game can script one).

**Touch points.** `crafting.rs`, `runtime/crafting_ui.rs`, `skills.rs`, `runtime/combat.rs`
(heal bonus), `defs.rs` (`food`), gen_example, README.

**Example.** A campfire in the Wilds and the elder's stove; four recipes (roast fish, stew,
carrot soup, apple pie); raw meat from the Wilds' animals.

**Verify.** Autoplay: cook a stew at the lit campfire, eat it → `effects=` shows Regen; an unlit
fire refuses the recipe; the Cooking bonus raises the healed HP (two statuses).

### [ ] Schedules across maps and with furniture — `feature/schedule-maps`

**Goal.** A scheduled character goes to other maps (home, the shop, the tavern) and uses
furniture there: sleeps in a bed, sits on a chair, stands at a counter.

**Design.**
- *Stops on other maps*: `ScheduleStop.map: String` (serde default, "" = its own map). The
  character walks to the portal / transition zone of its map that leads there (the nearest
  `ZoneKind::Portal` / `Transition` with that `target_map`) and leaves; on its home map it is
  `away` (as *inside* stops today). Where everyone is lives in the save game:
  `WorldState.npc_whereabouts: BTreeMap<String /* home map */, BTreeMap<u32, String /* map */>>`
  (serde default).
- *Visitors*: building a map's entities adds the characters whose current stop is on it (their
  instance comes from the home map through the map cache), with a runtime uid range of their own
  like `DROPPED_UID_BASE`; their triggers, look and items come along, state changes are saved
  under the home map's uid. Arriving while the player is there: they appear at the portal's
  arrival spawn and walk to the stop; leaving: they walk to the next portal and disappear. Maps
  the player isn't on aren't simulated (placed at the stop of the hour on entering, as today).
- *Furniture* (the player's `UseSpot` exists since 0.5.0): `ScheduleStop.prop: String` (an
  entity name on the stop's map; the tile is the spot in front of it). Arrived, the character
  snaps to the spot and takes the pose, drawn like the seated / lying player (`furniture.rs`
  `draw_seated_player` generalised to any character, with the prop's cover). A used prop is
  skipped by others and refused to the player ("Someone is using it").
- *Interaction*: talking to a sleeping character runs its triggers, with `Cond::Pose { target,
  pose }` (appended) for "Zzz…" lines, or a default "{name} is asleep." toast when no trigger
  matches. `status` `schedule=` gains the map and `pose`.

**Touch points.** `entity.rs` (`ScheduleStop`), `state.rs`, `runtime/{schedule,patrol,mod,
furniture,maps}.rs`, `events/mod.rs`, editor `app.rs` / `canvas.rs` (schedule editing: the stop's
map and prop, a stop on another map shown in a list), README.

**Example.** Tomas sleeps in the elder's sideways guest bed instead of vanishing at the door;
Tilda sits behind her counter by day; the bench on the village green in the afternoon.

**Verify.** Autoplay at 20:30 follows Tomas through the door (`status` on the house map shows him
walking in, then `pose=lie`), save / load at night keeps him in bed, the house at noon shows
nobody; the player can't take his bed; screenshots of sitting and sleeping NPCs.

### [ ] NPC routines: visible activities — `feature/activities` (follows the trades and *Schedules across maps*)

**Goal.** Characters are seen fishing, hoeing, hammering, chopping, sweeping or reading, without
producing anything.

**Design.**
- *Data*: `Activity { kind: ActivityKind, tool: String /* item id */, facing: Option<Facing> }`,
  `ActivityKind { Fish, Farm, Craft, Chop, Mine, Sweep, Read, Custom { sheet } }`; on a schedule
  stop (`ScheduleStop.activity`, serde default) and on a placed character doing it all day (in
  the map's RON `Meta`). Editor: an *Activity* combo (+ tool, facing) in the Daily schedule rows
  and the placed-character inspector.
- *Looks*: the character stands at its spot facing the activity with the tool in hand
  (`player_equipment_drawn` generalised to any character); a work cycle (the walk frames slowly
  in place) and per-activity effects drawn as pure functions of time like the fish shadows:
  *Fish* line and bobber (`rod_tip`) with now and then a splash; *Farm* soil specks; *Craft*
  sparks and the anvil sound nearby; *Chop* / *Mine* chips; *Sweep* dust; *Read* a book;
  *Custom* a creator's sheet. Sounds within a few tiles at the effects volume.
- *Behaviour*: talking pauses it (faces the player, then back to work); nothing is saved.

**Touch points.** `entity.rs`, `map/io.rs` (Meta), new `runtime/activities.rs`,
`runtime/{schedule,minigame,paperdoll}.rs`, editor `app.rs` / `widgets.rs`, gen_example, README.

**Example.** Tomas fishes at the pond in the afternoon, the blacksmith hammers at the anvil,
a farmer hoes the garden plots, the elder reads by the well.

**Verify.** `settime 16` → Tomas with `activity=fish` in `schedule=` and the rod in hand, bobber
on the water (screenshot); talking turns him and he goes back; the editor rows show and change
the activity (screenshot).

## 0.8.0 — Scenery and sound

Theme: maps gain smoother terrain edges and depth, and music loops without a gap.

### [ ] 47-tile corner auto-tiling — `feature/autotile-47`

**Goal.** Terrains with inner corners: the 16-tile sets of 0.4.0 only look at the four sides, so
a diagonal neighbour leaves a hard corner; 47-tile ("blob") sets look at all eight.

**Design.**
- `Terrain.blob: Vec<Option<u32>>` (serde default empty = 16-tile mode as today; 47 entries
  otherwise; a `Vec` because serde has no 47-element arrays). The mask uses eight neighbours,
  a diagonal counting only when both cardinals next to it are set, which leaves 47 distinct
  masks; a table maps the 256 raw masks to the 47 slots. `tile_for` falls back to the 16-tile
  set, then the inside, so a half-filled blob set still paints.
- `autotile::paint` / `retile` use the 8-neighbour mask for blob terrains (the cells to update
  stay the 3 × 3 around the change). `joins` and `base` work unchanged.
- Editor (*Database › Tilesets › Terrains*): a *Corners (47 tiles)* toggle; the slot grid becomes
  the common 7 × 7 blob template with a mask drawing in each empty slot; *Fill from template*
  assigns 47 slots from a block of the tileset in that standard order (how most blob tilesets
  are published).
- Imports: Tiled's *mixed* Wang sets with 47 tiles map onto it (`import/tiled.rs`, reported
  when it doesn't fit).

**Touch points.** `autotile.rs`, editor `terrain.rs`, `database.rs`, `import/tiled.rs`,
gen_example (art and terrain), README.

**Example.** The starter sand path gets a blob set, composed from quarter tiles in the generator
(inside, edge, outer corner, inner corner), and the village's paths are retiled with it.

**Verify.** Unit tests: the 256 → 47 table (47 distinct, symmetric under rotation), `paint` on an
L-shape gives the inner corner; editor script `terrainpaint` on an L, screenshot; old terrains
without `blob` still paint (test).

### [ ] Parallax layers — `feature/parallax`

**Goal.** Image layers that move at their own speed behind or over a map: a sky or abyss under
a cliff map, drifting cloud shadows or mist above it.

**Design.**
- `Map.parallax: Vec<ParallaxLayer { image: String /* project path */, factor: [f32; 2] /*
  0 = fixed to the screen, 1 = moves with the map */, offset: [f32; 2], drift: [f32; 2] /* px per
  second */, repeat: [bool; 2], above: bool, alpha: f32 }>` in the map's RON `Meta` (serde
  default empty).
- Drawing: `render/parallax.rs` (shared with the editor like `render/weather.rs`): layers with
  `above == false` after `clear_background` and before the tile layers, the others after the
  entities and before lighting / weather; source rect from the camera position × factor + drift ×
  time, tiled when repeating. Textures load with the map (`Assets`).
- Editor: *Map › Properties* gets a *Parallax layers* list (image picker, factor, drift, repeat,
  above, alpha, order); the canvas previews them (view toggle `viewopt parallax`).

**Touch points.** `map/mod.rs`, `map/io.rs` (Meta), `render/{parallax (new),mod}.rs`,
`runtime/mod.rs` (draw order), editor `app.rs` (map properties), `canvas.rs`, `main.rs`
(`viewopt parallax`), `refactor.rs` / `library.rs` (image refs travel with maps), README.

**Example.** Cloud shadows drifting over the Wilds (above, alpha 0.15) and a dark mist behind
the mine, seen under empty tiles and beyond the map edge (behind, factor 0.3).

**Verify.** Autoplay screenshots at two player positions show the clouds moving slower than the
ground; a map without layers draws as before; editor screenshot with the preview; map round trip
test.

### [ ] Gapless music loops — `feature/gapless-music`

**Goal.** Music loops without the short silence at the loop point.

**Design.**
- Cause (found with the rain): quad-snd 0.2.8's mixer, after rewinding a looped sound inside a
  mix buffer, writes the rest of the buffer from index 0 again, so a loop not ending on a
  buffer boundary gets an overlap and a silent tail. The rain avoids it with a length of whole
  `MIX_BUFFER_FRAMES` (4096) at 44.1 kHz.
- Fix for music: sounds played with `play_music` are decoded by the engine (Ogg Vorbis with
  `lewton`, already compiled for audrey, as a direct dependency; WAV with a small reader),
  resampled to 44.1 kHz, and stretched to the next whole number of mix buffers (≤ 93 ms more:
  a pitch change below 0.1 % for tracks over a minute; tracks under 20 s are padded with a
  crossfade of their start instead), then handed to macroquad as WAV (`wav_bytes`). Done at load
  for sounds used as music by maps or *Play music* actions, lazily (once) for others.
- Should macroquad ship a fixed quad-snd, the workaround is skipped (keep the replay test of
  the mixer, `rain_loop_survives_quad_snds_mixer`, for music lengths).

**Touch points.** `audio.rs`, `engine/Cargo.toml` (`lewton`), README (audio notes).

**Example.** The two example tracks loop cleanly (they are generated: also render them at whole
buffer lengths in `gen_example/music.rs`).

**Verify.** Unit tests: decoded and stretched lengths are whole buffers; the mixer replay finds
no gap for a music-length sound. Listening check by the user (unmuted run); underrun grep as for
the rain.

## 0.9.0 — Everywhere (platforms and reach)

Theme: rust2d and the games made with it run where the players and creators are: Windows,
macOS, the web, and in their language. Postponed from 0.6.0 (2026-11-06) so the gameplay
releases come first; the design notes are unchanged.

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

- Code signing and notarization of the Windows and macOS packages (today: unsigned, documented).
- Gamepad button glyphs in the HUD and hints (Steam Deck); rebinding gamepad buttons.
- Companions learn spells from scrolls and books given to them and cast heal / bolt spells by
  the rules of *Companion inventory* (`Companion.spells`, a mana pool from the definition).
- Activities that produce something: the fishing NPC sells fresh trout at a market stall the
  next morning (a trigger at the end of an *NPC routines* activity).
- **Swimming and ladders** (moved out of 0.5.0 and 0.8.0; design notes kept, branch
  `feature/swim-climb`): water the player can swim across and ladder tiles to climb.
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
    once), no interacting with props, no fishing, no sitting; the paperdoll is drawn with the
    lower 40 % of the frame clipped (`draw_frame_part_in`, as the seated pose clips) and a 2 px
    bob, a blue tint; a splash `effect` on entering; `status()` `swimming=true`. Leaving water
    restores everything. Companions swim along (same rule for them).
  - Ladders: on a ladder tile only Up / Down move (× 0.6 speed), the player faces Up (climbing
    pose = the Up row with the walk cycle), interact is allowed (so a ladder top can be a portal
    zone, which is how "climbing down into the mine" works). `status()` `climbing=true`.
  - Save games don't change (position is enough). Old projects: no water / ladder tiles → nothing
    changes.

  **Touch points.** `defs.rs`, `render/mod.rs` (`TilesetTex` flags), `runtime/{mod,paperdoll,
  companions,minigame,furniture}.rs`, `editor/src/database.rs` (tileset grid modes) and
  `canvas.rs` (overlay colours), gen_example (mark the village pond and the Wilds' lakes
  swimmable, keep the forest stream solid; a ladder tile in the dungeon tileset and a ladder in
  the mine leading to a lower shaft via portals), README.

  **Example.** The village pond becomes swimmable (its fishing spot covers the water: fishing is
  from the shore, not while swimming); the mine gets a ladder down to a small lower shaft with the
  pick-only chest moved there.

  **Verify.** Autoplay: walk into the pond, `status` `swimming=true` and the tile is water, speed
  halved (tiles per second from two statuses), `press x` doesn't attack, screenshot of the
  half-submerged hero; the forest stream still blocks; in the mine `hold up` on the ladder
  climbs (`climbing=true`), `hold left` doesn't move; an NPC never enters water (`ai=` positions).

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
