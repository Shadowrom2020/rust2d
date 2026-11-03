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
// Other:        player_hp() random(n) current_map()
// Targets: "self" (trigger owner), "player", or an entity name.

fn mira_talk() {
    add_var("mira_talks", 1);
    let n = get_var("mira_talks");
    if n == 1 {
        say("Mira", "Hi! I'm Mira. Have you met Elder Rowan by the well?");
    } else if flag("shrine_lit") {
        say("Mira", "You lit the shrine? Amazing!");
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
    emit("shrine_lit");
}

// Called by the slime's "Defeated" trigger.
// Goblins drop a few arrows.
fn goblin_defeated() {
    give_item("arrows", 1 + random(3));
}

fn slime_defeated() {
    add_var("slimes_defeated", 1);
    if random(2) == 0 {
        give_item("coin", 1 + random(3));
    }
    if get_var("slimes_defeated") == 5 {
        say("You have defeated 5 slimes! You feel stronger... (not really)");
    }
}

fn count_coins() {
    say("You have " + item_count("coin") + " coins.");
}
