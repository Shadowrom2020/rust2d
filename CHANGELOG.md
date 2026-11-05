# Changelog

All notable changes to rust2d (engine, game runner and editor). Versions follow
`MAJOR.MINOR.PATCH`; see AGENTS.md for how releases are made.

## Unreleased

### Characters and equipment
- **Layered characters**: every character is a body (people: in underwear, with recolorable
  skin and hair, a hair style and a beard; monsters: the whole creature) dressed by what it
  wears. A new **Clothing** equipment slot dresses them (outfit overlays, with a **female cut**
  worn by female bodies), armor is drawn over the clothing, and **weapons, shields and lights
  are visible in the hands** for every facing and walk frame (melee swings sweep the weapon).
  The pre-drawn sheet option is gone: a character of an old project keeps its sheet as its body.
- **NPCs have equipment and carried items** (per definition and per placed NPC): visible, used in
  combat (weapon, armor defense), dropped when defeated; actions *Equip item*, *Unequip slot*,
  *Give item to character*, *Take item from character* and script functions `equip`, `unequip`,
  `give_item_to`, `take_item_from` change them (saved).
- The **character creator** offers skin color, hair style, hair color and, for men, a **beard**
  (lists set in the editor); the pre-drawn looks are gone.
- The player character's equipment and carried items are what a new game starts with; saves from
  before clothing get the player's clothing once.

