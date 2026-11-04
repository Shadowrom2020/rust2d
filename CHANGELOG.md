# Changelog

All notable changes to rust2d (engine, game runner and editor). Versions follow
`MAJOR.MINOR.PATCH`; see AGENTS.md for how releases are made.

## Unreleased

### Editor
- **Removing assets from the library asks for confirmation** first. The dialog lists the assets
  and says up front when other library assets still use them (removal deletes their files and
  can't be undone).

## 0.0.6 — 2026-11-04

### Gameplay
- **Loot tables**: defeated characters drop items. Each line of a table is an item, a chance in
  percent and an amount (min–max). Character definitions have a default table, placed characters
  can use their own, and enemy spawners pass their table on to the characters they spawn. Drops
  land where the character fell and stay (also in save games) until picked up.

### Editor
- **Unsaved markers follow undo**: maps, definitions, scripts and the pixel editor only count as
  changed (• marker, save prompt on switching maps or closing) while they really differ from the
  saved files. Undoing back to the saved state, or editing a field and changing it back, clears
  the marker again.
- **Mouse-wheel scrolling** in panels, lists, the inspector and the script editor moves about three
  lines per notch (it was barely half a pixel).

### Fixes
- Dropped items and spawned characters could get the same internal id (both started at
  0x4000_0000), which could mix them up. Dropped items now use their own range; piles in saves
  from 0.0.5 are renumbered when loaded.

### Example project
- Slimes drop coins and apples, goblin archers arrows and coins (replacing the scripted rewards).
  In the forest, the slime den adds potions and the goblin camp has its own table: arrows, coins,
  bolts, sometimes a leather cap and rarely a crossbow.

## 0.0.5 — 2026-11-04

0.0.4 was prepared but never released; its changes are part of this release.

### Player character
- **Character creator** after *New game*: the player chooses a name, male / female and a look
  (animated preview). Name, sex and look are kept in save games and used for the player's sprite.
- Games can **lock the player's sex** (*Player chooses* / *Always male* / *Always female*,
  Database → Project → Character creator).
- Dialogs, choices and quest texts can **address the player**: `{player}` for the name,
  `{sir|madam}` for male / female forms. New *Player sex* condition; scripts have
  `player_name()`, `player_sex()`, `is_female()`, `is_male()` and `text()`.

### Gameplay
- **Drop items**: in the inventory, G drops the selected stack (Shift+G one item), or drag a
  stack out of the window. Dropped items stay where they are — also after leaving the map and in
  save games — until picked up again.
- **Patrol routes**: placed characters can walk a route of waypoints (loop or back and forth,
  pause, speed), finding their way around obstacles. They stop briefly when talked to and
  continue where they were after map changes and loading. New *Patrol* action and
  `patrol()` / `stop_patrol()` in scripts.
- `say(SELF, "...")` and a `{self}` speaker show the name of the trigger's owner as speaker.

### Editor
- **New project wizard**: game name, location, player sex, character creator, optional starter
  assets from the library, first map (name and size), pixel scale and inventory size.
- **Script editor**: syntax highlighting, line numbers, auto-completion (engine functions with
  parameters, script functions, keywords, variables — and inside string arguments the project's
  items, maps, spawn points, quests, sounds, prop states, flags, variables, entity names and
  script functions), a parameter hint for the current call, auto-indent, closing brackets and
  quotes, Ctrl+click to jump to a function.
- **Script check**: unknown ids in script calls (e.g. `give_item("potoin")`) are listed under
  *Problems* and underlined in the code.
- **Renames update scripts**: renaming an item, map or sound also rewrites the matching script
  calls (backups as `<file>.rhai.bak`).
- **Patrol routes** are drawn and edited on the map (click to add, drag to move, right-click to
  remove waypoints).
- **Speaker dropdown** in *Dialog* / *Choice* actions with `{self}`, `{player}`, the characters
  on the map and speakers already used — names can still be typed by hand.
- **Trigger lists only offer events that can happen** for their owner (e.g. map triggers:
  *Map enter* and *Custom event*). Older triggers with an impossible event are marked ⚠ with a
  one-click fix.
- Creator looks and character models can be picked from the asset library.

### Example project
- The player creates a character (six looks); NPCs address the player by name and sex.
- A town guard patrols the village road and stops once the shrine is lit.

### Development
- Branching concept in AGENTS.md (feature → develop → release → master) and CLAUDE.md with
  instructions for Claude Code sessions.

## 0.0.3 — 2026-11-03

- **Trading**: merchants (stock, quantities and prices per character definition or placed
  character, currency, sell rate), item values, trade window, *Open shop* action and
  `open_shop()`; merchant stock is saved.
- **Character models**: choose a look from the project or the asset library for a character
  definition or a single placed NPC.
- **Quests**: Database → Quests, *Quest* action and condition, quest script API, quest journal on
  J (attack is X now).
- Example: Tilda's shop (female merchant, counter, sign) and a second quest *Pest Control*; NPCs
  can be talked to across counters.
- Closing the game window asks whether to save first.
- Editor: the Scripts tab lists functions per file and jumps to them, opens the first script and
  saves when switching files.

## 0.0.2 — 2026-11-03

- Larger starter library: desert, mountains, city and dungeon tilesets, more props, male and
  female variants of every humanoid character.
- Database: visual sprite chooser, embedded sprite-sheet pixel editor, create and edit art for
  items, props and characters.
- Maps: Ctrl+M quick switcher, Ctrl+PgUp / PgDn cycling, a camera per map, double-click a portal
  to jump to its target.
- Two-way *map transition* zones drawn with the Transition tool (T), linked across both maps.
- Play-tests warn when `rust2d-game` was built from a different engine.

## 0.0.1 — 2026-11-03

First version: a small 2D engine for top-down RPGs on Linux, written in Rust (macroquad + egui).

- Engine: chunked tile maps up to 1024×1024 (32 px tiles); items, props (with named states),
  characters, tilesets, sprite sheets and sounds; data-driven triggers and actions plus Rhai
  scripting; real-time melee and ranged combat, inventory, equipment, containers, key locks,
  quick bar, save games, title screen and pause menu; asset library and `.r2dpack` packages;
  standalone release bundles; conversion of older 16 px projects.
- Game: `rust2d-game` runs a project or a released bundle.
- Editor: `rust2d-editor` for maps, entities, zones, transitions and definitions, with an asset
  library, pixel editor, new-asset wizard and game export.
- Example project: a short quest with generated art, sounds and music.
