use serde_json::json;
use zf_core::types::{DataType, TypeRegistry, compatible, validate_value};

#[test]
fn resource_contracts_preserve_nominal_identity_and_open_record_values() {
    let schema = json!({"kind":"record","fields":{"title":{"kind":"text"}}});
    let ty: DataType = serde_json::from_value(schema.clone()).unwrap();
    assert_eq!(serde_json::to_value(&ty).unwrap(), schema);
    let value = json!({"title":"Résumé", "extension":{"nested":[true,null,42]}});
    assert!(validate_value(&ty, &value, &TypeRegistry::new()).is_ok());
    assert!(validate_value(&ty, &json!({"title":42}), &TypeRegistry::new()).is_err());
    let types = TypeRegistry::from([
        ("RouteResult".into(), DataType::Text),
        ("OtherResult".into(), DataType::Text),
    ]);
    assert!(!compatible(
        &DataType::Named {
            name: "RouteResult".into()
        },
        &DataType::Named {
            name: "OtherResult".into()
        },
        &types,
    ));
}

#[test]
fn content_records_roundtrip_without_reinterpreting_open_json_or_references() {
    use zf_core::content::{ContentBlob, ContentRecord};
    let original = json!({"reference":"sha256:historical", "body":{
        "kind":"scalar", "value":{"large":18446744073709551615u64,"extra":[null,"🙂\n"],"reference":"user text"}
    }});
    let blob: ContentBlob = serde_json::from_value(original.clone()).unwrap();
    assert_eq!(serde_json::to_value(blob).unwrap(), original);
    let record =
        json!({"scope":"run-1","kind":"invocation","key":"call-2","valueRef":"sha256:historical"});
    let captured: ContentRecord = serde_json::from_value(record.clone()).unwrap();
    assert_eq!(serde_json::to_value(captured).unwrap(), record);
}

#[test]
fn historical_origins_and_tool_provenance_keep_exact_wire_identities() {
    use zf_core::events::{EventOrigin, ToolCallProvenance};
    use zf_core::identity::{RunId, SessionId};

    let original = json!({"nodePath":"root/docs/model","occurrenceId":"visit:4/model:2"});
    let origin: EventOrigin = serde_json::from_value(original.clone()).unwrap();
    assert_eq!(serde_json::to_value(origin).unwrap(), original);
    let provenance =
        json!({"invocationId":"invoke-old", "agentPath":"root/docs/model", "callIndex":2});
    let captured: ToolCallProvenance = serde_json::from_value(provenance.clone()).unwrap();
    assert_eq!(serde_json::to_value(captured).unwrap(), provenance);
    assert!(
        serde_json::from_value::<ToolCallProvenance>(
            json!({"invocationId":"i","agentPath":"a","callIndex":-1})
        )
        .is_err()
    );
    // Old identifiers are opaque, not restricted to UUIDs or normalized.
    let run: RunId = serde_json::from_value(json!("ancien/run é")).unwrap();
    assert_eq!(run.as_str(), "ancien/run é");
    assert_eq!(serde_json::to_value(run).unwrap(), json!("ancien/run é"));
    assert!(serde_json::from_value::<SessionId>(json!(123)).is_err());
}