### Editor
- **Character builder**: characters are assembled visually — a walking preview (facing buttons
  or turning, the four idle directions) beside rows of clickable cards for the body, skin
  swatches, hair style, hair color, beard, clothing (shown worn), head / body armor, weapon,
  left hand, plus the carried items; *Randomize* and *Start from…* (copy the look and equipment
  of a character of the project or the asset library — library sheets and items are brought
  along). It is the *Look & equipment* part of the Characters form (project and library),
  opens as a window for **New character…** (id, name, *Create*) and for a placed NPC's own
  look (*Customize this one…* in the map inspector, *Use the definition's* clears it). Items get
  a *Held sprite* and outfits a *Female cut* sheet, sheets a *Kind* and bodies a *Female body*
  flag, the armor slot *Clothing*; the Project tab sets the creator's bodies and its skin, hair
  style, hair color and beard choices with the same cards. The New-asset wizard no longer makes
  characters. Editor-script commands `charnew`, `charset`, `charrandom`, `chartemplate`,
  `charcreate` / `charapply`, `charcancel`, `charcustom`, `charreset`, `entequip`, `entskin`.

### Example project and starter library
- Every character is a layered model: shared bodies (male, female, goblin, **female goblin**),
  short / long / **ponytail** hair, **full beard and goatee**; outfits as clothing items in both
  cuts; held sprites for sword, spear, axe, bow, crossbow, shield, lantern and torch; slimes,
  skeletons, bats and ghosts are bodies. **Female bodies are more feminine** (narrow waist, bust,
  slim limbs, rounder chin, fitted outfits). The guard holds a spear and wears a helmet, goblin
  archers and the new **goblin scouts** (female, in the wilds) carry a crude bow (and drop it
  with their rags), the villager carries apples, Tilda sells green and red tunics, the creator
  offers 5 skin tones, 4 hair styles, 6 hair colors and 3 beard choices.

### Gameplay
- **Scripted spell effects**: new script functions (also trigger actions) `aim(fn)` (magic
  targeting; the click calls `fn(x, y)`, a spell's cost is paid only then), `fly(sheet, x, y, fn,
  #{ speed, frames, fps, scale, light })` (an animated, glowing sprite flies to a point and calls
  `fn(x, y)`), `effect(sheet, x, y, #{ ... })` (an animation played once, e.g. an explosion) and
  `damage_area(x, y, radius, damage)`. Script ids of sprite sheets in `fly` / `effect` are checked
  and renamed with the sheet.
- **Script light and healing**: `light_at(x, y, radius, seconds, #{ color, flicker })` (a timed
  light at a point), `heal_area(x, y, radius, amount)` (the player and friendly characters),
  `player_light(radius, seconds, #{ color, flicker })`, and `player_x()`, `player_y()`,
  `player_facing()`, `player_max_hp()`. `player_light` and `damage_around` accept integers and
  decimals in any mix (before, `player_light(5, 30.5)` failed).

### Asset library
- **Scripted skills and spells in the library and packages**: *Add to library* / *Export…* of
  assets that use scripts open a *Script functions* dialog; the functions the assets need
  (and what those call or pass as callbacks) are ticked, other functions only go along when
  ticked. Functions travel in packages and the library (`scripts/library.rhai`), are appended to
  a project's `scripts/library.rhai` when it takes the asset (existing functions are kept), follow
  id renames, and are pruned when the library no longer needs them. Assets the functions use by
  id (sprite sheets, sounds, items) come along. The starter library now has the Fireball spell
  and the Scavenging skill with their functions.

- Fix: boulders showed a stray dark mark at their upper left (the open double gate's post cap
  spilled into the boulder's cells on the props sheet).
- Fix: spell, skill, scroll and book icons were blank in libraries created before those icons
  existed (the library kept its older, narrower `items` sheet image). Starter updates now replace
  an existing sprite sheet or tileset image when the new one only adds frames and leaves the
  existing part unchanged; images the user edited stay as they are. Existing libraries apply
  this once on the next start.

### Documentation
- `Documentation/scripting.md`: the complete scripting guide and API reference (how scripts
  are run, immediate vs. queued functions, state, targets, units, every function, recipes,
  limits, a Rhai primer). Tests keep it complete and its examples compiling.
- `wait()` accepts whole numbers too (`wait(1)` used to be a script error).

### Editor
- Trigger actions *Aim (then run script)*, *Fly sprite to point*, *Play effect*, *Damage area*,
  *Light at point*, *Heal area*; *Light around player* gained color and flicker. The script editor
  completes all new functions (sprite sheets for `fly` / `effect`), and `queue_set_var`, which was
  missing; a test now fails when an engine function is missing from the completion table.
- Script editor: the parameter hint also works for the project's own script functions, and right
  after `(` or `,` an id argument (item, map, spawn point, sheet, script function, ...) opens its
  suggestions at once, inserted with quotes.

### Example project
- **Fireball** spell (Tome of the Fireball, sold by Tilda; needs Arcana 3): click a target, a
  fireball flies there and explodes, damaging everything within 2 tiles; embers glow on briefly. New sprite sheets
  `fireball` and `explosion` (also in the starter library) and sounds `fireball`, `explosion`.

## 0.0.8 — 2026-11-04

### Gameplay
- **Skills and magic**, both optional per project (Database › Project › Skills & magic). Skills
  (Database › Skills) have a max level and an effect per level: melee / ranged / spell damage, defense, speed, lockpicking
  (locks get a pick difficulty), haggling, max mana, mana regeneration, none, or a script function
  `name(level)`. Skill points come from the new *Give skill points* action / `give_skill_points()`
  and are spent on the Skills screen (K). Magic adds mana (bar under the hearts, regenerates, mana
  potions via *Add mana*) and spells (Database › Spells: cost, cooldown, required skill, effect:
  heal, bolt, blast, light or script) learned from items that *teach* them (scrolls, books) or the
  *Learn spell* action, cast from the Magic tab or the quick bar. New conditions *Skill level*,
  *Knows spell*; actions *Set skill level*, *Learn spell*, *Damage around player*, *Light around
  player*; script functions `skill`, `skill_points`, `knows_spell`, `mana`, `set_skill`,
  `give_skill_points`, `learn_spell`, `add_mana`, `damage_around`, `player_light`.
  Bolt spells are aimed with the mouse (magic crosshair; click an enemy to fire, X straight
  ahead, right-click / Esc cancels).

- **Options menu** on the title screen and in the pause menu: fullscreen, zoom, show FPS,
  master / music / effects volume and dialog text speed (slow, normal, fast, instant), with a
  reset to defaults. Settings apply immediately and are stored per game in
  `~/.config/rust2d/<game>/settings.ron` (XDG config folder; `RUST2D_SETTINGS_DIR` overrides it).
- **Map screen** (M or the new map button bottom right): the current map from above with the
  player's position and facing and labelled exits to other maps; pauses the game while open.
  Scroll with the arrow keys / WASD, by dragging the map or the scrollbars, or the mouse wheel;
  zoom with + / -, Space centers on the player.
- **Fog of war** per map (map inspector › Map screen): the map screen only shows the explored
  area (about 6 tiles around everywhere the player has been); kept per map in the save game.

### Fixes
- Changing a map's music or lighting in the inspector now marks the map as changed and can
  be undone (before, only saving together with another change kept it).
- Loading a saved game no longer runs the map's *Map enter* triggers again (a load resumes the
  game; the player didn't enter the map).

### Example project
- Fog of war on the forest, the wilds and the mine; the village and its buildings have none.
- Eight skills (Swordsmanship, Archery, Toughness, Lockpicking, Haggling, Arcana, Spellcraft and the
  scripted Scavenging),
  four spells (Heal, Light, Spark, Fire Burst), scrolls, spell books and blue mana potions; quests
  reward skill points; the forest chest can be picked, the mine's chest only picked.

## 0.0.7 — 2026-11-04

### Gameplay
- **Lighting, day and night**: maps are *Always bright* (default), *Outdoor* (day and night with
  warm dusk and dawn; darker at night) or *Dark* (dungeons, caves: pitch black at 100 %). Light
  sources — tiles (wall torches, lamps, windows), props per state, characters, items (equipped or
  on the ground) and single placed entities — light soft circles with their own radius, color,
  strength and optional flicker. The clock runs while playing (day length and start hour in the
  project), is saved, shown top right on outdoor maps and available to events (*Set time of day*,
  *Time of day* condition) and scripts (`hour()`, `is_night()`, `set_hour()`).
- **Left hand slot**: a fourth equipment slot for a shield (defense) or a light (lantern, torch).
  Weapons can be *two-handed*; equipping one puts the left-hand item away and vice versa.

### Editor
- **Icons, ribbon and grouped menus**: the editor uses the Phosphor icon font. Map mode has a
  tool ribbon under the menu (Edit · Select · Paint · Collision · Objects · Layer · View · Play)
  with icon buttons whose tooltips give name, shortcut and a one-line how-to; the status bar shows
  the active tool's how-to; the left panel holds the tool's options with the layers underneath.
  Menus are grouped by task (File, Edit, Map, Assets, View, Play). Database tabs, library kinds,
  the pixel editor's tools and all action buttons have matching icons (no more box glyphs).
- **Structures**: multi-tile assets placed with one click — houses, wall and fence segments,
  rooms, ponds, groves, a plaza and more. New *Structures* map tool (H) with a categorized
  palette, a preview that follows the mouse and one undo step per placement. A structure sets
  tiles on all layers, collision overrides (walkable doors) and places its entities (signs,
  barrels, chests). *📐 New from map area…* turns a rectangle of the map into a new structure;
  *Database → Structures* lists them, with a **structure editor**: assemble a structure from
  tileset stamps (drag-select tiles in a tileset, paint them on the ground / detail / overhead
  layer), mark cells that clear the map's tile, set collision, place props, resize, undo / redo.
  *✨ New asset… → Structure* (empty grid or a copy), *🏠 New structure* in the Asset Library
  window and *➕ New…* in the palette start one. The editor has grouped tools on the left, a
  layer stack (Overhead / Detail / Ground with descriptions, show / hide and what each holds),
  zoom, and a tileset with stamp preview or a prop gallery on the right. Ctrl+Z / Ctrl+Y undo
  and redo in it. Structures live in `defs/structures.ron`, can
  be added from / to the asset library and exported in packages. Games need nothing new: placing
  writes ordinary tiles and entities.
- **Asset Library window**: the library opens in its own window (top bar *📂 Asset Library ↗*,
  or `rust2d-editor --library`). *Browse* shows all library assets with import, export and
  remove; *Edit* is the Database editor for library assets; the pixel editor and the new-asset
  wizard create and change library assets in place. Changes the main editor makes to the library
  are picked up automatically.
- **📥 Add from library…** dialogs in the Database tabs, next to the tileset and placement
  palettes and in the File menu: show the library's tilesets (or props, characters, items, sprite
  sheets, sounds) as thumbnails, pick one or more and add them with everything they need. The old
  *Library* mode of the main editor is replaced by these.
- **Removing assets from the library asks for confirmation** first. The dialog lists the assets
  and says up front when other library assets still use them (removal deletes their files and
  can't be undone).
- **Pixel editor: Ctrl+C / Ctrl+X / Ctrl+V** copy, cut and paste the selected frame (cut + paste
  moves a frame). Pasting now makes the frame exactly the copied one (transparent pixels
  included) instead of drawing the copy over the old content.
- **Pixel editor: *Select frame* is the default tool** (shortcut S): clicking into the image
  selects the frame under the cursor instead of drawing; pick the pencil (B) or another tool to
  draw.

### Example project
- Day and night in the village, forest and wilds: lit windows, the guard's lantern, the shrine's
  glow. Tilda sells a lantern, torches and a wooden shield; spear, bow and crossbow are
  two-handed. A pitch-dark old mine in the forest with wall torches, slimes and a treasure room.
- 18 structures from the terrain tileset (cottages, shop front, stone walls, fences, a walled
  yard, a fenced garden, bedroom / storage / small rooms, pond, bridge, well square, groves,
  rocks). The starter library adds 16 more from the city and dungeon tilesets and its props.

### Fixes
- Slimes showed a thin line of their own body above them when facing left, right or up (the
  sprite's bottom spilled into the next row of the sheet). Erasing in the art generator never
  worked either: key rings, the leather cap's opening and similar cut-outs are now transparent.

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
