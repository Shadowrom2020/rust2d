//! Ids inside Rhai scripts: string literals passed to API functions that expect an item,
//! map, sound, quest, ... (`give_item("key", 1)`, `teleport("forest", "west")`).
//!
//! A small scanner (not a full Rhai parser) finds calls and their string arguments, skipping
//! comments and other strings. Renames use it to update scripts; the editor's script check
//! uses it to report ids that don't exist.

use std::collections::HashMap;

use crate::defs::Database;
use crate::refactor::RefKind;

/// What kind of id an API argument is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdKind {
    Item,
    Map,
    /// A spawn point on the map given by the previous argument.
    Spawn,
    Sound,
    Quest,
    /// A prop state name.
    State,
}

impl IdKind {
    pub fn label(self) -> &'static str {
        match self {
            IdKind::Item => "item",
            IdKind::Map => "map",
            IdKind::Spawn => "spawn point",
            IdKind::Sound => "sound",
            IdKind::Quest => "quest",
            IdKind::State => "prop state",
        }
    }
    /// The definition kind renamed together with this id kind.
    pub fn ref_kind(self) -> Option<RefKind> {
        match self {
            IdKind::Item => Some(RefKind::Item),
            IdKind::Map => Some(RefKind::Map),
            IdKind::Sound => Some(RefKind::Sound),
            _ => None,
        }
    }
}

/// Script API functions with id arguments: (function, argument index, kind).
pub const API_IDS: &[(&str, usize, IdKind)] = &[
    ("give_item", 0, IdKind::Item),
    ("take_item", 0, IdKind::Item),
    ("has_item", 0, IdKind::Item),
    ("item_count", 0, IdKind::Item),
    ("teleport", 0, IdKind::Map),
    ("teleport", 1, IdKind::Spawn),
    ("play_sound", 0, IdKind::Sound),
    ("play_music", 0, IdKind::Sound),
    ("start_quest", 0, IdKind::Quest),
    ("advance_quest", 0, IdKind::Quest),
    ("set_quest_stage", 0, IdKind::Quest),
    ("complete_quest", 0, IdKind::Quest),
    ("fail_quest", 0, IdKind::Quest),
    ("quest_stage", 0, IdKind::Quest),
    ("quest_active", 0, IdKind::Quest),
    ("quest_done", 0, IdKind::Quest),
    ("set_state", 1, IdKind::State),
];

/// A string literal argument of a function call.
#[derive(Clone, Debug, PartialEq)]
pub struct StrArg {
    pub func: String,
    pub arg: usize,
    pub value: String,
    /// Byte range of the literal including its quotes.
    pub span: (usize, usize),
    /// 1-based line.
    pub line: usize,
    /// String literals passed as the other arguments of the same call (for `Spawn`).
    pub siblings: Vec<Option<String>>,
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Ident(String),
    Str(String, usize, usize),
    Punct(char),
    Other,
}

/// Splits Rhai source into the tokens the scanner cares about (with byte offsets).
fn tokenize(src: &str) -> Vec<(Tok, usize)> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            // Block comments nest in Rhai.
            let mut depth = 0;
            while i < b.len() {
                if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
                    depth += 1;
                    i += 2;
                } else if b[i] == b'*' && b.get(i + 1) == Some(&b'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else if c == b'"' {
            let start = i;
            let mut value = String::new();
            i += 1;
            while i < b.len() && b[i] != b'"' {
                if b[i] == b'\\' && i + 1 < b.len() {
                    let e = b[i + 1];
                    value.push(match e {
                        b'n' => '\n',
                        b't' => '\t',
                        _ => e as char,
                    });
                    i += 2;
                } else {
                    let ch = src[i..].chars().next().unwrap();
                    value.push(ch);
                    i += ch.len_utf8();
                }
            }
            i += 1;
            out.push((Tok::Str(value, start, i.min(b.len())), start));
        } else if c == b'`' || c == b'\'' {
            // Interpolated strings and characters: skipped, not ids.
            let q = c;
            let start = i;
            i += 1;
            while i < b.len() && b[i] != q {
                i += if b[i] == b'\\' { 2 } else { 1 };
            }
            i += 1;
            out.push((Tok::Other, start));
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push((Tok::Ident(src[start..i].to_string()), start));
        } else if c.is_ascii_whitespace() {
            i += 1;
        } else if matches!(c, b'(' | b')' | b',' | b'[' | b']' | b'{' | b'}') {
            out.push((Tok::Punct(c as char), i));
            i += 1;
        } else {
            out.push((Tok::Other, i));
            i += src[i..].chars().next().map_or(1, |ch| ch.len_utf8());
        }
    }
    out
}

