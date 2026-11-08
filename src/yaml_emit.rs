//! A tiny, hand-rolled YAML emitter for JSON values.
//!
//! Hand-rolled rather than delegated to a YAML crate on purpose: the emitted
//! bytes are part of the engine's output contract, and a dependency upgrade
//! must not be able to change them. The surface is therefore small — it only
//! ever sees JSON values — and every golden is pinned in the tests below.
//!
//! Style rules:
//!
//!   * objects and arrays are block style, one entry per line;
//!   * object keys are sorted recursively, so output does not depend on the
//!     input's key order;
//!   * empty objects and arrays are inline (`{}`, `[]`), where block style
//!     would mean something else;
//!   * strings are plain when that is unambiguous, single-quoted when YAML
//!     could read the text as a different scalar, and double-quoted when the
//!     text carries characters that single quoting cannot express literally.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

const INDENT: usize = 2;

/// Emit a JSON object as YAML block entries at indent 0.
pub fn mapping_to_yaml(map: &BTreeMap<String, Value>) -> String {
    let mut out = String::new();
    emit_entries(map.iter().map(|(k, v)| (k.as_str(), v)), 0, &mut out);
    out
}

/// Emit any JSON value as YAML.
pub fn to_yaml(value: &Value) -> String {
    let mut out = String::new();
    match value {
        Value::Object(map) if map.is_empty() => out.push_str("{}\n"),
        Value::Object(map) => emit_entries(sorted(map).into_iter(), 0, &mut out),
        Value::Array(items) if items.is_empty() => out.push_str("[]\n"),
        Value::Array(items) => emit_array(items, 0, &mut out),
        other => {
            out.push_str(&scalar(other));
            out.push('\n');
        }
    }
    out
}

// --------------------------------------------------------------- emitters

fn sorted(map: &Map<String, Value>) -> Vec<(&str, &Value)> {
    let mut entries: Vec<(&str, &Value)> = map.iter().map(|(k, v)| (k.as_str(), v)).collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    entries
}

fn emit_entries<'a>(
    entries: impl Iterator<Item = (&'a str, &'a Value)>,
    indent: usize,
    out: &mut String,
) {
    for (key, value) in entries {
        emit_entry(key, value, indent, out);
    }
}

/// One value at `indent`, introduced by `lead`: a mapping key with its
/// colon, or an array dash. A non-empty collection opens a block beneath
/// the lead; anything else lands on the lead's own line.
fn emit(lead: &str, value: &Value, indent: usize, out: &mut String) {
    let pad = " ".repeat(indent);
    match value {
        Value::Object(map) if !map.is_empty() => {
            out.push_str(&pad);
            out.push_str(lead);
            out.push('\n');
            emit_entries(sorted(map).into_iter(), indent + INDENT, out);
        }
        Value::Array(items) if !items.is_empty() => {
            out.push_str(&pad);
            out.push_str(lead);
            out.push('\n');
            emit_array(items, indent + INDENT, out);
        }
        other => {
            out.push_str(&pad);
            out.push_str(lead);
            out.push(' ');
            out.push_str(&scalar(other));
            out.push('\n');
        }
    }
}

fn emit_entry(key: &str, value: &Value, indent: usize, out: &mut String) {
    emit(&format!("{}:", string_scalar(key)), value, indent, out);
}

fn emit_array(items: &[Value], indent: usize, out: &mut String) {
    for item in items {
        emit("-", item, indent, out);
    }
}

// ---------------------------------------------------------------- scalars

/// A value that fits on one line: a scalar, or an empty collection.
fn scalar(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => string_scalar(text),
        Value::Array(_) => "[]".to_string(),
        Value::Object(_) => "{}".to_string(),
    }
}

/// A string in whichever YAML scalar style keeps its meaning.
fn string_scalar(text: &str) -> String {
    if text.is_empty() {
        return "''".to_string();
    }
    if text.chars().any(char::is_control) {
        return double_quote(text);
    }
    if needs_quoting(text) {
        return single_quote(text);
    }
    text.to_string()
}

fn single_quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('\'');
    for character in text.chars() {
        if character == '\'' {
            out.push_str("''");
        } else {
            out.push(character);
        }
    }
    out.push('\'');
    out
}

