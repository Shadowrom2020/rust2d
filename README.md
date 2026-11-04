# rust2d

A small 2D engine for top-down RPGs on Linux, written in Rust ([macroquad](https://macroquad.rs) + [egui](https://github.com/emilk/egui)), with a map/content editor.

Current version: **0.0.7** — see [CHANGELOG.md](CHANGELOG.md) for what changed in each release.

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

Both binaries take the project folder as first argument. The game defaults to `example_project`; the editor without a folder starts on a welcome screen (*New project…*, *Open project…*, recent projects). *File → New / Open / Close project* first asks to save or discard unsaved map, definition and pixel-editor changes (Cancel keeps everything as it is; if saving fails nothing is closed), then replaces the whole editor state. Changes count as unsaved (• next to the map name, *defs •*, *• edited*) only while something really differs from the saved files: undoing back to the saved state — or typing and undoing in a field, toggling a checkbox back — makes them clean again. New projects need an empty folder and use 32 px tiles like the asset library. The game starts on a title screen (New game, Continue from the latest save, Load game, Exit). It also accepts `--map NAME`, `--spawn NAME` and `--pos X,Y` (start right there, used by the editor's play-test) and `--no-title` (start a new game immediately).

The example project contains a short quest: talk to the elder, get the key from the chest in his house (a bow and arrows lie there too), unlock the walled-in east gate (a vertical double gate; you keep the key and can lock it again behind you), follow the forest path (past slimes and goblin archers; a crossbow waits in the clearing) to the shrine. A cave in the forest leads to *The Wilds*, a procedurally generated 1024×1024 map with 40 enemy spawners. All art, sound effects and music in it are generated by the example program (`crates/engine/examples/gen_example/`; the pixel art is in `art.rs`): 32×32 px tiles and 32×48 px characters, shown at a pixel scale of 1.5. Tiles are always 32 px (`project::TILE_SIZE`); sprites, hitboxes and speeds are designed for that size.

**Older 16 px projects** are converted automatically when the editor opens them (a full backup `<project>-backup-16px` is made first). Images that are shrunken copies of asset-library art are replaced by the library originals, other images are enlarged without loss; frame sizes, hitboxes, speeds, entity positions and save games are scaled. Maps that were painted with full-size library tiles as 2×2 blocks of quarter tiles get a grid half as big showing the same picture. The game refuses unconverted projects with a hint to open them in the editor.

### Game controls

| Key | Action |
|---|---|
| Arrows / WASD (Shift = run) | Move |
| E / Space / Enter | Interact, advance dialog |
| X | Attack in the facing direction |
| K | Skills & Magic (if the game has them): *Skills* tab — Up/Down select, Enter or the + button spends a skill point; *Magic* tab — Enter casts the selected spell, 1–8 puts it on the quick bar; Left/Right switch tabs, K / Esc close |
| J | Quest journal: active quests (newest first) and finished ones; the selected quest shows its description and the objectives reached so far (Up/Down or click select, J / Esc close) |
| Q | Switch weapon (bare hands → owned weapons) |
| 1–8 / click a quick bar slot | Use that item (inventory open: 1–8 puts the selected item on the bar; drag shortcuts to rearrange, drag off or right-click to remove) |
| C | Character sheet + inventory (same window as I) |
| Left click | Walk to the tile / attack the enemy under the cursor / advance dialog |
| Right click | Walk to and interact with the NPC, item or prop under the cursor (or zones on that tile). NPCs behind a counter (one solid tile between, in a straight line) are talked to across it — also with E. |
| I / Tab / C / buttons bottom right (bag = inventory, person = character sheet, star = skills & magic) | Character sheet + inventory: arrows/click select, Enter / double-click / right-click use or equip, **G drops the selected stack on the ground (Shift+G: one item), dragging a stack out of the window drops it too** — dropped items stay where they are (also after leaving the map and in save games) until picked up again, drag items onto / off the equipment slots or to rearrange, left arrow from the first column moves to the equipment slots (Enter unequips), Esc close |
| In a shop | Click a *Buy* / *Sell* button (or double-click a row, or Enter / E on the selected row) to trade one, hold Shift for ten; arrows / W S select, Tab / ← → switch between the merchant's stock and your items, mouse wheel scrolls, Esc close |
| In a loot window | Click a stack to take it (right-click: take one), click your own stack to store it, drag between grids, T take all, Tab switch side, Esc close |
| Esc | Pause menu: Resume, Save game (new named save or overwrite), Load game (all saves incl. the quicksave, newest first; Del deletes), Save and quit (saves to the slot this game was last saved to / loaded from, or a new "Save N", then returns to the main menu), Quit (back to the main menu without saving, after a confirmation). Only *Exit* on the main menu closes the game. |
| Window close button | While playing: asks *Save and exit* (to the slot "Save and quit" uses; not possible in the middle of an event), *Exit without saving* or *Cancel*. On the main menu or the game-over screen the game closes right away. |
| F5 / F9 | Quick save / quick load (`<project>/saves/quick.ron`) |
| F3 | Debug overlay (collision, zones, edge strips, hitboxes) |

### Editor basics

| Input | Action |
|---|---|
| Mouse wheel / middle-drag / Space+drag / arrow keys | Zoom / pan |
| V B R F E I C N Z T H | Select (default), Brush, Rect, Fill, Erase, Pick, Collision, Entities, Zones, Transition, Structures |

The **ribbon** under the menu holds the tools as icon buttons in groups (*Edit* undo / redo · *Select* · *Paint* brush, rect, fill, erase, pick · *Collision* · *Objects* entities, zones, transitions, structures · *Layer* · *View* grid, collision overlay, zones, dim layers, minimap, fit · *Play*); hovering any button shows its name, shortcut and a one-line how-to, and the **status bar** repeats the active tool's how-to. The left panel shows the active tool's options (tileset, entity or structure palette) with the map's *Layers* underneath. Menus are grouped by task: *File* (project, save all, export game, quit), *Edit*, *Map* (new / open / save map, fit), *Assets* (new asset, add from library, import package, Asset Library window, save definitions, reload), *View*, *Play*.
| Ctrl+M / Ctrl+PgUp / Ctrl+PgDn | Go to map (search) / previous / next map — the map dropdown in the top bar switches too |
| Right-click (tile tools) | Pick the tile under the cursor |
| 1–9 | Select layer |
| Ctrl+Z / Ctrl+Y | Undo / redo |
| Ctrl+S | Save map and definitions |
| Ctrl+D / Del | Duplicate / delete the selected entity or zone |
| Esc | Deselect; stop placing (Entities, Structures); otherwise back to the Select tool |
| F5 / F6 | Play-test from the camera position (or the selected spawn) / from the project start |
| G, Home | Toggle grid, fit map to the view |

- **Select** (default tool): click an entity or zone to select it (an entity is found by its sprite or by the tile it stands on), drag to move it, click again to cycle through objects stacked on the same spot; empty space or right-click deselects. It never paints — picking a tile in the palette switches to the Brush.
- **Tiles**: drag in the tileset palette to select a multi-tile stamp. Shift+drag with Rect erases.
- **Collision**: left paints "force solid", right paints "force walkable", Shift+left resets to the tileset default. Which tiles are solid by default is set per tileset in *Database → Tilesets*.
- **Entities**: pick a prop, character or item in the left panel and click empty space to place it (snapped to the tile; hold Shift for free placement). **Right-click or Esc stops placing** (clicking the armed entry again does too); with nothing armed, clicks only select / move. The palette has a search box (name or id), *All / Prop / Character / Item* tabs, a *Recent* row of the last used definitions, and `[` / `]` or PgUp / PgDn step to the previous / next definition. *On this map* lists the placed entities (filtered by the same search) — click one to select it and jump there. Select an entity to edit its name, state and triggers on the right; *Place more like this* arms its definition.
- **Structures** (H): multi-tile assets — houses, wall and fence segments, rooms, ponds, groves, a plaza… — placed with **one click**. The palette groups them by category with thumbnails (search box, *Add from library…*); pick one and a transparent preview follows the mouse (centered on the cursor), a click places it, right-click or Esc stops. A structure sets tiles on several layers (ground, detail, overhead), collision overrides (e.g. a walkable door) and places its entities (a shop front's sign and barrel, a storage room's chest); empty upper-layer cells inside it clear what the map had there (no trees growing through a room). Each placement is one undo step. **New from map area…**: drag a rectangle on the map, then give it an id, name and category (*Include the ground layer* off for things that should stand on any ground) — its tiles, collision overrides and the entities standing in it become a new structure. **Structure editor** (*Database → Structures*, and the Asset Library window's *Edit* view; new ones via *New asset… → Structure* — empty grid or a copy of another structure — *New structure* in its Browse view, or *New…* in the Structures palette): name and category on top, then three columns. **Left**: the tools — *Tiles* (Paint tiles, Erase = the map keeps its tile, Clear map tile = placing removes the map's tile, shown as a red ×), *Collision* (Solid, Walkable, Tileset default, Keep map collision), *Objects* (Place prop) — and the **layer stack** in drawing order: *Overhead* (above characters: roofs, treetops), *Detail* (walls, doors, furniture), *Ground* (floors, grass, paths), each with what it holds and a checkbox to show / hide it; click a layer to paint on it (the others are faded; layers don't matter for collision and props and are greyed out then), plus size (*Resize*) and *Undo / Redo* (Ctrl+Z / Ctrl+Y, per structure; while a text field has the focus they edit the text instead). **Middle**: what a click does (e.g. *Paint tiles* on *Detail layer*), zoom (*Fit* or the slider) and the grid — click or drag to paint, right-click picks a cell's tile (props: removes it); props in the structure are listed below. **Right**: the tileset (click a tile or drag over several to select a stamp, with a large preview), the collision legend, or a searchable gallery of props. The example project has 18 terrain structures; the starter library adds town houses, a brick shop, a watchtower, city walls, iron fence, hedge, plaza, market, canal, campsite and dungeon rooms, corridors and a prison cell. Placing writes ordinary tiles and entities, so games don't need anything new to show them.
- **Switching maps**: the map name in the top bar is a dropdown of all maps (plus *+ New map…*); Ctrl+M opens a searchable switcher, Ctrl+PgUp / PgDn step through the maps. Each map remembers where you were looking. A selected portal has ** Go to <map>** (or double-click the portal) and edge links have a  button.
- **Map transitions**: a *Map transition* zone is one end of a two-way link between maps — entering it moves the player next to its other end on the other map (one tile inside, away from the map edge, so they don't bounce back), and that end leads back. With the **Transition** tool (T): drag over the exit (e.g. where a road leaves the map), pick the target map, then drag the other end there — both zones are created and linked (`to_<map>`). Or draw one with the *Zones* tool and set it up in the inspector: *Leads to map* / *Other end*, **Draw the other end on …**, ** Go to**, **Unlink** (both ends). The inspector shows whether the link works both ways; broken links are labelled in red on the map. Portals and spawn points work as before.
- **Zones**: drag to create a spawn point, portal, trigger area or **enemy spawner**. Portals pick their target map and spawn from dropdowns; spawners pick a character, how many, and the respawn mode.
- **Edge transitions**, **music**, **map triggers** and **Rename map** are in the right panel when nothing is selected.
- **Files**: *File → New project… / Open project…* use the desktop's native dialogs (xdg-desktop-portal). Tilesets, sprite sheets and sounds have *Browse…* buttons; files from outside the project are copied into `tilesets/`, `sprites/` or `sounds/`.
- **Big props**: in *Database → Props*, drag over several cells in the sheet grid of the sprite picker (or set *Size (cells)*). Placement snaps by footprint (even widths center on a tile edge), a translucent ghost previews the prop, and the selected prop shows its collision box.
- **Locks**: in *Database → Props*, tick *Lockable* to give every instance of a prop a default lock (key item, whether the key *is used up* or *stays in the inventory*, what happens when unlocked, locked / unlock messages). *Can be locked again with the key* lets the player relock an opened gate / door by interacting with it (or using the key on it; containers only by using the key) while carrying the key — without a key item this simply closes it again. It can't be closed on somebody standing in it, and isn't available for props that disappear. A placed prop can *Customize lock* (another key), drop the definition's lock, or *Make this lockable* on its own. 'Unlocked' / 'Locked again' triggers and the *Set locked* action (`set_locked(target, b)` in scripts) react to / control locks.
- **Prop states**: in *Database → Props* every prop has a list of **states** — each with a name, its own sprite and whether it blocks movement (e.g. the Iron Gate: *closed*, *open*, *destroyed*; the shrine: *dormant*, *glowing*). The first one is the default; *+ Add state* adds one, renames it (placed props, the lock / container and "self" triggers follow), *Make default*, ⏶⏷ and ✖ organise them, and the selected state's sprite is picked (and painted with *Edit pixels*) right below the list. A lock can *switch state* when unlocked (with a closed / target preview), a container can switch state when first opened, a placed prop chooses its starting *State* in the map inspector, the **Set prop state** action / `set_state(target, "name")` switch it at runtime and the **Prop state** condition checks it. The current state is saved. Older projects using frame offsets are converted to named states when loaded.
- **Containers**: in *Database → Props*, tick *Lootable container* to give every instance of a prop a default container (slots, starting contents, "opens only if" conditions + locked message, the state to switch to when opened). In the map inspector, a placed prop can *Customize size & contents* (or become a container even if its definition isn't one). Items have a *Stack size*; the player's *Inventory slots* are set in *Database → Project*.
- **New project wizard**: *File → New project…* (or the welcome screen) asks for the **game name** (the project folder name follows it, e.g. `knight-tales`, and can be changed), **where** to create it (*Browse…*), the **player character's sex** (*Player chooses / Always male / Always female*), whether the **character creator** is shown and its default name, whether to copy **starter assets** from the library (grass tileset, hero looks, coin / apple / potion / sword / key, chest / sign / barrel / well / boulder), the **first map** (name and size; grass-filled with the starter assets, with a start point in the middle), the **pixel scale** and the **inventory slots**. The project opens right away, ready for F5.
- **Character creator & addressing the player**: in *Database → Project → Character creator*, set the **player character's sex** (*Player chooses*, or locked to *Always male* / *Always female* — the creator then shows it fixed and only offers looks of that sex; play-tests and saves use it too), tick *Show the character creator…* to let players create their character after *New game*: a **name** (default name configurable), **Male / Female** and one of the **looks** of that sex (an animated preview walks in all directions). Looks are sprite sheets in the player character's layout, each with a sex and a label; *+ Add a look…* picks one from the project or the asset library (library looks are copied in), the thumbnail changes it. Name, sex and look are kept in save games; play-tests from the editor skip the creator and use the default name. The **speaker** field of *Dialog* / *Choice* actions has a dropdown (⏷) with `{self}`, `{player}`, the characters on the open map and the speakers its dialogs already use — or type any name. In **dialog and choice texts** (speaker too) and quest texts, a speaker of `{self}` shows the name of the trigger's owner, and `{player}` is replaced by the name and `{sir|madam}` by the first word for a male player and the second for a female one (`{name}` still shows variable `name`); conditions have **Player sex**; scripts have `player_name()`, `player_sex()` (`"male"` / `"female"`), `is_female()`, `is_male()` and `text("...")` (fills in the same placeholders). The example has six looks, and Elder Rowan, Mira, Tilda and the villagers address the player by name and with *sir / madam*-style forms.
- **Loot**: characters that can be defeated (Max HP > 0) have a **loot table** in *Database → Characters → Loot*: each line is an item, a drop chance in percent and an amount (min–max); every line rolls on its own. A placed character can *Customize loot* in the map inspector (or go back to the definition's), and an **enemy spawner** zone has *Loot of spawned characters*, which every character it spawns uses instead of the character's own table. When a character is defeated, what drops lands on the ground where it fell — like items the player dropped, it stays there (also across map changes and in save games) until picked up. In the example, slimes drop coins and apples and goblin archers arrows and coins; in the forest, the slime den adds potions and the **goblin camp** has its own richer table (arrows, coins, bolts, sometimes a leather cap, rarely a crossbow).
- **Patrol routes**: select a placed character, *Patrol route → Add a patrol route*, then click tiles on the map to add waypoints (drag a waypoint to move it, right-click removes it, Esc or *✔ Done editing* when finished). The route is drawn with numbered points and arrows while the character is selected. Choose *Loop* or *Back and forth*, the *Pause* at each point and an optional *Speed*. In the game the character walks the route, finding its way around obstacles, waits briefly when talked to, and continues where it was after map changes and loading a saved game. The **Patrol** action (start / stop; scripts: `patrol(target)`, `patrol(target, [[x, y], …])`, `stop_patrol(target)`) controls it. The example's town guard patrols the village road and stops once the shrine is lit.
- **Scripts**: *Database → Scripts* lists every `.rhai` file of the project with its functions underneath; click a file to edit it, or a function to jump straight to it. The first script opens automatically, switching files saves the edited one, *Save & check* (Ctrl+S) saves and reports script errors in the status bar, and *+ New* creates a script (named after the text field, or `script_N.rhai`). The **script editor** has syntax highlighting (keywords, strings, numbers, comments, engine functions, your own functions), line numbers and **auto-completion**: while typing it suggests engine functions with their parameters, your script functions, keywords and `let` variables (Ctrl+Space shows everything; ↑↓ choose, Tab / Enter accept, Esc close). Inside a string argument it offers exactly what the function expects — items for `give_item("`, maps and then that map's spawn points for `teleport(`, quests, sounds, prop states, flags and variables used in the project, entity names / `self` / `player` for targets, script functions for `then_call("` and directions for `face(`. A line above the editor shows the parameters of the call around the cursor (the current one highlighted) with a short description. Enter keeps the indentation (one level more after `{`), brackets and quotes close themselves, and **Ctrl+click** on a function name jumps to its definition (also in other files). **Problems** under the file list are checked live and underlined in red in the code: ids passed to script functions that don't exist in the project — unknown items, maps, spawn points (of the given map), sounds, quests and prop states, e.g. `give_item("potoin")` — with file and line; click one to jump there.
- **Quests**: *Database → Quests* defines quests: id, title, description and **stages**, one objective each (write `{name}` to show the value of variable `name`, e.g. "Defeat slimes ({pest_slimes}/5)"). Triggers drive them with the **Quest** action (*start*, *advance to the next stage*, *set stage*, *complete*, *fail*) and check them with the **Quest** condition (*not started*, *active*, *completed*, *failed*, *active at stage*, *at stage or later*); scripts use the quest functions below. The player gets a message for every change and sees the progress in the journal (J); quest progress is kept in save games. The example has two: *The Strange Lights* (Elder Rowan) and *Pest Control* (Tilda: defeat five slimes for 30 coins and a potion).
- **Character models**: the look of a character is its sprite sheet. **Choose model…** (in *Database → Characters*) and **Model → Change…** (map inspector, for one placed NPC only) open the model chooser: *This project* lists the character sheets already in the project, *Asset library* every character of the library (search, *All / Male / Female*). Click to preview all four directions, double-click or *✔ Use this model* to apply; a library model's sheet is copied into the project's `sprites/` (renamed if its id is taken by a different sheet). A placed NPC with its own model shows *(own)* and has *Reset* to go back to the definition's look.
- **Merchants**: in *Database → Characters*, tick *Merchant* to make every instance of a character a trader: optional shop title, the **currency** item (default `coin`), what the merchant pays for the player's items (*sell rate* in % of the price; 0 = buys nothing), whether *talking opens the shop* (otherwise use the **Open shop** action, e.g. after a dialog choice, or `open_shop(target)` in scripts), and the **stock**: item, quantity or *unlimited*, and price (0 = the item's **Value**, set in *Database → Items*). In the map inspector a placed character can *Customize stock & prices* or become a merchant on its own (*Make this a merchant*). In the game, talking to a merchant runs its Interact triggers (e.g. a greeting) and then opens the trade window. Items the player sells join the merchant's stock; the stock of every merchant is kept in save games. In the example village, Tilda (*Merchant (female)*) has her shop in the house with the sign north-east of the elder: she stands behind her counter and is talked to across it (E in front of the counter, or right-click her); her own triggers greet the customer, offer / reward her quest, and then her shop opens.
- **Equipment**: in *Database → Items*, choose *Equipment: Weapon / Helmet / Body armor / Left hand*. Weapons are melee (damage, cooldown, reach, arc, knockback) or ranged (projectile sprite, speed, range, ammo item); tick **Two-handed** for weapons that need both hands (the example's spear, bow and crossbow). Armor has defense, a speed change and an optional overlay sprite sheet (same layout as the player sheet). **Left hand** items are shields (defense) or lights (lantern, torch — give the item a *Light*); they can't be held together with a two-handed weapon: equipping one puts the other back into the inventory. Characters have a *Defense* value; hostile characters get an optional ranged attack.
- **Skills & magic** (optional per project: *Database → Project → Skills & magic*): **Skills** are defined in *Database → Skills* (name, description, icon, max level) with a built-in **effect** per level — *Melee damage* (weapons and bare hands, +%), *Ranged damage* (bows, crossbows, +%), *Spell damage* (bolt and blast spells, +%), *Defense* (+points), *Speed* (+%), *Lockpick* (opens locks up to a difficulty: set *Lockpicking* on a prop's or placed prop's lock; a lock without a key item can only be picked), *Haggle* (merchants' prices better by %), *Mana* (+max mana), *Mana regen*, *None* (for conditions and scripts), or *Script*: a Rhai function called as `name(level)` whenever the level changes and once when a game starts or loads, so a script can apply any effect it likes (the example's *Scavenging* makes slimes drop extra coins). Players get **skill points** from the *Give skill points* action or `give_skill_points(n)` (the example's quests give them; *Starting skill points* for new games) and spend them on the **Skills screen (K)**. **Magic** (tick *Magic*) adds **mana** (*Max mana*, *Mana regeneration* per second, plus skill bonuses; a blue bar under the hearts) and **spells** (*Database → Spells*: mana cost, cooldown, an optional required skill level, an icon and an effect — *Heal*, *Bolt* (a projectile in the facing direction), *Blast* (damages everything within a radius), *Light* (a glow around the player for a while) or *Script*). Spells are learned from items with *Teaches spell* (scrolls that are *Consumed*, books that aren't) or the *Learn spell* action, and cast from the Magic tab (K) or from the **quick bar** (1–8 in the Magic tab puts a spell there). **Bolt spells are aimed with the mouse**: casting one turns the cursor into a magic crosshair (gold over an enemy, which also gets a ring); the next left click fires the bolt at the enemy under the cursor (or at the clicked spot), X fires straight ahead, right-click or Esc cancels — mana and cooldown are only spent when the bolt flies. Conditions: *Skill level*, *Knows spell*; actions: *Give skill points*, *Set skill level*, *Learn spell*, *Add mana* (mana potions), *Damage around player*, *Light around player*. Everything is saved with the game. The example has eight skills (Swordsmanship, Archery, Toughness, Lockpicking, Haggling, Arcana, Spellcraft, the scripted Scavenging) and four spells (Heal and Light from scrolls Tilda sells, Spark from the elder's spell book, Fire Burst from a tome in the mine's pick-only chest; the forest chest can be picked at Lockpicking 1).
- **Lighting, day and night**: each map has a **Lighting** setting (map inspector): *Always bright* (default), *Outdoor (day & night)* — bright by day, darker at night by its *night darkness*, with warm dusk and dawn — or *Dark (dungeon, cave)* — constant darkness, 100 % = pitch black so only light sources show anything. **Light sources** (radius in tiles, color, strength, optional flicker, with presets): tiles (*Database → Tilesets → Light tiles*: list, or *Pick light tiles in the grid*; a bulb marks them), props per state (*Database → Props → Light*, e.g. a lamp lit in "lit" and dark in "off"), characters (*Database → Characters → Light*), items (*Database → Items → Light*: glows while equipped — e.g. in the left hand — or lying on the ground) and any placed entity (*Light* in the map inspector, instead of its definition's). The **clock** runs on: *Database → Project* sets the *Day length* (real minutes per in-game day, default 24, 0 = stands still) and the hour new games start at; save games keep the time. On outdoor maps the game shows the time top right. The *Set time of day* action and the *Time of day* condition (e.g. 20 to 6 = at night) use it; scripts have `hour()`, `is_night()` and `set_hour(h)`. **View → Lighting preview** (bulb in the ribbon, with a time slider for outdoor maps) shows darkness and lights in the editor. The example: village, forest and wilds have day and night (lit windows glow, the patrolling guard carries a lantern, the shrine glows once lit), Tilda sells a lantern, torches and a wooden shield, and a pitch-dark **old mine** in the forest's northern cliff, just west of the cave to the wilds has wall torches, slimes and a treasure room without torches (a torch lies at its entrance).
- **Choosing sprites**: item icons and prop-state sprites show a large preview and a **🖼 Choose…** button. The chooser lists every sprite sheet as a thumbnail (with search) and shows the chosen sheet's frames — as big numbered thumbnails for single frames, or as the sheet grid where you can drag over several cells for big props. Frames already used by other items / props are marked (hover shows by whom); click to pick, double-click to pick and close.
- **Items, props and characters with their art**: *+ New*, *Duplicate* and *Delete* manage the definitions; each form has **Edit art**, which shows the pixel editor under the form at that definition's sprite (the item's icon, the selected prop state, the character's sheet). **+ New blank icon / frame for this state** finds an empty frame (or block of frames for big props) on the sheet — or adds a row of frames when it is full — assigns it and opens it for drawing; items / props without a sheet get one. Characters have **+ New sheet**: a blank 4×4 walking sheet or a copy of another character's sheet to start from.
- **Editing sprite sheets**: *Database → Sprite sheets* shows the sheet's settings (image, frame size, which items / props / characters use it) and the **pixel editor right below** — draw, copy / mirror / clear frames, add frames (*Canvas … Resize*), preview animations; Ctrl+S saves and updates the sheet everywhere. A new sheet whose image doesn't exist yet offers to create a blank one.
- **Database**: every definition has a *Rename…* button next to its id. Renaming rewrites all references (maps on disk and the open map, triggers, other definitions, project settings) and saves immediately. **Scripts follow too**: item, map and sound ids passed to script functions (`give_item("coin", 1)`, `teleport("forest", …)`, `play_sound("…")`, …) are rewritten (only those exact arguments — comments and other strings stay as they are); changed files keep a `<file>.rhai.bak` backup and the status bar lists the changed lines.
- **Tile animation**: in *Database → Tilesets*, add an animation, then click tiles in the image to append frames.
- **Sounds**: *Database → Sounds* lists sound files with volume and a preview button. Built-in game sounds are set in *Database → Project*.

### Releasing a game

*File → Export game (standalone)…* saves the project and writes a bundle that runs without the editor or Rust:

```
releases/<game-name>/
  <game-name>      the game executable (run it to play)
  data/            project.ron, defs/, maps/, scripts/ and only the images / sounds in use
  README.txt
releases/<game-name>-linux.zip   (optional, to share)
```

The executable is a copy of `rust2d-game`; started without arguments it runs the `data/` folder next to it, titles the window with the game name and keeps save games in `~/.local/share/<game-name>/saves` (the bundle folder may be read-only). Save games of the project are never shipped. The export checks the project first (start map must exist; missing files and an unset player are reported) and replaces an earlier export in the same place, but never touches other folders. Build the game in release mode first (`cargo build --release -p rust2d-game`) — the editor picks `target/release/rust2d-game` when it exists and warns when only a debug build is available. The bundle targets the platform it was built on (Linux x86_64 here) and needs the usual system libraries (OpenGL, X11/Wayland, ALSA).

### Asset library, packages and the pixel editor

- **Asset library**: a shared collection of tilesets, sprite sheets, sounds, items, props, characters and structures that every project can use. It lives in `~/.local/share/rust2d/library` (or `$XDG_DATA_HOME/rust2d/library`; `$RUST2D_LIBRARY` overrides it) and is filled with the default assets the first time it is opened: the example game's art plus **desert, mountain, city and dungeon tilesets** (with animated oasis / canal water, waterfalls and lava), **21 more props** (street lamp lit/off, crate and pot intact/broken, campfire lit/out, lever off/on, wall torch lit/out, signpost, tombstone, anvil, haystack, bones, treasure, crystals, bench, sack, big cactus, palm tree, market stall, fountain, tent, cart) and more characters. Every humanoid type comes as a **male and a female version** (`<id>` / `<id>_f`; the noble lady's counterpart is `noble_m`): hero, elder, villager, goblin archer, merchant, town guard, wizard / sorceress, farmer, desert nomad, blacksmith, noble, and the hostile bandit and skeleton; plus a bat and a ghost. When a newer editor brings more default assets, the library adds the ones it doesn't have yet the next time it is opened; existing assets — including your edits — are never changed. 
- **Asset Library window** (top bar → *Asset Library* or *Assets → Asset Library window…*): opens the library in **its own window** (a second editor process, `rust2d-editor --library [DIR]`; clicking again while it is open just says so). It has three views:
  - *Browse*: every library asset as a card (click to select, filter by kind, search). *Edit* (or double-click a card) opens it in the Edit view; *New asset…* creates a new prop, item, character, tileset or sprite sheet **in the library**; *Import package…* adds a `.r2dpack`; *Export package…* writes the selection (a zip with `manifest.ron` and the files) to share; *Remove from library…* asks for confirmation first (listing the assets; Esc or *Cancel* keeps them) and refuses to remove things other library assets still use.
  - *Edit*: the Database forms (Items, Props, Characters, Tilesets, Sprite sheets, Sounds, Structures) on the library's definitions, with *Edit pixels*, *Rename…* and *Delete…* (asks first). Save with **Ctrl+S** or *File → Save*; closing the window with unsaved edits asks to save or discard them.
  - *Pixel editor*: draw and fix library images in place.

  When the main editor puts something into the library (*Add to library*) while the window is open, the window reloads by itself; if it has unsaved edits a banner offers *Reload* or *Keep mine*.
- **Add from library** (in a project): **Add from library…** in every *Database* asset tab (Items, Props, Characters, Tilesets, Sprite sheets, Sounds, Structures), next to the map's *Tileset* palette (tilesets), the *Structures* palette and the *Place* palette (the selected kind: props, characters or items; *All* shows everything), and in *Assets → Add from library…* (all kinds, with kind tabs). The dialog shows the library's assets of that kind as cards with thumbnails — always as last saved in the Asset Library window. Click cards to pick one or more (*Pick all shown*, search), assets the project already has are marked *in project*, and a line lists what the picks bring along; **Add N to project** (or a double-click on a card) copies them in:
  - Adding copies the picked assets **and everything they need** (sheets, ammo, keys, overlay sheets, sounds used by triggers) into the project. Assets already in the project with identical content are reused; an id or file name that is taken by something different gets a `_2` suffix and all references follow. Packages made for another tile size (older 16 px exports) are enlarged to 32 px on import: images (nearest neighbour, lossless), frame sizes, hitboxes and speeds.
- **From a project**: *Database* has *Add to library* and *Export…* for the selected definition, and *Assets → Import asset package into project…* imports a `.r2dpack` directly.
- **New asset…** (*Assets* menu, *Database*, or the Asset Library window for library assets): a wizard for a prop (size in tiles), item, character (4 walk frames × 4 facings), tileset or sprite sheet. Draw a new image, start from a copy of an existing one, or reuse a frame of an existing sheet. New images open in the pixel editor; the definitions are created with sensible hitboxes and speeds for the project's tile size.
- **Pixel editor** (top bar → *Pixel editor*, or *Edit pixels* on a sprite sheet / tileset): **select frame (S, the default tool: clicks only select the frame under the cursor, nothing is drawn)**, pencil (B), eraser (E), fill (G), line (L), rectangle (R) and color picker (I); left / right mouse buttons draw with two colors (X swaps), palette + colors used in the image, wheel zoom, middle-drag pan, pixel and frame grids (character sheets label their rows Down / Left / Right / Up), undo / redo (Ctrl+Z / Ctrl+Y), frames: click one to select it, **Ctrl+C** copies it, **Ctrl+X** cuts it (copies and clears — paste elsewhere to move it), **Ctrl+V** replaces the selected frame with the copy (also as buttons, plus *Paste mirrored*, *Flip ↔* and *Clear*), resize the canvas in frames, and a live preview of the selected frame and of its row as an animation. Ctrl+S saves the PNG and reloads it everywhere.

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
| MapEnter | the map, when the player enters it (not when a saved game is loaded — that resumes the game) |
| Custom (name) | map, all zones and all active entities, raised by the `Emit` action or `emit()` |
| Defeated | a character whose HP reached 0 |
| Unlocked | a prop whose key lock was opened |
| Locked | a prop that was locked again |

The editor only offers the events that can reach a trigger where it is defined:

| Triggers of | Events |
|---|---|
| a map | Map enter, Custom event |
| a zone | Enter, Leave, Interact, Custom event |
| a character (placed or definition) | Interact, Use item, Defeat, Custom event |
| a prop (placed or definition) | Interact, Use item, Unlocked, Locked, Custom event |
| an item pickup on a map | Interact, Use item, Custom event |
| an item's *On use* | Use item |

Older triggers with an event that can never reach them (e.g. a map trigger *On Enter*, which only zones receive) still load; they are marked ⚠ with a one-click fix (*Use 'Map enter'*).

Entities run their definition's triggers plus their own. Picking up an item entity is automatic on Interact.

**Conditions:** flag set, variable comparison, has item (count), player HP comparison, prop state, quest, player sex, time of day (from–to hours, wraps around midnight), skill level comparison, knows spell, script function returning `bool`; each can be negated.

**Actions:** Dialog, Choice (stores the chosen index in a variable), Give/Take item, Set flag, Set variable (= or +=), Teleport (map + spawn), Set active (show/hide an entity), Set prop state (e.g. open / destroyed), Set frame (old frame offset), Move (walk by tiles, blocking), Face, Wait, Emit event, Run script, If/Else, Play sound, Play music, Heal player (negative = damage), Set locked, Set time of day, Give skill points, Set skill level, Learn spell, Add mana, Damage around player, Light around player. Action targets are `self` (the trigger's owner), `player`, or an entity name. Entity state changes persist across map changes and saves.

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
- Queued actions (run after the function returns, in order): `say(text)`, `say(speaker, text)`, `say(SELF, text)` (the owner's name as speaker, e.g. "Town Guard"; any entity uid works), `choice(text, [options], var)`, `teleport(map, spawn)`, `set_active(target, b)`, `set_state(target, name)`, `set_frame(target, f)` (old), `move_by(target, dx, dy)`, `face(target, "up"|"down"|"left"|"right")`, `wait(secs)`, `emit(event)`, `then_call(fn)`, `heal(n)`, `play_sound(id)`, `play_music(id)`, `set_locked(target, b)`, `open_shop(target)`, `patrol(target)`, `patrol(target, [[x, y], …])`, `stop_patrol(target)`, `start_quest(id)`, `advance_quest(id)`, `set_quest_stage(id, n)`, `complete_quest(id)`, `fail_quest(id)`. Use `then_call` to continue after a dialog or choice.
- Player: `player_name()`, `player_sex()` ("male" / "female"), `is_female()`, `is_male()`, `text("Thanks, {sir|madam} {player}!")`.
- Time: `hour()` (0..24, e.g. 13.5), `is_night()`, `set_hour(22)` (queued).
- Skills and magic: `skill("lockpicking")` (level, 0 = none), `skill_points()`, `knows_spell("heal")`, `mana()`; queued: `set_skill(id, level)`, `give_skill_points(n)`, `learn_spell(id)`, `add_mana(n)`, `damage_around(damage, radius)`, `player_light(radius, seconds)`. A skill with the *Script* effect calls `fn name(level)` whenever its level changes (and once when a game starts or loads), a spell with the *Script* effect calls its function when cast.
- Other: `player_hp()`, `random(n)` (0..n), `quest_stage(id)` (-1 = not started), `quest_active(id)`, `quest_done(id)`.
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

- Enemies placed directly on a map stay dead once defeated; use a spawner zone for respawning enemies.
- Native file dialogs need a running xdg-desktop-portal (standard on GNOME/KDE desktops); paths can always be typed by hand.