/// Every string literal that is a whole argument of a function call.
pub fn string_args(src: &str) -> Vec<StrArg> {
    let toks = tokenize(src);
    let line_of = |pos: usize| src[..pos].bytes().filter(|&b| b == b'\n').count() + 1;
    let mut out = Vec::new();
    for k in 0..toks.len() {
        let (Tok::Ident(name), _) = &toks[k] else { continue };
        if toks.get(k + 1).map(|t| &t.0) != Some(&Tok::Punct('(')) {
            continue;
        }
        if k > 0 && toks[k - 1].0 == Tok::Ident("fn".into()) {
            continue; // a definition, not a call
        }
        // Collect the call's arguments (tokens at nesting depth 1, split by commas).
        let mut args: Vec<Vec<&Tok>> = vec![Vec::new()];
        let mut depth = 0;
        for (t, _) in &toks[k + 1..] {
            match t {
                Tok::Punct('(' | '[' | '{') => {
                    depth += 1;
                    if depth == 1 {
                        continue;
                    }
                }
                Tok::Punct(')' | ']' | '}') => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                Tok::Punct(',') if depth == 1 => {
                    args.push(Vec::new());
                    continue;
                }
                _ => {}
            }
            args.last_mut().unwrap().push(t);
        }
        let literal = |a: &Vec<&Tok>| match a.as_slice() {
            [Tok::Str(v, s, e)] => Some((v.clone(), *s, *e)),
            _ => None,
        };
        let siblings: Vec<Option<String>> = args.iter().map(|a| literal(a).map(|l| l.0)).collect();
        for (n, a) in args.iter().enumerate() {
            if let Some((value, s, e)) = literal(a) {
                out.push(StrArg { func: name.clone(), arg: n, value, span: (s, e), line: line_of(s), siblings: siblings.clone() });
            }
        }
    }
    out
}

