//! Status effects: timed conditions on the player and on characters (poison, burning,
//! regeneration, slow, haste, stun, strength, weakness, shield). Weapons apply them on hit,
//! characters by touch, and the *Apply effect* / *Cure effect* actions (potions, traps, spells,
//! scripts) by hand. The player's effects are in the save game (`WorldState::effects`); the
//! runtime part (ticking, drawing) is `runtime/effects.rs`.

use serde::{Deserialize, Serialize};

/// The built-in effects. New kinds are appended (their index is stored in maps and definitions).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EffectKind {
    /// Loses `strength` HP per second (ignores defense).
    Poison,
    /// Like poison, from fire.
    Burn,
    /// Regains `strength` HP per second.
    Regen,
    /// Moves `strength` percent slower.
    Slow,
    /// Moves `strength` percent faster.
    Haste,
    /// Can't move or attack.
    Stun,
    /// Deals `strength` percent more damage.
    Strength,
    /// Deals `strength` percent less damage.
    Weakness,
    /// `strength` more defense.
    Shield,
}

impl EffectKind {
    pub const ALL: [EffectKind; 9] = [
        EffectKind::Poison,
        EffectKind::Burn,
        EffectKind::Regen,
        EffectKind::Slow,
        EffectKind::Haste,
        EffectKind::Stun,
        EffectKind::Strength,
        EffectKind::Weakness,
        EffectKind::Shield,
    ];

    /// The name used in scripts (`apply_effect("player", "poison", 1, 6)`).
    pub fn id(self) -> &'static str {
        match self {
            EffectKind::Poison => "poison",
            EffectKind::Burn => "burn",
            EffectKind::Regen => "regen",
            EffectKind::Slow => "slow",
            EffectKind::Haste => "haste",
            EffectKind::Stun => "stun",
            EffectKind::Strength => "strength",
            EffectKind::Weakness => "weakness",
            EffectKind::Shield => "shield",
        }
    }

    pub fn from_id(id: &str) -> Option<EffectKind> {
        let id = id.trim().to_ascii_lowercase();
        EffectKind::ALL.into_iter().find(|k| k.id() == id)
    }

    /// The name shown in the editor.
    pub fn label(self) -> &'static str {
        match self {
            EffectKind::Poison => "Poison",
            EffectKind::Burn => "Burning",
            EffectKind::Regen => "Regeneration",
            EffectKind::Slow => "Slow",
            EffectKind::Haste => "Haste",
            EffectKind::Stun => "Stun",
            EffectKind::Strength => "Strength",
            EffectKind::Weakness => "Weakness",
            EffectKind::Shield => "Shield",
        }
    }

    /// The badge text in the game (ASCII: the game font has nothing else).
    pub fn badge(self) -> &'static str {
        match self {
            EffectKind::Poison => "PSN",
            EffectKind::Burn => "BRN",
            EffectKind::Regen => "REG",
            EffectKind::Slow => "SLW",
            EffectKind::Haste => "HST",
            EffectKind::Stun => "STN",
            EffectKind::Strength => "STR",
            EffectKind::Weakness => "WK",
            EffectKind::Shield => "SHD",
        }
    }

    /// The badge color.
    pub fn color(self) -> [u8; 3] {
        match self {
            EffectKind::Poison => [90, 190, 60],
            EffectKind::Burn => [240, 120, 30],
            EffectKind::Regen => [230, 80, 120],
            EffectKind::Slow => [90, 130, 200],
            EffectKind::Haste => [80, 200, 210],
            EffectKind::Stun => [150, 150, 160],
            EffectKind::Strength => [210, 60, 50],
            EffectKind::Weakness => [130, 100, 150],
            EffectKind::Shield => [200, 170, 70],
        }
    }

    /// What `strength` means, for the editor.
    pub fn unit(self) -> &'static str {
        match self {
            EffectKind::Poison | EffectKind::Burn => "HP lost per second",
            EffectKind::Regen => "HP regained per second",
            EffectKind::Slow | EffectKind::Haste | EffectKind::Strength | EffectKind::Weakness => "percent",
            EffectKind::Shield => "defense points",
            EffectKind::Stun => "(not used)",
        }
    }

    /// Whether the effect is bad for whoever has it (the editor and the HUD order by it).
    pub fn harmful(self) -> bool {
        matches!(self, EffectKind::Poison | EffectKind::Burn | EffectKind::Slow | EffectKind::Stun | EffectKind::Weakness)
    }

    /// A sensible strength for a new effect of this kind.
    pub fn default_strength(self) -> f32 {
        match self {
            EffectKind::Poison | EffectKind::Burn => 0.5,
            EffectKind::Regen => 0.5,
            EffectKind::Slow | EffectKind::Haste | EffectKind::Strength | EffectKind::Weakness => 30.0,
            EffectKind::Shield => 2.0,
            EffectKind::Stun => 0.0,
        }
    }
}

