// Rhai scripts for the example project.
// Functions here can be called from triggers with the "Run script" action
// or used as conditions ("Script" condition, must return a bool).
//
// World state:  flag(n) set_flag(n, b) get_var(n) set_var(n, v) add_var(n, v)
//               item_count(id) has_item(id) give_item(id, n) take_item(id, n)
// Queued:       say(text) say(speaker, text) choice(text, [options], var)
//               teleport(map, spawn) set_active(target, b) set_frame(target, f)
//               move_by(target, dx, dy) face(target, "up"|"down"|"left"|"right")
//               wait(seconds) emit(event) then_call(fn)
//               heal(n) play_sound(id) play_music(id) set_locked(target, b)
//               open_shop(target) set_state(target, state)
//               patrol(target) patrol(target, [[x, y], ...]) stop_patrol(target)
// Quests:       start_quest(id) advance_quest(id) set_quest_stage(id, n) complete_quest(id)
//               fail_quest(id) (queued); quest_stage(id) (-1 = not started) quest_active(id)
//               quest_done(id)
// Player:       player_name() player_sex() ("male"/"female") is_female() is_male()
//               text("...") fills in placeholders: {player} {sir|madam} {variable}
// Skills/magic: skill(id) skill_points() knows_spell(id) mana(); queued: set_skill(id, level)
//               give_skill_points(n) learn_spell(id) add_mana(n) damage_around(dmg, radius)
//               player_light(radius, seconds)
// Other:        player_hp() random(n) current_map()
// Targets: "self" (trigger owner), "player", or an entity name.

fn mira_talk() {
    add_var("mira_talks", 1);
    let n = get_var("mira_talks");
    if n == 1 {
        say("Mira", "Hi! I'm Mira. " + player_name() + ", right? Have you met Elder Rowan by the well?");
    } else if flag("shrine_lit") {
        // Scripts can check the player's sex too (or use "{hero|heroine}" in the text).
        if is_female() {
            say("Mira", "You lit the shrine? When I grow up I want to be a heroine like you!");
        } else {
            say("Mira", "You lit the shrine? When I grow up I want to be a hero like you!");
        }
    } else if n < 4 {
        say("Mira", "Talked to me " + n + " times now. You must be bored!");
    } else {
        say("Mira", "Want to see me walk?");
        choice("Watch Mira walk?", ["Sure", "No thanks"], "mira_walk");
        then_call("mira_walk");
    }
}

fn mira_walk() {
    if get_var("mira_walk") == 0 {
        move_by("mira", 2, 0);
        move_by("mira", 0, 1);
        move_by("mira", -2, 0);
        move_by("mira", 0, -1);
        face("mira", "down");
        say("Mira", "Ta-da!");
    }
}

fn shrine_touch() {
    if flag("shrine_lit") {
        say("The shrine glows softly.");
        return;
    }
    if !flag("quest_accepted") {
        say("An old stone shrine. It seems dormant.");
        return;
    }
    say("You touch the shrine. Ancient runes begin to glow!");
    set_flag("shrine_lit", true);
    set_state("self", "glowing");
    set_quest_stage("strange_lights", 2);
    emit("shrine_lit");
}

// Called by the slime's "Defeated" trigger (what slimes drop is set in their loot table).
// A scripted skill effect (Database > Skills > Scavenging, effect "Script"): the engine calls
// scavenging(level) whenever the level changes, and once when a game starts or loads.
fn scavenging(level) {
    set_var("scavenging", level);
}

fn slime_defeated() {
    add_var("slimes_defeated", 1);
    // Scavenging (a scripted skill) adds a coin per level to every slime.
    if get_var("scavenging") > 0 {
        give_item("coin", get_var("scavenging"));
    }
    // Tilda's quest counts slimes once it was accepted.
    if quest_active("pest_control") && quest_stage("pest_control") == 0 {
        add_var("pest_slimes", 1);
        if get_var("pest_slimes") >= 5 {
            set_quest_stage("pest_control", 1);
        }
    }
    if get_var("slimes_defeated") == 5 {
        say("You have defeated 5 slimes! You feel stronger... (not really)");
    }
}

// The town guard patrols the road (see his patrol route in the map inspector). Once the
// shrine is lit he stops: stop_patrol(target) / patrol(target) / patrol(target, [[x, y], ...]).
fn guard_talk() {
    if flag("shrine_lit") {
        say("Guard", "The lights are gone - I can finally take a rest. Thank you, {player}!");
        stop_patrol("guard");
    } else {
        say("Guard", "Halt! Oh, it's you, {player}. Careful in Whisperwood, {sir|ma'am} - strange lights out there.");
    }
}

fn count_coins() {
    say("You have " + item_count("coin") + " coins.");
}