/// The id arguments of known API calls, with their kind.
pub fn id_refs(src: &str) -> Vec<(StrArg, IdKind)> {
    string_args(src)
        .into_iter()
        .filter_map(|a| API_IDS.iter().find(|(f, n, _)| *f == a.func && *n == a.arg).map(|(_, _, k)| (a, *k)))
        .collect()
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Renames an id in a script: only string literals that are whole arguments of API calls
/// expecting that kind of id, and only exact matches. Returns the new source and the lines
/// that changed (empty = unchanged).
pub fn rename_in_script(src: &str, kind: RefKind, old: &str, new: &str) -> (String, Vec<usize>) {
    let mut hits: Vec<(usize, usize, usize)> = id_refs(src)
        .into_iter()
        .filter(|(a, k)| k.ref_kind() == Some(kind) && a.value == old)
        .map(|(a, _)| (a.span.0, a.span.1, a.line))
        .collect();
    if hits.is_empty() {
        return (src.to_string(), Vec::new());
    }
    hits.sort();
    let mut out = src.to_string();
    for (s, e, _) in hits.iter().rev() {
        out.replace_range(*s..*e, &quote(new));
    }
    let mut lines: Vec<usize> = hits.iter().map(|h| h.2).collect();
    lines.dedup();
    (out, lines)
}

/// A problem found by [`check_script`].
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptIssue {
    pub line: usize,
    pub message: String,
    /// Byte range of the offending string literal.
    pub span: (usize, usize),
}

/// Reports ids in API calls that don't exist in the project. `spawns`: map -> spawn names.
pub fn check_script(src: &str, db: &Database, maps: &[String], spawns: &HashMap<String, Vec<String>>) -> Vec<ScriptIssue> {
    let states: Vec<String> = db.props.iter().flat_map(|p| p.state_names()).collect();
    let mut out = Vec::new();
    for (a, kind) in id_refs(src) {
        let v = a.value.as_str();
        let known = match kind {
            IdKind::Item => db.item(v).is_some(),
            IdKind::Map => maps.iter().any(|m| m == v),
            IdKind::Spawn => match a.siblings.first().cloned().flatten() {
                Some(map) if maps.contains(&map) => spawns.get(&map).is_some_and(|s| s.iter().any(|x| x == v)),
                _ => true, // map unknown or computed: can't tell
            },
            IdKind::Sound => db.sound(v).is_some(),
            IdKind::Quest => db.quest(v).is_some(),
            IdKind::State => states.iter().any(|s| s == v),
        };
        if !known {
            let extra = match kind {
                IdKind::Spawn => format!(" on map '{}'", a.siblings.first().cloned().flatten().unwrap_or_default()),
                _ => String::new(),
            };
            out.push(ScriptIssue { line: a.line, message: format!("{}(\"{v}\"): unknown {}{extra}", a.func, kind.label()), span: a.span });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = r#"
// give_item("key", 1) in a comment stays
fn reward() {
    give_item("key", 1);   /* nested /* comment */ give_item("key") */
    say("Here is your key");            // not an id argument
    if has_item( "key" ) && item_count("keys") > 0 { teleport("village", "start"); }
    let k = "key";
    take_item(k, 1);
    set_state("gate", "open");
    play_sound(`key`);
}
"#;

    #[test]
    fn finds_id_arguments() {
        let refs = id_refs(SRC);
        let got: Vec<(String, IdKind, usize)> = refs.iter().map(|(a, k)| (a.value.clone(), *k, a.line)).collect();
        assert_eq!(
            got,
            vec![
                ("key".into(), IdKind::Item, 4),
                ("key".into(), IdKind::Item, 6),
                ("keys".into(), IdKind::Item, 6),
                ("village".into(), IdKind::Map, 6),
                ("start".into(), IdKind::Spawn, 6),
                ("open".into(), IdKind::State, 9),
            ]
        );
    }

    #[test]
    fn renames_only_id_arguments() {
        let (out, lines) = rename_in_script(SRC, RefKind::Item, "key", "gate_key");
        assert_eq!(lines, vec![4, 6]);
        assert!(out.contains(r#"give_item("gate_key", 1)"#) && out.contains(r#"has_item( "gate_key" )"#));
        // Comments, other strings, variables and other ids are untouched.
        assert!(out.contains(r#"// give_item("key", 1) in a comment"#) && out.contains(r#"give_item("key") */"#));
        assert!(out.contains(r#"say("Here is your key")"#) && out.contains(r#"let k = "key";"#) && out.contains(r#"item_count("keys")"#));
        assert_eq!(rename_in_script(SRC, RefKind::Map, "village", "town").1, vec![6]);
        assert!(rename_in_script(SRC, RefKind::Sound, "key", "x").1.is_empty());
    }

    #[test]
    fn example_script_has_no_unknown_ids() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example_project");
        let project = crate::project::Project::load(&root).unwrap();
        let maps = project.list_maps();
        let spawns: HashMap<String, Vec<String>> = maps
            .iter()
            .map(|m| {
                let map = project.load_map(m).unwrap();
                (m.clone(), map.zones.iter().map(|z| z.name.clone()).collect())
            })
            .collect();
        let src = std::fs::read_to_string(root.join("scripts/main.rhai")).unwrap();
        assert_eq!(check_script(&src, &project.db, &maps, &spawns), vec![]);
    }

    #[test]
    fn reports_unknown_ids() {
        let mut db = Database::default();
        db.items.push(crate::defs::ItemDef {
            id: "key".into(),
            name: "Key".into(),
            description: String::new(),
            icon: crate::defs::SpriteRef::default(),
            consumable: false,
            on_use: vec![],
            weapon: None,
            armor: None,
            max_stack: 99,
            value: 0,
        light: None,
        });
        let maps = vec!["village".to_string()];
        let spawns: HashMap<String, Vec<String>> = [("village".to_string(), vec!["gate".to_string()])].into();
        let issues = check_script(SRC, &db, &maps, &spawns);
        let msgs: Vec<String> = issues.iter().map(|i| format!("{}: {}", i.line, i.message)).collect();
        assert_eq!(
            msgs,
            vec![
                "6: item_count(\"keys\"): unknown item".to_string(),
                "6: teleport(\"start\"): unknown spawn point on map 'village'".to_string(),
                "9: set_state(\"open\"): unknown prop state".to_string(),
            ]
        );
    }
}
