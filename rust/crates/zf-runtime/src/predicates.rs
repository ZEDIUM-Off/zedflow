//! Deterministic evaluation of the typed conditions declared by flow contracts.
use anyhow::{Result, bail, ensure};
use serde_json::Value;
use std::{cmp::Ordering, collections::HashMap};
#[cfg(test)]
use zf_flows::node_contracts::parse_predicate;
use zf_flows::node_contracts::{Predicate, PredicateOperator};

fn field<'a>(state: &'a HashMap<String, Value>, key: &str) -> Option<&'a Value> {
    let Some(pointer) = key.strip_prefix('/') else {
        return state.get(key);
    };
    let mut parts = pointer.split('/');
    let first = parts.next()?.replace("~1", "/").replace("~0", "~");
    let mut value = state.get(&first)?;
    for part in parts {
        let part = part.replace("~1", "/").replace("~0", "~");
        value = match value {
            Value::Object(object) => object.get(&part)?,
            Value::Array(array) => {
                if part.len() > 1 && part.starts_with('0') {
                    return None;
                }
                array.get(part.parse::<usize>().ok()?)?
            }
            _ => return None,
        };
    }
    Some(value)
}

fn number_order(left: &Value, right: &Value) -> Result<Ordering> {
    ensure!(
        left.is_number() && right.is_number(),
        "Types incompatibles : comparaison numérique attendue"
    );
    fn integer(value: &Value) -> Option<i128> {
        value
            .as_i64()
            .map(i128::from)
            .or_else(|| value.as_u64().map(i128::from))
    }
    if let (Some(left), Some(right)) = (integer(left), integer(right)) {
        return Ok(left.cmp(&right));
    }
    // Mixed integer/float comparison preserves the integer's precision rather
    // than silently rounding large JSON integers into a neighbouring f64.
    fn int_float(integer: i128, float: f64) -> Ordering {
        if float >= i128::MAX as f64 {
            return Ordering::Less;
        }
        if float <= i128::MIN as f64 {
            return Ordering::Greater;
        }
        let truncated = float as i128;
        integer.cmp(&truncated).then_with(|| {
            if float.fract() > 0.0 {
                Ordering::Less
            } else if float.fract() < 0.0 {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        })
    }
    if let (Some(left), Some(right)) = (integer(left), right.as_f64()) {
        return Ok(int_float(left, right));
    }
    if let (Some(left), Some(right)) = (left.as_f64(), integer(right)) {
        return Ok(int_float(right, left).reverse());
    }
    left.as_f64()
        .and_then(|left| right.as_f64().and_then(|right| left.partial_cmp(&right)))
        .ok_or_else(|| anyhow::anyhow!("Nombre non fini"))
}

fn same_type(left: &Value, right: &Value) -> bool {
    matches!(
        (left, right),
        (Value::Null, Value::Null)
            | (Value::Bool(_), Value::Bool(_))
            | (Value::Number(_), Value::Number(_))
            | (Value::String(_), Value::String(_))
            | (Value::Array(_), Value::Array(_))
            | (Value::Object(_), Value::Object(_))
    )
}

fn contains(container: &Value, needle: &Value) -> Result<bool> {
    match (container, needle) {
        (Value::String(text), Value::String(needle)) => Ok(text.contains(needle)),
        (Value::Array(values), needle) => Ok(values.iter().any(|value| value == needle)),
        _ => bail!("Types incompatibles : inclusion dans un tableau ou une chaîne attendue"),
    }
}

pub fn evaluate(predicate: &Predicate, state: &HashMap<String, Value>) -> Result<bool> {
    match predicate {
        Predicate::All { items } => {
            let values = items
                .iter()
                .map(|item| evaluate(item, state))
                .collect::<Result<Vec<_>>>()?;
            Ok(values.into_iter().all(|value| value))
        }
        Predicate::Any { items } => {
            let values = items
                .iter()
                .map(|item| evaluate(item, state))
                .collect::<Result<Vec<_>>>()?;
            Ok(values.into_iter().any(|value| value))
        }
        Predicate::Compare {
            field: key,
            operator,
            value,
        } => {
            let actual = field(state, key);
            if matches!(operator, PredicateOperator::Exists) {
                return Ok(actual.is_some());
            }
            let Some(actual) = actual else {
                return Ok(false);
            };
            let expected = value
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Valeur de comparaison requise"))?;
            Ok(match operator {
                PredicateOperator::Eq | PredicateOperator::Ne => {
                    ensure!(
                        same_type(actual, expected),
                        "Types incompatibles pour {key} : aucune conversion implicite"
                    );
                    if matches!(operator, PredicateOperator::Eq) {
                        actual == expected
                    } else {
                        actual != expected
                    }
                }
                PredicateOperator::Gt => number_order(actual, expected)? == Ordering::Greater,
                PredicateOperator::Gte => number_order(actual, expected)? != Ordering::Less,
                PredicateOperator::Lt => number_order(actual, expected)? == Ordering::Less,
                PredicateOperator::Lte => number_order(actual, expected)? != Ordering::Greater,
                PredicateOperator::Contains => contains(actual, expected)?,
                PredicateOperator::In => contains(expected, actual)?,
                PredicateOperator::Exists => true,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn typed_groups_nested_paths_and_missing_values() {
        let state = HashMap::from([("data".into(), json!({"count":3,"tags":["ok"],"nil":null}))]);
        let predicate = parse_predicate(&json!({"kind":"all","items":[{"kind":"compare","field":"/data/count","operator":"gte","value":3},{"kind":"any","items":[{"kind":"compare","field":"missing","operator":"exists"},{"kind":"compare","field":"/data/tags","operator":"contains","value":"ok"}]}]})).unwrap();
        assert!(evaluate(&predicate, &state).unwrap());
        assert!(
            evaluate(
                &parse_predicate(
                    &json!({"kind":"compare","field":"/data/nil","operator":"eq","value":null})
                )
                .unwrap(),
                &state
            )
            .unwrap()
        );
        assert!(
            !evaluate(
                &parse_predicate(
                    &json!({"kind":"compare","field":"missing","operator":"ne","value":null})
                )
                .unwrap(),
                &state
            )
            .unwrap()
        );
        assert!(
            evaluate(
                &parse_predicate(
                    &json!({"kind":"compare","field":"/data/count","operator":"eq","value":"3"})
                )
                .unwrap(),
                &state
            )
            .is_err()
        );
        assert!(parse_predicate(&json!({"kind":"all","items":[]})).is_err());
        assert!(
            parse_predicate(&json!({"kind":"compare","field":"data","operator":"eq"})).is_err()
        );
    }

    #[test]
    fn large_integer_comparisons_do_not_round() {
        assert_eq!(
            number_order(&json!(9007199254740993_u64), &json!(9007199254740992.0)).unwrap(),
            Ordering::Greater
        );
        assert_eq!(
            number_order(&json!(-3), &json!(-3.5)).unwrap(),
            Ordering::Greater
        );
    }
}
