//! Exact serialized request segments. Captures are invocation-owned runtime
//! records; content addressing shares unchanged messages across invocations.
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;

tokio::task_local! {
    pub static CAPTURE: (Arc<crate::runtime::RunServices>, String);
}

/// Serialize each envelope field once, splitting message and tool arrays at
/// their item boundaries. Concatenation is the actual request body, not a later
/// reconstruction from a provider-neutral manifest.
pub fn segments(body: &Value) -> Result<Vec<String>> {
    let object = body.as_object().context("Request must be an object")?;
    let mut parts = vec!["{".into()];
    for (index, (name, value)) in object.iter().enumerate() {
        if index != 0 {
            parts.push(",".into());
        }
        parts.push(format!("{}:", serde_json::to_string(name)?));
        if matches!(name.as_str(), "input" | "tools" | "contents") && value.is_array() {
            parts.push("[".into());
            for (index, item) in value
                .as_array()
                .context("Expected array")?
                .iter()
                .enumerate()
            {
                if index != 0 {
                    parts.push(",".into());
                }
                parts.push(serde_json::to_string(item)?);
            }
            parts.push("]".into());
        } else if name == "tools" && value.is_object() {
            parts.push("{".into());
            for (index, (name, tool)) in value
                .as_object()
                .context("Expected tools object")?
                .iter()
                .enumerate()
            {
                if index != 0 {
                    parts.push(",".into());
                }
                parts.push(format!("{}:", serde_json::to_string(name)?));
                parts.push(serde_json::to_string(tool)?);
            }
            parts.push("}".into());
        } else {
            parts.push(serde_json::to_string(value)?);
        }
    }
    parts.push("}".into());
    Ok(parts)
}

pub fn document(boundary: &str, parts: &[String]) -> Value {
    let bytes = parts.concat();
    json!({"version":1,"boundary":boundary,"encoding":"utf8-segments-v1",
        "byteLength":bytes.len(),"sha256":format!("{:x}",Sha256::digest(bytes.as_bytes())),"segments":parts})
}

pub fn bytes(document: &Value) -> Result<Vec<u8>> {
    ensure!(
        document["version"] == 1 && document["encoding"] == "utf8-segments-v1",
        "Unsupported request capture encoding"
    );
    let parts: Vec<String> = serde_json::from_value(document["segments"].clone())?;
    let bytes = parts.concat().into_bytes();
    ensure!(
        document["byteLength"].as_u64() == Some(bytes.len() as u64),
        "Incomplete request capture"
    );
    ensure!(
        document["sha256"] == format!("{:x}", Sha256::digest(&bytes)),
        "Request capture hash mismatch"
    );
    Ok(bytes)
}

pub async fn capture(boundary: &str, parts: &[String]) -> Result<()> {
    if let Ok((services, invocation)) = CAPTURE.try_with(Clone::clone) {
        record_capture(&services, &invocation, boundary, parts).await?;
    }
    Ok(())
}

pub async fn record_capture(
    services: &crate::runtime::RunServices,
    invocation: &str,
    boundary: &str,
    parts: &[String],
) -> Result<()> {
    let reference = services
        .persist_record("inference-raw", invocation, &document(boundary, parts))
        .await?;
    services
        .emit(
            json!({"type":"inference_request_capture","invocationId":invocation,
        "rawRef":reference,"boundary":boundary,"status":"prepared"}),
        )
        .await;
    Ok(())
}

/// HTTP response headers establish that the request reached the adapter's
/// transport. A network failure leaves the capture prepared, not falsely sent.
pub async fn dispatched() -> Result<()> {
    if let Ok((services, invocation)) = CAPTURE.try_with(Clone::clone) {
        record_dispatch(&services, &invocation).await?;
    }
    Ok(())
}

pub async fn record_dispatch(
    services: &crate::runtime::RunServices,
    invocation: &str,
) -> Result<()> {
    if services
        .read_record("inference-dispatch", invocation)
        .await?
        .is_none()
    {
        services
            .persist_record("inference-dispatch", invocation, &json!({"status":"sent"}))
            .await?;
    }
    services.emit(json!({"type":"inference_request_dispatched","invocationId":invocation,"status":"sent"})).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_unicode_tools_and_stable_message_segments() {
        let message = json!({"role":"user","content":"Été 🦀\n\"citation\""});
        let a =
            json!({"instructions":"a","input":[message],"tools":[{"name":"read"}],"stream":true});
        let mut b = a.clone();
        b["instructions"] = json!("different envelope length");
        b["input"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"assistant","content":"oui"}));
        let parts = segments(&a).unwrap();
        let encoded = document("codexHttpBody", &parts);
        assert_eq!(
            serde_json::from_slice::<Value>(&bytes(&encoded).unwrap()).unwrap(),
            a
        );
        let shared = serde_json::to_string(&message).unwrap();
        assert!(parts.contains(&shared));
        assert!(segments(&b).unwrap().contains(&shared));
        let declaration = json!({"description":"Lire","parameters":{"type":"object"}});
        let adk = json!({"tools":{"read":declaration,"write":{"description":"Écrire"}},"contents":[message]});
        let adk_parts = segments(&adk).unwrap();
        assert!(adk_parts.contains(&serde_json::to_string(&declaration).unwrap()));
        assert_eq!(
            serde_json::from_str::<Value>(&adk_parts.concat()).unwrap(),
            adk
        );
        let mut broken = encoded;
        broken["segments"][0] = json!(" ");
        assert!(bytes(&broken).is_err());
    }
    #[tokio::test]
    async fn captures_share_history_and_export_exact_bytes() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let store = zf_storage::content_store::ContentStore::new(pool.clone())
            .await
            .unwrap();
        let mut messages = vec![];
        let mut roots = vec![];
        let mut last = vec![];
        for index in 0..100 {
            messages.push(json!({"role":"user","content":format!("{index} {}","é🦀".repeat(100))}));
            let body = json!({"instructions":format!("Revision {index}"),"input":messages,"tools":[{"name":"read"}]});
            let parts = segments(&body).unwrap();
            last = parts.concat().into_bytes();
            roots.push(
                store
                    .intern(&document("codexHttpBody", &parts))
                    .await
                    .unwrap(),
            );
        }
        let blobs = store.export_blobs(&roots).await.unwrap();
        let stored = serde_json::to_vec(&blobs).unwrap().len();
        assert!(
            stored < 1_000_000,
            "Stored {stored} bytes for a 100-message growing history"
        );
        let target = zf_storage::content_store::ContentStore::new(
            sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect("sqlite::memory:")
                .await
                .unwrap(),
        )
        .await
        .unwrap();
        target.import_blobs(&blobs).await.unwrap();
        assert_eq!(
            bytes(&target.resolve(roots.last().unwrap()).await.unwrap()).unwrap(),
            last
        );
    }
}
