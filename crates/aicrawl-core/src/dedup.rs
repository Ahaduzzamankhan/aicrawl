//! De-duplication of extracted collections.
//!
//! Items are compared by the configured identifier fields. Non-object items fall
//! back to their scalar value, so `["a", "a", "b"]` also de-duplicates.

use std::collections::HashSet;

use serde_json::Value;

/// Removes duplicate items in place, keeping the first occurrence.
/// Returns the number of removed items.
pub fn dedupe(items: &mut Vec<Value>, keys: &[String]) -> usize {
    let before = items.len();
    let mut seen: HashSet<String> = HashSet::new();
    items.retain(|item| {
        let key = item_key(item, keys);
        seen.insert(key)
    });
    before - items.len()
}

fn item_key(item: &Value, keys: &[String]) -> String {
    match item {
        Value::Object(map) => {
            let mut parts: Vec<String> = Vec::new();
            for key in keys {
                let value = map.get(key).cloned().unwrap_or(Value::Null);
                parts.push(format!(
                    "{}={}",
                    key,
                    match &value {
                        Value::String(text) => text.clone(),
                        other => other.to_string(),
                    }
                ));
            }
            parts.join("\u{1f}")
        }
        other => match other {
            Value::String(text) => text.clone(),
            value => value.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn removes_duplicate_objects_by_key() {
        let mut items = vec![
            json!({"name": "Widget", "price": "1"}),
            json!({"name": "Gadget", "price": "2"}),
            json!({"name": "Widget", "price": "3"}),
        ];
        let removed = dedupe(&mut items, &["name".into()]);
        assert_eq!(removed, 1);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["price"], "1");
    }

    #[test]
    fn uses_the_full_key_tuple() {
        let mut items = vec![
            json!({"name": "Widget", "size": "S"}),
            json!({"name": "Widget", "size": "M"}),
        ];
        assert_eq!(dedupe(&mut items, &["name".into(), "size".into()]), 0);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn deduplicates_scalars() {
        let mut items = vec![json!("a"), json!("a"), json!("b")];
        assert_eq!(dedupe(&mut items, &["name".into()]), 1);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn missing_keys_still_deduplicate() {
        let mut items = vec![json!({"x": 1}), json!({"x": 2})];
        assert_eq!(dedupe(&mut items, &["name".into()]), 1);
        assert_eq!(items.len(), 1);
    }
}