/// An effect to apply: what, how strong, for how long.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EffectSpec {
    pub kind: EffectKind,
    /// See [`EffectKind`] (per second, percent or points).
    #[serde(default)]
    pub strength: f32,
    pub seconds: f32,
}

impl EffectSpec {
    pub fn new(kind: EffectKind, strength: f32, seconds: f32) -> Self {
        EffectSpec { kind, strength, seconds }
    }
    /// A new effect with the kind's default strength, for the editor.
    pub fn template(kind: EffectKind) -> Self {
        EffectSpec { kind, strength: kind.default_strength(), seconds: if kind == EffectKind::Stun { 1.5 } else { 6.0 } }
    }
}

/// An effect that is running.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActiveEffect {
    pub kind: EffectKind,
    pub strength: f32,
    /// Seconds left.
    pub remaining: f32,
    /// Fractions of HP not yet lost / regained (poison, burning, regeneration).
    #[serde(default)]
    pub accum: f32,
}

/// Starts an effect. Applying a kind that is already running restarts it with the longer of
/// the two durations and the stronger of the two strengths.
pub fn apply(list: &mut Vec<ActiveEffect>, spec: &EffectSpec) {
    if spec.seconds <= 0.0 {
        return;
    }
    match list.iter_mut().find(|e| e.kind == spec.kind) {
        Some(e) => {
            e.remaining = e.remaining.max(spec.seconds);
            e.strength = e.strength.max(spec.strength);
        }
        None => list.push(ActiveEffect { kind: spec.kind, strength: spec.strength, remaining: spec.seconds, accum: 0.0 }),
    }
}

/// Ends the effects of a kind (`None` = all). Returns how many ended.
pub fn cure(list: &mut Vec<ActiveEffect>, kind: Option<EffectKind>) -> usize {
    let before = list.len();
    list.retain(|e| kind.is_some_and(|k| k != e.kind));
    before - list.len()
}

/// Advances the effects by `dt` seconds and drops the ones that ran out. Returns the whole HP
/// change of this step: negative = damage (poison, burning), positive = healing (regeneration).
pub fn tick(list: &mut Vec<ActiveEffect>, dt: f32) -> i32 {
    let mut hp = 0;
    for e in list.iter_mut() {
        let step = dt.min(e.remaining.max(0.0));
        let sign = match e.kind {
            EffectKind::Poison | EffectKind::Burn => -1,
            EffectKind::Regen => 1,
            _ => 0,
        };
        if sign != 0 {
            e.accum += e.strength.max(0.0) * step;
            let whole = e.accum.floor();
            e.accum -= whole;
            hp += sign * whole as i32;
        }
        e.remaining -= dt;
    }
    list.retain(|e| e.remaining > 0.0);
    hp
}

fn sum(list: &[ActiveEffect], kind: EffectKind) -> f32 {
    list.iter().filter(|e| e.kind == kind).map(|e| e.strength).sum()
}

/// Movement speed factor (slow and haste; stun is [`stunned`]).
pub fn speed_factor(list: &[ActiveEffect]) -> f32 {
    (1.0 + (sum(list, EffectKind::Haste) - sum(list, EffectKind::Slow)) / 100.0).clamp(0.1, 3.0)
}

