//! Merge JSON chunks into a target.
//!
//! `injections.json` *ensures presence*: the chunk's content must be there,
//! and everything the target already carries is left alone.
//!
//! | target and chunk are | result |
//! |---|---|
//! | both objects | key-wise merge; keys only the target has are kept; shared keys merge recursively |
//! | both arrays | union: the target's items and order are kept, chunk items not already present are appended |
//! | anything else | the chunk wins |
//!
//! "Not already present" is element-wise: a chunk item counts as new unless
//! some item of the target compares equal to it, and a chunk item that is
//! itself an array is still one item.

use serde_json::{Map, Number, Value};

/// Ensure `chunk` is present in `target`.
pub fn merge(target: &Value, chunk: &Value) -> Value {
    match (target, chunk) {
        (Value::Object(target), Value::Object(chunk)) => {
            let mut out = target.clone();
            for (key, value) in chunk {
                let existing = out.get(key).cloned().unwrap_or(Value::Null);
                out.insert(key.clone(), merge(&existing, value));
            }
            Value::Object(out)
        }
        (Value::Array(target), Value::Array(chunk)) => union(target, chunk),
        _ => chunk.clone(),
    }
}

/// Union of two arrays: `target`'s items and order, then `chunk`'s novel items.
pub fn union(target: &[Value], chunk: &[Value]) -> Value {
    let mut out = target.to_vec();
    for item in chunk {
        if !out.iter().any(|existing| json_eq(existing, item)) {
            out.push(item.clone());
        }
    }
    Value::Array(out)
}

/// Fold `merge` over a chunk list, the way several `injections.json` entries
/// aimed at one file combine. The fold starts from an empty object, so a
/// chunk list behaves as one object built up left to right.
pub fn merge_chunks(chunks: impl IntoIterator<Item = Value>) -> Value {
    chunks
        .into_iter()
        .fold(Value::Object(Map::new()), |acc, chunk| merge(&acc, &chunk))
}

/// Would merging `chunk` leave `target` unchanged?
///
/// This is what `check` uses to decide whether a JSON injection target still
/// needs work.
pub fn is_noop(target: &Value, chunk: &Value) -> bool {
    json_eq(&merge(target, chunk), target)
}

// ------------------------------------------------------------- equality

/// JSON equality: numbers compare by value (so `1` and `1.0` are the same),
/// object key order is irrelevant, and everything else compares structurally.
pub fn json_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::String(a), Value::String(b)) => a == b,
        (Value::Number(a), Value::Number(b)) => number_eq(a, b),
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| json_eq(x, y))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(key, value)| b.get(key).is_some_and(|other| json_eq(value, other)))
        }
        _ => false,
    }
}

fn number_eq(a: &Number, b: &Number) -> bool {
    // Compare integers exactly first, so values beyond f64's 53 bits of
    // precision still distinguish.
    if let (Some(x), Some(y)) = (a.as_u64(), b.as_u64()) {
        return x == y;
    }
    if let (Some(x), Some(y)) = (a.as_i64(), b.as_i64()) {
        return x == y;
    }
    // At least one side is a float: compare numerically, so `1` and `1.0`
    // are the same value.
    matches!(
        (a.as_f64(), b.as_f64()),
        (Some(x), Some(y)) if x == y
    )
}