fn double_quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for character in text.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c == '\u{7f}' => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Indicators: a plain scalar may not begin with these.
const INDICATORS: &[char] = &[
    '!', '&', '*', '?', '|', '>', '%', '@', '`', '"', '\'', '#', '-', ':', ',', '[', ']', '{', '}',
];

/// True when YAML would read the text as something other than this string.
fn needs_quoting(text: &str) -> bool {
    if text.starts_with(' ') || text.ends_with(' ') {
        return true;
    }
    if text.starts_with(INDICATORS) {
        return true;
    }
    // `: ` ends the key, and a trailing `:` does too.
    if text.contains(": ") || text.ends_with(':') {
        return true;
    }
    // ` #` starts a comment.
    if text.contains(" #") {
        return true;
    }
    looks_like_another_scalar(text)
}

/// Strings that a YAML loader would turn into null, a boolean or a number.
///
/// The single letters `y` and `n` are in this set despite being ugly map
/// keys: YAML 1.1 (PyYAML and friends) reads them as booleans, so leaving
/// them plain would round-trip as `true`/`false`. YAML 1.2 is not enough to
/// rely on here.
fn looks_like_another_scalar(text: &str) -> bool {
    if text == "~" {
        return true;
    }
    if matches!(
        text,
        "null"
            | "Null"
            | "NULL"
            | "true"
            | "True"
            | "TRUE"
            | "false"
            | "False"
            | "FALSE"
            | "yes"
            | "Yes"
            | "YES"
            | "no"
            | "No"
            | "NO"
            | "on"
            | "On"
            | "ON"
            | "off"
            | "Off"
            | "OFF"
            | "y"
            | "Y"
            | "n"
            | "N"
    ) {
        return true;
    }
    if text.parse::<i64>().is_ok() || text.parse::<u64>().is_ok() || text.parse::<f64>().is_ok() {
        return true;
    }
    // YAML 1.1 spellings of infinities and NaN, and hex/octal/binary ints
    // that `str::parse` does not recognise.
    let lower = text.to_ascii_lowercase();
    if matches!(lower.as_str(), ".inf" | "-.inf" | ".nan") {
        return true;
    }
    let digits = lower.trim_start_matches(['-', '+']);
    digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0o"))
        .or_else(|| digits.strip_prefix("0b"))
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_hexdigit()))
}