/// Damage factor (strength and weakness).
pub fn damage_factor(list: &[ActiveEffect]) -> f32 {
    (1.0 + (sum(list, EffectKind::Strength) - sum(list, EffectKind::Weakness)) / 100.0).clamp(0.0, 5.0)
}

/// Scales a damage value by [`damage_factor`]; damage that was there stays at least 1.
pub fn scale_damage(list: &[ActiveEffect], damage: i32) -> i32 {
    if damage <= 0 || list.is_empty() {
        return damage;
    }
    ((damage as f32 * damage_factor(list)).round() as i32).max(1)
}

/// Extra defense (shield).
pub fn defense_bonus(list: &[ActiveEffect]) -> i32 {
    sum(list, EffectKind::Shield).round() as i32
}

pub fn stunned(list: &[ActiveEffect]) -> bool {
    list.iter().any(|e| e.kind == EffectKind::Stun)
}

/// For `status()`: `poison:0.5:4.2` per effect.
pub fn describe(list: &[ActiveEffect]) -> Vec<String> {
    list.iter().map(|e| format!("{}:{}:{:.1}", e.kind.id(), e.strength, e.remaining)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poison_ticks_and_ends() {
        let mut l = Vec::new();
        apply(&mut l, &EffectSpec::new(EffectKind::Poison, 0.5, 4.0));
        let mut lost = 0;
        for _ in 0..50 {
            lost += tick(&mut l, 0.1);
        }
        // 0.5 HP per second for 4 seconds = 2 HP, then it is over.
        assert_eq!(lost, -2);
        assert!(l.is_empty());
    }

    #[test]
    fn reapply_refreshes() {
        let mut l = Vec::new();
        apply(&mut l, &EffectSpec::new(EffectKind::Slow, 30.0, 5.0));
        tick(&mut l, 3.0);
        apply(&mut l, &EffectSpec::new(EffectKind::Slow, 20.0, 4.0));
        assert_eq!(l.len(), 1);
        assert_eq!((l[0].strength, l[0].remaining), (30.0, 4.0));
        apply(&mut l, &EffectSpec::new(EffectKind::Regen, 1.0, 0.0));
        assert_eq!(l.len(), 1, "zero seconds = nothing");
    }

    #[test]
    fn modifiers() {
        let mut l = Vec::new();
        assert_eq!((speed_factor(&l), damage_factor(&l), defense_bonus(&l), stunned(&l)), (1.0, 1.0, 0, false));
        assert_eq!(scale_damage(&l, 3), 3);
        apply(&mut l, &EffectSpec::new(EffectKind::Slow, 40.0, 5.0));
        apply(&mut l, &EffectSpec::new(EffectKind::Weakness, 50.0, 5.0));
        apply(&mut l, &EffectSpec::new(EffectKind::Shield, 2.0, 5.0));
        apply(&mut l, &EffectSpec::new(EffectKind::Stun, 0.0, 1.0));
        assert!((speed_factor(&l) - 0.6).abs() < 1e-5);
        assert_eq!(scale_damage(&l, 1), 1, "weakened hits still hurt");
        assert_eq!(scale_damage(&l, 4), 2);
        assert_eq!(defense_bonus(&l), 2);
        assert!(stunned(&l));
        assert_eq!(cure(&mut l, Some(EffectKind::Stun)), 1);
        assert!(!stunned(&l));
        assert_eq!(cure(&mut l, None), 3);
    }

    #[test]
    fn ids_and_ron() {
        for k in EffectKind::ALL {
            assert_eq!(EffectKind::from_id(k.id()), Some(k));
            assert!(k.badge().is_ascii());
        }
        assert_eq!(EffectKind::from_id(" Poison "), Some(EffectKind::Poison));
        let a: ActiveEffect = ron::from_str("(kind: Burn, strength: 1.0, remaining: 2.0)").unwrap();
        assert_eq!(a.accum, 0.0);
    }
}
