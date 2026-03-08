/// What Godot type a JSON value should map to.
///
/// Determined purely from JSON structure — no Godot dependency. The actual
/// `Variant` construction happens in the GDExtension adapter layer, which
/// calls `VariantTarget::from_json()` and then matches on the result.
///
/// Rules:
/// - 2-element all-numeric arrays → Vector2
/// - 3-element all-numeric arrays → Vector3
/// - Other arrays → generic Array
/// - Objects → Dictionary
/// - Primitives → direct mapping
#[derive(Debug, Clone, PartialEq)]
pub enum VariantTarget {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Vector2(f64, f64),
    Vector3(f64, f64, f64),
    Array(Vec<VariantTarget>),
    Dictionary(Vec<(String, VariantTarget)>),
}

impl VariantTarget {
    /// Determine the target Godot type from a JSON value.
    pub fn from_json(value: &serde_json::Value) -> Result<Self, String> {
        match value {
            serde_json::Value::Null => Ok(Self::Nil),
            serde_json::Value::Bool(b) => Ok(Self::Bool(*b)),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(Self::Int(i))
                } else if let Some(f) = n.as_f64() {
                    Ok(Self::Float(f))
                } else {
                    Err(format!("unsupported number: {n}"))
                }
            }
            serde_json::Value::String(s) => Ok(Self::String(s.clone())),
            serde_json::Value::Array(arr) => {
                // 2-element all-numeric → Vector2
                if arr.len() == 2 && arr.iter().all(|v| v.is_number()) {
                    let x = arr[0].as_f64().unwrap_or(0.0);
                    let y = arr[1].as_f64().unwrap_or(0.0);
                    return Ok(Self::Vector2(x, y));
                }
                // 3-element all-numeric → Vector3
                if arr.len() == 3 && arr.iter().all(|v| v.is_number()) {
                    let x = arr[0].as_f64().unwrap_or(0.0);
                    let y = arr[1].as_f64().unwrap_or(0.0);
                    let z = arr[2].as_f64().unwrap_or(0.0);
                    return Ok(Self::Vector3(x, y, z));
                }
                // Generic array
                let items = arr
                    .iter()
                    .map(Self::from_json)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Self::Array(items))
            }
            serde_json::Value::Object(map) => {
                let entries = map
                    .iter()
                    .map(|(k, v)| Self::from_json(v).map(|t| (k.clone(), t)))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Self::Dictionary(entries))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn null_maps_to_nil() {
        assert_eq!(VariantTarget::from_json(&json!(null)).unwrap(), VariantTarget::Nil);
    }

    #[test]
    fn bool_true_maps_to_bool() {
        assert_eq!(
            VariantTarget::from_json(&json!(true)).unwrap(),
            VariantTarget::Bool(true)
        );
    }

    #[test]
    fn bool_false_maps_to_bool() {
        assert_eq!(
            VariantTarget::from_json(&json!(false)).unwrap(),
            VariantTarget::Bool(false)
        );
    }

    #[test]
    fn integer_maps_to_int() {
        assert_eq!(
            VariantTarget::from_json(&json!(42)).unwrap(),
            VariantTarget::Int(42)
        );
    }

    #[test]
    fn negative_integer_maps_to_int() {
        assert_eq!(
            VariantTarget::from_json(&json!(-7)).unwrap(),
            VariantTarget::Int(-7)
        );
    }

    #[test]
    fn float_maps_to_float() {
        assert_eq!(
            VariantTarget::from_json(&json!(3.14)).unwrap(),
            VariantTarget::Float(3.14)
        );
    }

    #[test]
    fn string_maps_to_string() {
        assert_eq!(
            VariantTarget::from_json(&json!("hello")).unwrap(),
            VariantTarget::String("hello".into())
        );
    }

    #[test]
    fn two_element_numeric_array_maps_to_vector2() {
        assert_eq!(
            VariantTarget::from_json(&json!([1.0, 2.0])).unwrap(),
            VariantTarget::Vector2(1.0, 2.0)
        );
    }

    #[test]
    fn two_element_integer_array_maps_to_vector2() {
        assert_eq!(
            VariantTarget::from_json(&json!([3, 4])).unwrap(),
            VariantTarget::Vector2(3.0, 4.0)
        );
    }

    #[test]
    fn three_element_numeric_array_maps_to_vector3() {
        assert_eq!(
            VariantTarget::from_json(&json!([1.0, 2.0, 3.0])).unwrap(),
            VariantTarget::Vector3(1.0, 2.0, 3.0)
        );
    }

    #[test]
    fn three_element_negative_array_maps_to_vector3() {
        assert_eq!(
            VariantTarget::from_json(&json!([-1.0, 0.0, 5.0])).unwrap(),
            VariantTarget::Vector3(-1.0, 0.0, 5.0)
        );
    }

    #[test]
    fn four_element_array_maps_to_generic_array() {
        let result = VariantTarget::from_json(&json!([1, 2, 3, 4])).unwrap();
        assert!(matches!(result, VariantTarget::Array(_)));
    }

    #[test]
    fn mixed_array_maps_to_generic_array_not_vector() {
        // [1, "two"] — not all numeric, so generic array
        let result = VariantTarget::from_json(&json!([1, "two"])).unwrap();
        assert!(matches!(result, VariantTarget::Array(_)));
    }

    #[test]
    fn two_element_mixed_array_is_not_vector2() {
        let result = VariantTarget::from_json(&json!([1, null])).unwrap();
        assert!(matches!(result, VariantTarget::Array(_)));
    }

    #[test]
    fn empty_array_maps_to_generic_array() {
        let result = VariantTarget::from_json(&json!([])).unwrap();
        assert_eq!(result, VariantTarget::Array(vec![]));
    }

    #[test]
    fn one_element_numeric_array_is_generic() {
        // [5.0] — only 1 element, not Vector2/3
        let result = VariantTarget::from_json(&json!([5.0])).unwrap();
        assert!(matches!(result, VariantTarget::Array(_)));
    }

    #[test]
    fn object_maps_to_dictionary() {
        let result = VariantTarget::from_json(&json!({"hp": 100})).unwrap();
        match result {
            VariantTarget::Dictionary(entries) => {
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].0, "hp");
                assert_eq!(entries[0].1, VariantTarget::Int(100));
            }
            other => panic!("Expected Dictionary, got {:?}", other),
        }
    }

    #[test]
    fn empty_object_maps_to_empty_dictionary() {
        let result = VariantTarget::from_json(&json!({})).unwrap();
        assert_eq!(result, VariantTarget::Dictionary(vec![]));
    }

    #[test]
    fn nested_structure_with_vector3() {
        let result = VariantTarget::from_json(&json!({
            "pos": [1.0, 2.0, 3.0],
            "name": "Player",
            "alive": true
        }))
        .unwrap();

        match result {
            VariantTarget::Dictionary(entries) => {
                assert_eq!(entries.len(), 3);
                let pos = entries.iter().find(|(k, _)| k == "pos").unwrap();
                assert_eq!(pos.1, VariantTarget::Vector3(1.0, 2.0, 3.0));
                let name = entries.iter().find(|(k, _)| k == "name").unwrap();
                assert_eq!(name.1, VariantTarget::String("Player".into()));
                let alive = entries.iter().find(|(k, _)| k == "alive").unwrap();
                assert_eq!(alive.1, VariantTarget::Bool(true));
            }
            other => panic!("Expected Dictionary, got {:?}", other),
        }
    }

    #[test]
    fn nested_array_of_arrays() {
        let result = VariantTarget::from_json(&json!([[1.0, 2.0], [3.0, 4.0]])).unwrap();
        match result {
            VariantTarget::Array(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0], VariantTarget::Vector2(1.0, 2.0));
                assert_eq!(items[1], VariantTarget::Vector2(3.0, 4.0));
            }
            other => panic!("Expected Array, got {:?}", other),
        }
    }
}