// ----------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn emit(value: Value) -> String {
        to_yaml(&value)
    }

    fn emit_mapping(value: Value) -> String {
        let map = value.as_object().expect("a mapping").clone();
        mapping_to_yaml(&map.into_iter().collect())
    }

    // The three strings a consumer's frontmatter is most likely to hit.

    #[test]
    fn a_flat_mapping_is_sorted_and_unquoted() {
        assert_eq!(
            emit_mapping(json!({"team": "platform", "owner": "default"})),
            "owner: default\nteam: platform\n"
        );
    }

    #[test]
    fn a_colon_and_space_quotes_the_value() {
        assert_eq!(
            emit_mapping(json!({"description": "a: b"})),
            "description: 'a: b'\n"
        );
    }

    #[test]
    fn a_plain_value_stays_plain() {
        assert_eq!(emit_mapping(json!({"scope": "skills"})), "scope: skills\n");
    }

    // Structural goldens.

    #[test]
    fn an_empty_mapping_emits_nothing() {
        assert_eq!(mapping_to_yaml(&BTreeMap::new()), "");
    }

    #[test]
    fn keys_are_sorted_recursively() {
        let value = json!({"b": 1, "a": {"z": 1, "w": 2}});
        assert_eq!(emit(value), "a:\n  w: 2\n  z: 1\nb: 1\n");
    }

    #[test]
    fn keys_that_look_like_booleans_are_quoted() {
        assert_eq!(
            emit(json!({"y": 1, "on": 2, "n": 3})),
            "'n': 3\n'on': 2\n'y': 1\n"
        );
    }

    #[test]
    fn scalars_on_one_line() {
        assert_eq!(emit(json!(null)), "null\n");
        assert_eq!(emit(json!(true)), "true\n");
        assert_eq!(emit(json!(false)), "false\n");
        assert_eq!(emit(json!(42)), "42\n");
        assert_eq!(emit(json!(1.5)), "1.5\n");
    }

    #[test]
    fn empty_collections_are_inline() {
        assert_eq!(emit(json!({"a": {}})), "a: {}\n");
        assert_eq!(emit(json!({"a": []})), "a: []\n");
    }

    #[test]
    fn a_nested_object_uses_block_style() {
        let value = json!({"outer": {"inner": 1, "other": "two"}});
        assert_eq!(emit(value), "outer:\n  inner: 1\n  other: two\n");
    }

    #[test]
    fn a_nested_array_uses_block_style() {
        let value = json!({"items": ["a", "b"]});
        assert_eq!(emit(value), "items:\n  - a\n  - b\n");
    }

    #[test]
    fn an_array_of_objects_puts_the_object_on_its_own_block() {
        let value = json!({"items": [{"a": 1, "b": 2}]});
        assert_eq!(emit(value), "items:\n  -\n    a: 1\n    b: 2\n");
    }

    #[test]
    fn an_array_of_arrays_is_unambiguous() {
        let value = json!({"items": [[1, 2]]});
        assert_eq!(emit(value), "items:\n  -\n    - 1\n    - 2\n");
    }

    #[test]
    fn mixed_nesting_is_deterministic() {
        let value = json!({
            "z": [{"k": {"w": 1, "x": 2}}, "plain"],
            "a": {"list": [true, null]}
        });
        assert_eq!(
            emit(value),
            "a:\n  list:\n    - true\n    - null\nz:\n  -\n    k:\n      w: 1\n      x: 2\n  - plain\n"
        );
    }

    // String quoting.

    #[test]
    fn empty_strings_are_quoted() {
        assert_eq!(emit(json!("")), "''\n");
    }

    #[test]
    fn leading_or_trailing_space_quotes() {
        assert_eq!(emit(json!(" a")), "' a'\n");
        assert_eq!(emit(json!("a ")), "'a '\n");
    }

    #[test]
    fn indicators_quote_the_value() {
        assert_eq!(emit(json!("-x")), "'-x'\n");
        assert_eq!(emit(json!("#x")), "'#x'\n");
        assert_eq!(emit(json!("*x")), "'*x'\n");
        assert_eq!(emit(json!(">x")), "'>x'\n");
    }

    #[test]
    fn a_trailing_colon_quotes_the_value() {
        assert_eq!(emit(json!("key:")), "'key:'\n");
    }

    #[test]
    fn a_hash_comment_starter_quotes_the_value() {
        assert_eq!(emit(json!("a #b")), "'a #b'\n");
    }

    #[test]
    fn a_hash_inside_a_word_does_not() {
        assert_eq!(emit(json!("a#b")), "a#b\n");
    }

    #[test]
    fn a_colon_without_a_space_does_not() {
        assert_eq!(emit(json!("a:b")), "a:b\n");
    }

    #[test]
    fn scalars_yaml_would_misread_are_quoted() {
        assert_eq!(emit(json!("true")), "'true'\n");
        assert_eq!(emit(json!("null")), "'null'\n");
        assert_eq!(emit(json!("yes")), "'yes'\n");
        assert_eq!(emit(json!("y")), "'y'\n");
        assert_eq!(emit(json!("on")), "'on'\n");
        assert_eq!(emit(json!("12")), "'12'\n");
        assert_eq!(emit(json!("1.5")), "'1.5'\n");
        assert_eq!(emit(json!("~")), "'~'\n");
        assert_eq!(emit(json!("0x1f")), "'0x1f'\n");
    }

    #[test]
    fn an_inner_apostrophe_is_doubled_when_the_value_is_quoted() {
        assert_eq!(emit(json!("'quoted'")), "'''quoted'''\n");
        assert_eq!(emit(json!("don't: stop")), "'don''t: stop'\n");
    }

    #[test]
    fn an_apostrophe_inside_a_word_does_not() {
        assert_eq!(emit(json!("it's")), "it's\n");
    }

    #[test]
    fn control_characters_use_double_quotes() {
        assert_eq!(emit(json!("a\nb")), "\"a\\nb\"\n");
        assert_eq!(emit(json!("tab\there")), "\"tab\\there\"\n");
    }

    #[test]
    fn quotes_inside_a_word_do_not() {
        // A plain scalar may carry `"` as long as it does not start with one.
        assert_eq!(emit(json!("say \"hi\"")), "say \"hi\"\n");
        assert_eq!(emit(json!("it's fine")), "it's fine\n");
    }

    #[test]
    fn ordinary_prose_stays_plain() {
        assert_eq!(emit(json!("Keep secrets safe.")), "Keep secrets safe.\n");
    }
}
