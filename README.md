# rust2d

A small 2D engine for top-down RPGs on Linux, written in Rust ([macroquad](https://macroquad.rs) + [egui](https://github.com/emilk/egui)), with a map/content editor.

- **Tile maps** up to 1024×1024 tiles, any number of layers (overhead layers draw above characters), stored chunked and sparse.
- **Editor** for maps (brush/stamp, rectangle, flood fill, erase, eyedropper, collision painting, undo/redo), entities, zones and transitions, plus a database editor for items, props, characters, tilesets, sprite sheets, project settings and scripts.
- **Map transitions**: portal zones (teleport to a named spawn on any map) and edge links (walk off the north/south/east/west edge into a neighbouring map, keeping your position along the edge).
- **Event system**: triggers (`Interact`, `Enter`, `Leave`, `UseItem`, `MapEnter`, `Defeated`, custom events) with conditions and action lists, authored in the editor, plus **Rhai** scripts for anything more complex.
- **Quick action bar**: 8 shortcut slots at the bottom of the screen. Drag items (or equipped gear) from the inventory onto it, or select an item and press 1–8; then press 1–8 or click a slot to use the item (drink potions, eat, equip weapons/armor, use keys). Slots show how many you own and grey out at 0; the bar is saved.
- **Multi-tile props**: a prop's sprite can be a block of several cells of its sheet (e.g. a 2×2 double gate, a 3×4 oak), with its own collision box (e.g. only the trunk). Y-sorting lets the player walk behind big crowns. Locks without a key item are doors/gates that simply open when used.
- **Key locks**: any prop (gates, doors, chests) can need a key item. Interacting checks the inventory for the key, uses it up (optional) and unlocks: chests then open, gates disappear or switch to another state (e.g. "open", which doesn't block movement). Locked props within reach show a padlock; unlocked state is saved.
- **Character sheet & equipment**: head, body and weapon slots next to the inventory grid, with a portrait wearing the equipped armor and live stats (attack, defense, speed, attack time, reach/range). Armor reduces damage (`max(1, damage − defense)`) and can slow you down; armor sprites are drawn over the player.
- **Real-time combat**: hearts (1 heart = 2 HP), **melee weapons** with their own damage, speed, reach, swing arc and knockback (sword, spear, axe) and **ranged weapons** (bow, crossbow, with ammo), knockback and invulnerability frames, hostile characters that chase or shoot at the player, `Defeated` triggers, healing items, game over → reload.
- **Inventory & containers**: slot-based inventory (size set per project, stack size per item) with a grid window (drag to rearrange, double-click to use/equip). Chests, barrels etc. are lootable containers: a window shows the container next to your inventory and you choose what to take (or store things in it). Size, starting contents, lock conditions and an "opened" sprite frame are set in the editor; contents are saved.
- **Mouse controls**: left-click a tile to walk there (A* pathfinding), left-click an enemy to attack it, right-click an NPC/item/prop to walk over and interact; dialogs, choices and the inventory are clickable.
- **Enemy spawners**: invisible zones placed in the editor that keep a group of characters alive, with a respawn mode per spawner: never (defeated enemies stay dead, saved), every time the map is entered, or after N seconds.
- **Audio**: sound effects and per-map looping music (WAV/OGG), built-in sounds for pickups, attacks, hits, dialogs and doors.
- **Animated tiles**: per-tileset frame animations (e.g. water), shown in the game and the editor.
- **Safe renaming**: renaming an item, prop, character, tileset, sprite sheet, sound or map updates every reference in the project.

## Quick start

```sh
cargo run -p rust2d-engine --example gen_example   # (re)generate example_project/
cargo build --workspace                            # builds game + editor
cargo run -p rust2d-game                           # play example_project
cargo run -p rust2d-editor                         # editor (welcome screen)
cargo run -p rust2d-editor -- example_project      # edit the example directly
```

Both binaries take the project folder as first argument. The game defaults to `example_project`; the editor without a folder starts on a welcome screen (*New project…*, *Open project…*, recent projects). *File → New / Open / Close project* first asks to save or discard unsaved map, definition and pixel-editor changes (Cancel keeps everything as it is; if saving fails nothing is closed), then replaces the whole editor state. New projects need an empty folder and use 32 px tiles like the asset library. The game starts on a title screen (New game, Continue from the latest save, Load game, Exit). It also accepts `--map NAME`, `--spawn NAME` and `--pos X,Y` (start right there, used by the editor's play-test) and `--no-title` (start a new game immediately).

The example project contains a short quest: talk to the elder, get the key from the chest in his house (a bow and arrows lie there too), unlock the walled-in east gate (a vertical double gate; you keep the key and can lock it again behind you), follow the forest path (past slimes and goblin archers; a crossbow waits in the clearing) to the shrine. A cave in the forest leads to *The Wilds*, a procedurally generated 1024×1024 map with 40 enemy spawners. All art, sound effects and music in it are generated by the example program (`crates/engine/examples/gen_example/`; the pixel art is in `art.rs`): 32×32 px tiles and 32×48 px characters, shown at a pixel scale of 1.5. Tiles are always 32 px (`project::TILE_SIZE`); sprites, hitboxes and speeds are designed for that size.

**Older 16 px projects** are converted automatically when the editor opens them (a full backup `<project>-backup-16px` is made first). Images that are shrunken copies of asset-library art are replaced by the library originals, other images are enlarged without loss; frame sizes, hitboxes, speeds, entity positions and save games are scaled. Maps that were painted with full-size library tiles as 2×2 blocks of quarter tiles get a grid half as big showing the same picture. The game refuses unconverted projects with a hint to open them in the editor.

### Game controls

| Key | Action |
|---|---|
| Arrows / WASD (Shift = run) | Move |
| E / Space / Enter | Interact, advance dialog |
| J / X | Attack in the facing direction |
| Q | Switch weapon (bare hands → owned weapons) |
| 1–8 / click a quick bar slot | Use that item (inventory open: 1–8 puts the selected item on the bar; drag shortcuts to rearrange, drag off or right-click to remove) |
| C | Character sheet + inventory (same window as I) |
| Left click | Walk to the tile / attack the enemy under the cursor / advance dialog |
| Right click | Walk to and interact with the NPC, item or prop under the cursor (or zones on that tile) |
| I / Tab / C / bag button | Character sheet + inventory: arrows/click select, Enter / double-click / right-click use or equip, drag items onto / off the equipment slots or to rearrange, left arrow from the first column moves to the equipment slots (Enter unequips), Esc close |
| In a loot window | Click a stack to take it (right-click: take one), click your own stack to store it, drag between grids, T take all, Tab switch side, Esc close |
| Esc | Pause menu: Resume, Save game (new named save or overwrite), Load game (all saves incl. the quicksave, newest first; Del deletes), Save and quit (saves to the slot this game was last saved to / loaded from, or a new "Save N", then returns to the main menu), Quit (back to the main menu without saving, after a confirmation). Only *Exit* on the main menu closes the game. |
| F5 / F9 | Quick save / quick load (`<project>/saves/quick.ron`) |
| F3 | Debug overlay (collision, zones, edge strips, hitboxes) |

### Editor basics

| Input | Action |
|---|---|
| Mouse wheel / middle-drag / Space+drag / arrow keys | Zoom / pan |
| V B R F E I C N Z | Select (default), Brush, Rect, Fill, Erase, Pick, Collision, Entities, Zones |
| Right-click (tile tools) | Pick the tile under the cursor |
| 1–9 | Select layer |
| Ctrl+Z / Ctrl+Y | Undo / redo |
| Ctrl+S | Save map and definitions |
| Ctrl+D / Del | Duplicate / delete the selected entity or zone |
| Esc | Deselect; stop placing (Entities); otherwise back to the Select tool |
| F5 / F6 | Play-test from the camera position (or the selected spawn) / from the project start |
| G, Home | Toggle grid, fit map to the view |

- **Select** (default tool): click an entity or zone to select it (an entity is found by its sprite or by the tile it stands on), drag to move it, click again to cycle through objects stacked on the same spot; empty space or right-click deselects. It never paints — picking a tile in the palette switches to the Brush.
- **Tiles**: drag in the tileset palette to select a multi-tile stamp. Shift+drag with Rect erases.
- **Collision**: left paints "force solid", right paints "force walkable", Shift+left resets to the tileset default. Which tiles are solid by default is set per tileset in *Database → Tilesets*.
- **Entities**: pick a prop, character or item in the left panel and click empty space to place it (snapped to the tile; hold Shift for free placement). **Right-click or Esc stops placing** (clicking the armed entry again does too); with nothing armed, clicks only select / move. The palette has a search box (name or id), *All / Prop / Character / Item* tabs, a *Recent* row of the last used definitions, and `[` / `]` or PgUp / PgDn step to the previous / next definition. *On this map* lists the placed entities (filtered by the same search) — click one to select it and jump there. Select an entity to edit its name, state and triggers on the right; *Place more like this* arms its definition.
- **Zones**: drag to create a spawn point, portal, trigger area or **enemy spawner**. Portals pick their target map and spawn from dropdowns; spawners pick a character, how many, and the respawn mode.
- **Edge transitions**, **music**, **map triggers** and **Rename map** are in the right panel when nothing is selected.
- **Files**: *File → New project… / Open project…* use the desktop's native dialogs (xdg-desktop-portal). Tilesets, sprite sheets and sounds have *Browse…* buttons; files from outside the project are copied into `tilesets/`, `sprites/` or `sounds/`.
- **Big props**: in *Database → Props*, drag over several cells in the sheet grid of the sprite picker (or set *Size (cells)*). Placement snaps by footprint (even widths center on a tile edge), a translucent ghost previews the prop, and the selected prop shows its collision box.
- **Locks**: in *Database → Props*, tick *Lockable* to give every instance of a prop a default lock (key item, whether the key *is used up* or *stays in the inventory*, what happens when unlocked, locked / unlock messages). *Can be locked again with the key* lets the player relock an opened gate / door by interacting with it (or using the key on it; containers only by using the key) while carrying the key — without a key item this simply closes it again. It can't be closed on somebody standing in it, and isn't available for props that disappear. A placed prop can *Customize lock* (another key), drop the definition's lock, or *Make this lockable* on its own. 'Unlocked' / 'Locked again' triggers and the *Set locked* action (`set_locked(target, b)` in scripts) react to / control locks.
- **Prop states**: in *Database → Props* every prop has a list of **states** — each with a name, its own sprite and whether it blocks movement (e.g. the Iron Gate: *closed*, *open*, *destroyed*; the shrine: *dormant*, *glowing*). The first one is the default; *+ Add state* adds one, ✏ renames it (placed props, the lock / container and "self" triggers follow), *Make default*, ⏶⏷ and ✖ organise them, and the selected state's sprite is picked (and painted with *✏ Edit pixels*) right below the list. A lock can *switch state* when unlocked (with a closed / target preview), a container can switch state when first opened, a placed prop chooses its starting *State* in the map inspector, the **Set prop state** action / `set_state(target, "name")` switch it at runtime and the **Prop state** condition checks it. The current state is saved. Older projects using frame offsets are converted to named states when loaded.
- **Containers**: in *Database → Props*, tick *Lootable container* to give every instance of a prop a default container (slots, starting contents, "opens only if" conditions + locked message, the state to switch to when opened). In the map inspector, a placed prop can *Customize size & contents* (or become a container even if its definition isn't one). Items have a *Stack size*; the player's *Inventory slots* are set in *Database → Project*.
- **Equipment**: in *Database → Items*, choose *Equipment: Weapon / Helmet / Body armor*. Weapons are melee (damage, cooldown, reach, arc, knockback) or ranged (projectile sprite, speed, range, ammo item). Armor has defense, a speed change and an optional overlay sprite sheet (same layout as the player sheet). Characters have a *Defense* value; hostile characters get an optional ranged attack.
- **Database**: every definition has a *Rename…* button next to its id. Renaming rewrites all references (maps on disk and the open map, triggers, other definitions, project settings) and saves immediately. Strings inside Rhai scripts are not rewritten.
- **Tile animation**: in *Database → Tilesets*, add an animation, then click tiles in the image to append frames.
- **Sounds**: *Database → Sounds* lists sound files with volume and a preview button. Built-in game sounds are set in *Database → Project*.

### Releasing a game

*File → 📦 Export game (standalone)…* saves the project and writes a bundle that runs without the editor or Rust:

```
releases/<game-name>/
  <game-name>      the game executable (run it to play)
  data/            project.ron, defs/, maps/, scripts/ and only the images / sounds in use
  README.txt
releases/<game-name>-linux.zip   (optional, to share)
```

The executable is a copy of `rust2d-game`; started without arguments it runs the `data/` folder next to it, titles the window with the game name and keeps save games in `~/.local/share/<game-name>/saves` (the bundle folder may be read-only). Save games of the project are never shipped. The export checks the project first (start map must exist; missing files and an unset player are reported) and replaces an earlier export in the same place, but never touches other folders. Build the game in release mode first (`cargo build --release -p rust2d-game`) — the editor picks `target/release/rust2d-game` when it exists and warns when only a debug build is available. The bundle targets the platform it was built on (Linux x86_64 here) and needs the usual system libraries (OpenGL, X11/Wayland, ALSA).

### Asset library, packages and the pixel editor

- **Library** (top bar → *📂 Library*): a shared collection of tilesets, sprite sheets, sounds, items, props and characters that every project can use. It lives in `~/.local/share/rust2d/library` (or `$XDG_DATA_HOME/rust2d/library`; `$RUST2D_LIBRARY` overrides it) and is filled with the example game's assets the first time it is opened. Click cards to select, filter by kind or search, then:
  - *Add to project* copies the selected assets **and everything they need** (sheets, ammo, keys, overlay sheets, sounds used by triggers) into the project. Assets already in the project with identical content are reused; an id or file name that is taken by something different gets a `_2` suffix and all references follow. Packages made for another tile size (older 16 px exports) are enlarged to 32 px on import: images (nearest neighbour, lossless), frame sizes, hitboxes and speeds.
  - *Export package…* writes a `.r2dpack` file (a zip with `manifest.ron` and the files) to share; *Import package…* adds one to the library; *Remove from library* refuses to remove things other library assets still use.
- **From a project**: *Database* has *📂 Add to library* and *📦 Export…* for the selected definition, and *File → Import asset package into project…* imports a `.r2dpack` directly.
- **New asset…** (*File*, *Database* or *Library*): a wizard for a prop (size in tiles), item, character (4 walk frames × 4 facings), tileset or sprite sheet. Draw a new image, start from a copy of an existing one, or reuse a frame of an existing sheet. New images open in the pixel editor; the definitions are created with sensible hitboxes and speeds for the project's tile size.
- **Pixel editor** (top bar → *✏ Pixel editor*, or *✏ Edit pixels* on a sprite sheet / tileset): pencil (B), eraser (E), fill (G), line (L), rectangle (R) and color picker (I); left / right mouse buttons draw with two colors (X swaps), palette + colors used in the image, wheel zoom, middle-drag pan, pixel and frame grids (character sheets label their rows Down / Left / Right / Up), undo / redo (Ctrl+Z / Ctrl+Y), copy / paste / paste mirrored / flip / clear a frame, resize the canvas in frames, and a live preview of the selected frame and of its row as an animation. Ctrl+S saves the PNG and reloads it everywhere.

## Project layout

```
project.ron              name, start map/spawn, pixel scale, player character (tiles are always 32 px)
defs/tilesets.ron        tileset images + solid tile indices
defs/sheets.ron          sprite sheets (image + frame size)
defs/items.ron           items (icon, description, consumable, on-use triggers)
defs/props.ron           props (sprite, solidity, hitbox, default triggers)
defs/characters.ron      characters (sheet, speed, hitbox, Static/Wander/Chase behaviour, HP/damage/hostile, default triggers)
defs/sounds.ron          sounds (file, volume)
sounds/                  WAV / OGG files
maps/<name>.map          binary map, see below
scripts/*.rhai           Rhai functions, all files share one namespace
saves/                   save games (RON)
```

Map files are `"R2DMAP" + version + lz4(postcard(core))`. The core holds the tile layers in compact binary form; entities, zones, edge links, triggers and properties are stored inside it as RON text, so new optional fields don't break existing maps. Version 1 files (before this layout) still load.

Character sheets have 4 rows (down, left, right, up) × *walk frames* columns; column 0 is the idle pose. Entity positions are the feet (bottom-centre) in pixels.

## Events

A **trigger** = *when* (event) + *if* (all conditions) + *do* (actions), optionally `once` per save game.

| Event | Fired on |
|---|---|
| Interact | entity in front of the player, or the zones the player stands in, when pressing E |
| Enter / Leave | trigger zones |
| UseItem (item or any) | entity in front of the player; if nothing reacts, the item's own `on_use` triggers |
| MapEnter | the map, after loading |
| Custom (name) | map, all zones and all active entities, raised by the `Emit` action or `emit()` |
| Defeated | a character whose HP reached 0 |
| Unlocked | a prop whose key lock was opened |

Entities run their definition's triggers plus their own. Picking up an item entity is automatic on Interact.

**Conditions:** flag set, variable comparison, has item (count), player HP comparison, prop state, script function returning `bool`; each can be negated.

**Actions:** Dialog, Choice (stores the chosen index in a variable), Give/Take item, Set flag, Set variable (= or +=), Teleport (map + spawn), Set active (show/hide an entity), Set prop state (e.g. open / destroyed), Set frame (old frame offset), Move (walk by tiles, blocking), Face, Wait, Emit event, Run script, If/Else, Play sound, Play music, Heal player (negative = damage), Set locked. Action targets are `self` (the trigger's owner), `player`, or an entity name. Entity state changes persist across map changes and saves.

### Scripting (Rhai)

```rust
fn shrine_touch() {
    if !flag("quest_accepted") { say("It seems dormant."); return; }
    say("Ancient runes begin to glow!");
    set_flag("shrine_lit", true);
    set_state("self", "glowing");
    emit("shrine_lit");
}
```

- World state (immediate): `flag(n)`, `set_flag(n, b)`, `get_var(n)`, `set_var(n, v)`, `add_var(n, v)`, `item_count(id)`, `has_item(id)`, `give_item(id, n)`, `take_item(id, n)`, `current_map()`.
- Queued actions (run after the function returns, in order): `say(text)`, `say(speaker, text)`, `choice(text, [options], var)`, `teleport(map, spawn)`, `set_active(target, b)`, `set_state(target, name)`, `set_frame(target, f)` (old), `move_by(target, dx, dy)`, `face(target, "up"|"down"|"left"|"right")`, `wait(secs)`, `emit(event)`, `then_call(fn)`, `heal(n)`, `play_sound(id)`, `play_music(id)`, `set_locked(target, b)`. Use `then_call` to continue after a dialog or choice.
- Other: `player_hp()`, `random(n)` (0..n).
- `SELF` is the owner entity's uid (or -1). Scripts are sandboxed (no file access, operation limit); errors are logged and never crash the game.

## Architecture

```
crates/engine   library: map model + IO, defs, events, scripting, rendering, runtime
crates/game     player binary (rust2d-game)
crates/editor   editor binary (rust2d-editor)
```

- `map/`: `TileLayer` stores 32×32 chunks as `Vec<Option<Box<Chunk>>>`, so empty areas cost nothing. Collision is a per-tile override byte combined with the tileset's solid flags. `transition.rs` holds zones, edge links and the edge-crossing maths.
- `events/`: `Trigger`/`Condition`/`Action` data types and the `ActionRunner`, a queue that executes actions in order and pauses on blocking ones (dialogs, waits, movement, teleports). The game implements `ActionHost` for world-specific actions.
- `script.rs`: Rhai host. Script functions change world state directly and queue `Action`s, so scripted and editor-authored logic share one execution path.
- `render/`: draws only the visible tiles (macroquad batches quads per texture). When zoomed far out, a one-pixel-per-tile overview texture is drawn instead, which keeps 1024×1024 maps smooth in the editor.
- `runtime/`: movement with axis-separated AABB collision, wandering/chasing NPCs, zone enter/leave tracking, edge and portal transitions with fades, a small LRU cache of visited maps, dialog/inventory UI, save/load. `combat.rs` holds melee/ranged attacks, projectiles, enemy shooters, the HUD and game over; `mouse.rs` click-to-move/act orders; `spawner.rs` spawner zones.
- `path.rs`: A* on the tile grid (8 directions, no corner cutting, node budget so clicks stay cheap on 1024×1024 maps).
- `audio.rs`: sound loading/playback on top of macroquad (files are validated first so a broken sound can't crash the game) and a small WAV encoder used by the example generator.
- `refactor.rs`: rewrites id references in definitions, config and maps for renames.
- `migrate.rs`: converts projects saved with another tile size to 32 px (see above).
- `release.rs`: standalone bundles (project check, copying the files in use, zip) and finding the bundled `data/` folder at game start.
- `library.rs`: asset bundles (dependency collection, `.r2dpack` packages, enlarging old 16 px packages, conflict-aware merging) and the shared library folder; `imageio.rs`: PNG decode / encode for it and the editor's pixel editor. The editor embeds `crates/editor/assets/starter.r2dpack`, which `gen_example` builds from the example project.

## Testing

```sh
cargo test --workspace          # map chunks, 1024² save/load, v1 map migration, edge maths, triggers, runner,
                                # Rhai API, renames, tile animation, WAV encoding, pathfinding,
                                # editor undo/fill, asset import, new projects
cargo clippy --workspace --all-targets
```

For automated visual checks, both binaries accept a debug script through an environment variable:

```sh
# game: hold/press keys, wait, take screenshots, print state
RUST2D_AUTOPLAY="press enter; wait 3; press e; hold right 2; click 30 16; clickenemy; status; spawners; shot /tmp/a.png; quit" ./target/debug/rust2d-game
RUST2D_SHOT=/tmp/a.png ./target/debug/rust2d-game        # screenshot after 1s, then exit
RUST2D_MUTE=1 ./target/debug/rust2d-game                  # no audio

# editor: tools and mouse clicks/drags in tile coordinates
RUST2D_EDITOR_SCRIPT="tool brush; brush 2 0 1 1; drag 2 2 20 2; dump 0 10 2; rename item key gate_key; shot /tmp/e.png; quit" ./target/debug/rust2d-editor
```

## Limitations / ideas

- Items can't be dropped onto the ground from the inventory yet (store them in a container instead).
- Strings inside Rhai scripts are not updated by renames (the editor says so when it matters).
- Enemies placed directly on a map stay dead once defeated; use a spawner zone for respawning enemies.
- Native file dialogs need a running xdg-desktop-portal (standard on GNOME/KDE desktops); paths can always be typed by hand.
- Patrol routes are scripted with `Move` actions rather than being a built-in behaviour.