// ----------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn merge_json(target: Value, chunk: Value) -> Value {
        merge(&target, &chunk)
    }

    // The merge semantics, pinned.

    #[test]
    fn objects_merge_key_wise_and_keep_target_only_keys() {
        let target = json!({"user_only": "kept", "nested": {"b": 2}});
        let chunk = json!({"instructions": [".opencode/rules/**/*.md"], "nested": {"a": 1}});
        assert_eq!(
            merge_json(target, chunk),
            json!({
                "user_only": "kept",
                "instructions": [".opencode/rules/**/*.md"],
                "nested": {"a": 1, "b": 2}
            })
        );
    }

    #[test]
    fn array_union_keeps_target_order_and_appends_novel_items() {
        assert_eq!(
            merge_json(
                json!([".claude/rules/*.md"]),
                json!([".opencode/rules/**/*.md"])
            ),
            json!([".claude/rules/*.md", ".opencode/rules/**/*.md"])
        );
    }

    #[test]
    fn merging_the_same_chunk_twice_changes_nothing() {
        let target = json!({"instructions": ["a"], "nested": {"b": 2}});
        let chunk = json!({"instructions": ["a", "b"], "nested": {"a": 1}});
        let once = merge_json(target.clone(), chunk.clone());
        assert_eq!(merge_json(once.clone(), chunk), once);
    }

    #[test]
    fn a_scalar_chunk_is_enforced() {
        assert_eq!(
            merge_json(json!({"a": 1}), json!({"a": 2})),
            json!({"a": 2})
        );
        assert_eq!(merge_json(json!(1), json!(2)), json!(2));
    }

    #[test]
    fn an_empty_object_or_array_chunk_is_a_no_op() {
        assert!(is_noop(&json!({"a": 1}), &json!({})));
        assert!(is_noop(&json!([1, 2]), &json!([])));
    }

    #[test]
    fn a_type_mismatch_lets_the_chunk_win() {
        assert_eq!(merge_json(json!(1), json!({"a": 2})), json!({"a": 2}));
        assert_eq!(merge_json(json!({"a": 1}), json!(5)), json!(5));
        assert_eq!(merge_json(json!({"a": 1}), json!([5])), json!([5]));
        assert_eq!(merge_json(json!([1]), json!({"a": 1})), json!({"a": 1}));
    }

    // ------------------------------------------------------------- equality

    #[test]
    fn numbers_compare_by_value_not_representation() {
        assert!(json_eq(&json!(1), &json!(1.0)));
        assert!(json_eq(&json!(1.0), &json!(1.00)));
        assert!(!json_eq(&json!(1), &json!(1.5)));
        assert!(!json_eq(&json!(2), &json!(1.0)));
        // A float must not be truncated into an integer comparison.
        assert!(!json_eq(&json!(1), &json!(1.5)));
        assert_eq!(
            merge_json(json!({"n": [1]}), json!({"n": [1.0]})),
            json!({"n": [1]})
        );
    }

    #[test]
    fn object_key_order_does_not_affect_equality() {
        assert!(json_eq(&json!({"a": 1, "b": 2}), &json!({"b": 2, "a": 1})));
        assert!(!json_eq(&json!({"a": 1}), &json!({"a": 1, "b": 2})));
    }

    #[test]
    fn a_null_chunk_wins_over_anything() {
        assert_eq!(merge_json(json!({"a": 1}), json!(null)), json!(null));
        assert_eq!(merge_json(json!([1]), json!(null)), json!(null));
        assert_eq!(
            merge_json(json!({"a": {"x": 1}}), json!({"a": null})),
            json!({"a": null})
        );
    }

    #[test]
    fn a_missing_key_takes_the_chunk_value_whole() {
        assert_eq!(
            merge_json(json!({"a": 1}), json!({"b": {"c": [1, 2]}})),
            json!({"a": 1, "b": {"c": [1, 2]}})
        );
    }

    #[test]
    fn shared_keys_merge_recursively() {
        assert_eq!(
            merge_json(json!({"n": {"b": 2}}), json!({"n": {"a": 1}})),
            json!({"n": {"b": 2, "a": 1}})
        );
    }

    #[test]
    fn union_membership_is_element_wise_not_subsequence() {
        // A chunk item that is itself an array is still one item: it is
        // appended unless an item of the target compares equal to it.
        assert_eq!(
            merge_json(json!([1, 2, 3]), json!([[2]])),
            json!([1, 2, 3, [2]])
        );
    }

    #[test]
    fn an_item_already_present_is_not_duplicated() {
        assert_eq!(
            merge_json(json!(["a", "b"]), json!(["c", "a", "d"])),
            json!(["a", "b", "c", "d"])
        );
    }

    // -------------------------------------------------------- merge_chunks

    #[test]
    fn merge_chunks_folds_in_order() {
        assert_eq!(
            merge_chunks([
                json!({"a": 1, "shared": "first"}),
                json!({"b": 2, "shared": "second"})
            ]),
            json!({"a": 1, "b": 2, "shared": "second"})
        );
    }

    #[test]
    fn merge_chunks_of_nothing_is_an_empty_object() {
        assert_eq!(merge_chunks([]), json!({}));
    }

    #[test]
    fn merge_chunks_agrees_with_a_single_chunk() {
        let chunks = [json!({"a": 1, "l": ["x"]}), json!({"b": 2, "l": ["y"]})];
        let combined = merge_chunks(chunks.iter().cloned());
        assert_eq!(
            merge(&json!({"l": ["x", "z"]}), &combined),
            merge(&merge(&json!({"l": ["x", "z"]}), &chunks[0]), &chunks[1])
        );
    }

    // ------------------------------------------------------------- is_noop

    #[test]
    fn is_noop_detects_a_needed_change() {
        assert!(!is_noop(&json!({"user": 1}), &json!({"a": 1})));
        assert!(!is_noop(
            &json!({"instructions": ["a"]}),
            &json!({"instructions": ["a", "b"]})
        ));
    }

    #[test]
    fn is_noop_is_true_when_the_content_is_already_present() {
        assert!(is_noop(
            &json!({"user": 1, "instructions": ["a", "b"]}),
            &json!({"instructions": ["b"]})
        ));
    }

    // ------------------------------------------------------------ property

    /// Merging the same chunk again must change nothing, for a spread of
    /// shapes. This is the property `sync` relies on to be idempotent.
    #[test]
    fn merge_is_idempotent() {
        let cases = [
            (json!({}), json!({"a": 1})),
            (json!({"a": 1}), json!({"a": 2})),
            (json!({"a": {"b": 1}}), json!({"a": {"c": 2}})),
            (json!([1, 2]), json!([2, 3])),
            (json!({"l": []}), json!({"l": ["x"]})),
            (json!(1), json!({"a": 1})),
            (json!({"a": 1}), json!(null)),
            (json!([1, 2, 3]), json!([[2]])),
            (json!({"n": [1]}), json!({"n": [1.0]})),
        ];
        for (target, chunk) in cases {
            let once = merge(&target, &chunk);
            let twice = merge(&once, &chunk);
            assert_eq!(
                once, twice,
                "merge must be idempotent for {target} + {chunk}"
            );
            assert!(is_noop(&once, &chunk), "{chunk} still needed on {once}");
        }
    }

    #[test]
    fn equality_is_reflexive_and_symmetric_on_the_same_cases() {
        let values = [
            json!(null),
            json!(true),
            json!(1),
            json!(1.0),
            json!("a"),
            json!([1, 2]),
            json!({"a": 1, "b": [1, 2]}),
        ];
        for value in &values {
            assert!(json_eq(value, value));
            for other in &values {
                assert_eq!(json_eq(value, other), json_eq(other, value));
            }
        }
    }
}
