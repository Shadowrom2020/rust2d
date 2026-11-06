# R(ust)PG2D roadmap

Planned features of R(ust)PG2D, grouped into themed releases, with the design decisions and the
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
    rustpg2d-game | grep GLIBC_`), the Debian 12 container (`ldd`), and the Flatpak permissions
    (`--device`, `--socket`) in `editor/src/package.rs` and `xtask/src/dist.rs`.
11. **Example content.** Regenerate with `cargo run -p rustpg2d-engine --example gen_example`
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

## 0.6.0 — The party (companions and faces)

Theme: companions become real party members (the player heals them, picks them up when they
fall, gives them orders and gear), and characters can have drawn portraits. Builds on
*Companions* and *Dialog portraits* of 0.5.0.

### [x] Healing companions — `feature/heal-companions`

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

**Decided while building.** `Action::Heal` keeps its shape: version-1 map files decode actions
with postcard, which can't skip a new field, so the named heal is a new appended variant
`Action::HealCharacter { target, amount }` ("player", "self" or a name) and an item used on a
companion runs its *on use* actions with the companion as "self" and *Heal player* turned into
*Heal character self* (`runtime/healing.rs` `redirect_heals`). The choice only opens for items
whose *on use* heals (other items behave as before) and pauses the world like the minigames (the
character window steps aside meanwhile); it is a bar at the bottom (toasts use the top), the
most hurt candidate is pre-selected; one click on a name in the bar, a character or a HUD row
heals at once and pointing at one selects it (user feedback: no extra E after clicking). `UseTarget::Ally` with no companion here refuses with a
toast; with exactly one candidate it is used at once. The Heal spell pays its mana only once
the target is chosen. The red potion lost its "already at full health" line (the target may be
someone else). `heal_area` now saves companion HP too.

**Touch points.** `defs.rs` (`UseTarget`, `ItemDef.use_on`), `events/mod.rs` (`Heal.target`),
`runtime/{skills,companions,combat,mod,inventory_ui}.rs`, a HUD part in `runtime/ui.rs` or
`companions.rs`, `script.rs` (`heal_npc`), editor `widgets.rs` (Heal target field) and
`database.rs` (item *Use on*), `script_editor.rs`, README, scripting.md.

**Example.** The guard companion; Tilda sells a *Healing Scroll* (`Choose`); the red potion
becomes `Choose`.

**Verify (done).** Autoplay from a crafted save (`follow("guard")` and `heal_npc("guard", -5)` in
the console): a potion opens the choice with the guard pre-selected (`aim=heal:guard`), E → guard
5/8 → 8/8, the player's HP unchanged, one potion used; Right moves to the player; Esc cancels and
keeps the potion; the Heal spell: mana 20 while choosing, 10 after, guard healed; without a
companion the potion heals the player at once; clicking the guard's HUD row and clicking him on
the map both heal him with a bandage; F5 saves his healed HP. Screenshots of the bar, the ring on
the guard and the HUD row; the editor's *Use on* field and a *Heal character* action. Unit tests:
`use_on` defaults to the player (not written), `redirect_heals` inside *If* actions.

### [x] Knocked-down companions — `feature/downed-companions` (follows *Healing companions*)

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

**Decided while building.** `Companion.down: Option<[f32; 2]>` stores where it lies (a downed
companion is always on the current map: leaving asks first), `Link.down` the seconds it has
been lying; `knock_down` replaces the old knock-out in `defeat_with`, `companion_healed` (called
by `heal_entity` and `heal_area`) gets it up. Talking to a downed companion always just toasts
"… is out cold" (its triggers don't run: a dialog with someone unconscious makes no sense).
The question goes through `Game::transition`, so portals, edges and scripted teleports all ask;
after "Stay" leaving doesn't ask again for 1.5 s (walking into an edge would re-ask at once).
Effects are cleared when it falls (poison can't tick on), and its HP bar above it is hidden.

**Touch points.** `state.rs` (`Companion.down`), `project.rs` (`DownRule`),
`runtime/{companions,combat,mod,maps,furniture}.rs` (the rotated drawing made reusable for
characters), editor *Project settings*, README.

**Example.** Nothing new beyond the guard; the setting stays at *stays down*.

**Verify (done).** Autoplay on a scratch copy: `heal_npc("guard", -20)` → `guard:0/8:down`, he
stays at 9,18 while the player walks off, a potion (choice pre-selects him) → 6/8 and he
follows; F5 / F9 while down keeps him down at the same spot (`down: Some((288.4, 606.5))` in the
save); a teleport asks, "Stay" keeps the map, "Leave" → `companions=[]` and the shop; in the
forest's slime den four slimes hunt the player (12 → 3 HP) and leave him alone; *gets up after
3 s* → 1/8, *goes home* → dismissed at once; talking to him toasts "out cold"; screenshots of
him lying, the HUD's DOWN, the question and the editor setting. Unit test: old saves without
`down`.

### [x] Commands for companions — `feature/companion-orders`

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

**Decided while building.** Fighting and staying are separate: `Companion.order: Order { Attack,
Defend, Passive }` and `Companion.post: Option<[f32; 2]>` (where it waits), so a staying
companion keeps its way of fighting (with *Stay*, it fights what comes within 2.5 tiles of it
and walks back to its post). No `stay_follows_across_maps` setting: a staying companion always
catches up on a map change (`companions_follow_again` in `Game::transition`, not when loading
a save). The menu is `runtime/orders.rs`, a bar at the bottom like the heal choice (pauses the
game; one click on an order gives it). Pad: all buttons were taken, so LB opens the menu while
playing and only switches tabs inside an open window (`Act::PrevTab` places = windows).
Scripts also get `is_staying(name)`; `status()` `companions=` shows `:defend` / `:passive`,
`:stay`, `:fighting` and `orders=who:choice` while the menu is open.

**Touch points.** `state.rs` (`Order`), `runtime/{companions,input,menu or ui}.rs`,
`events/mod.rs`, `script.rs`, `script_refs.rs` (no ids: the name is a character name like
`follow`), editor `widgets.rs`, `script_editor.rs`, README (controls table), scripting.md.

**Example.** The guard's dialog after joining offers "Wait here" / "Follow me" / "Stay out of
fights" via `order`.

**Verify (done).** Autoplay on a scratch copy: `press g`, Right ×3, E → `guard:8/8:stay`, he stays
at 8,18 while the player walks off (screenshot with STAY in the HUD row); pad LB opens the menu;
with Mira along the menu has an *All / Villager / Town Guard* row, a click on *Defend* sets both,
choosing the guard then *Passive* only him. In the slime den (staying): *Passive* never
`:fighting`, *Attack* and *Defend* fight the slimes hunting him; next to an idle slime *Attack*
fights it, *Defend* doesn't. F5 / F9 keeps `defend:stay` at the post; a teleport makes a staying
guard follow again; the guard's dialog (*Wait here*) → `stay`; the editor's *Companion order*
fields (screenshot).

### [x] Companion inventory — `feature/companion-inventory`

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

**Decided while building.** Not a tab of the character window but its own **companion window**
(`runtime/companion_ui.rs`, built like the loot window: gear + bag + the player's inventory),
opened from the companion's HUD row, *Gear...* in the orders menu or `open_companion(name)` /
`Action::OpenCompanion` (appended; opens once the event is over). Mouse: click moves a stack,
right-click one, dragging onto a gear slot (or Enter on a bag item) equips; no double-click
(the first click would already move the stack). The bag holds 12 kinds of items (stored as the
entity's item list, `persist_npc_items`). Automatic use (`runtime/npc_use.rs`), for every
character with HP and items, enemies included (user feedback: a guard with potions has to drink
them when it's critical): the item's *on use* actions are scanned (heal, cure on "player",
positive effect on "player") and run with the character as "self" (`healing::redirect_heals`
also redirects effects; dialog lines are dropped); health is critical at 35 % or when HP ≤ the
last hit taken (`RtEntity.last_hit`: an 8-HP guard taking 4-point hits drinks at 4); one item
per 4 s (`RtEntity.item_cd`). Ammunition only for a ranged weapon the player gave it (its definition's own weapon
needs none); out of ammo it keeps out of fights and says so once. `CharacterDef.gear_locked`
also keeps its own gear on. `status()`: `compwin=name[bag]`; autoplay `slot` / `slotdrag` take
`gear`, `bag` and `player` while the window is open.

**Touch points.** `runtime/{charwin,inventory_ui,companions,npc_items,combat}.rs`, `defs.rs`
(`gear_locked`), `script.rs`, game `main.rs` (autoplay), README, scripting.md.

**Example.** The guard's dialog while he follows offers "Let me see your gear"
(`open_companion`); the hunting bow in the elder's house and Tilda's arrows and potions are
there to try it.

**Verify (done).** Autoplay on a scratch copy: clicking the guard's HUD row opens the window
(`compwin=guard[]`); clicks give him 2 potions and 20 arrows, a right-click takes one arrow back
(`guard[potionx2,arrowsx19]`); dragging the bow onto his weapon slot equips it, the spear goes
into his bag (`npcs=` `guard:…,bow|arrowsx5,spearx1`); hurt to 2/8 he drinks a potion at once
("Town Guard uses Red Potion.", 8/8, one left); in a real fight in the slime den (no scripted
damage) he drinks one by himself and the potion's dialog line doesn't show; the Bandit Chief,
given a potion and hurt to a critical 2/10, drinks it; in the slime den with 3 arrows he shoots
3 → 2 → 1 → 0, then "Town Guard is out of Arrows." and the HUD row's red ×0; F5 / F9 keeps his
bag; the dialog's "Let me see your gear" opens the window; with *can't be given gear* the bow
stays with the player. Screenshots of the window, the toasts, the HUD ammo and the editor
checkbox. Unit test: what items do (`scan`).

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

Theme: R(ust)PG2D and the games made with it run where the players and creators are: Windows,
macOS, the web, and in their language. Postponed from 0.6.0 (2026-11-06) so the gameplay
releases come first; the design notes are unchanged.

### [ ] Windows export — `feature/windows`

**Design.** Gate the `std::os::unix` uses (`editor/files.rs` copy permissions,
`packaging/lib.rs` symlink and executable bit, `release.rs` chmod); Windows paths for saves and
settings (`release::user_saves_dir`, `Settings::dir`: `%APPDATA%`); `.exe` handling in
`export::game_binary` and `release::export` (`<slug>-windows.zip`, README text per platform).
CI builds `rustpg2d-game.exe` with `x86_64-pc-windows-gnu` in the Packages and Release jobs and
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
  `%APPDATA%\rustpg2d`, data `%LOCALAPPDATA%\rustpg2d`, cache `%LOCALAPPDATA%\rustpg2d\cache`, home
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
  example, LICENSE, README) and upload `rustpg2d-<version>-windows-x86_64.zip`.
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
- Paths: `~/Library/Application Support/rustpg2d` (config and data), `~/Library/Caches/rustpg2d`
  in `paths.rs`. Bundle layout: `R(ust)PG2D.app/Contents/{MacOS/rustpg2d-editor,
  MacOS/rustpg2d-game, Resources/example_project, Info.plist, R(ust)PG2D.icns}`; `bundled_example`
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
smoke test with `RUSTPG2D_AUTOPLAY` under the runner's window server), opens the example and
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

### 0.5.0 — Living world and adventure (2026-11-06)

- [x] Weather — `feature/weather`
- [x] NPC schedules — `feature/schedules`
- [x] Companions — `feature/companions`
- [x] Crafting — `feature/crafting`
- [x] Achievements — `feature/achievements`
- [x] Minigames: lockpicking and fishing (with the Tool slot) — `feature/minigames`
- [x] The player uses furniture — `feature/furniture`
- [x] Dialog portraits — `feature/portraits`
- [x] Renamed from rust2d to R(ust)PG2D (`rustpg2d`) — `feature/rename-rustpg2d`

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
