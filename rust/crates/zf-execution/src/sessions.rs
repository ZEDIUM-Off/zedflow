//! Durable session projection and journal persistence for execution observations.
use crate::service::{ExecutionState, now};
use serde_json::{Value, json};
use std::sync::atomic::Ordering;
use zf_storage::{content_store::ContentStore, session_store, timeline};

pub(crate) async fn persist_event(
    b: &ExecutionState,
    id: &str,
    run: &Value,
    event: &Value,
) -> anyhow::Result<()> {
    let _guard = b.writer.lock().await;
    let (latest, _) = b.sync.latest(id).await?;
    let mut merged = run.clone();
    // A parent wait is visible while launched children are still active. Its
    // final flush must preserve an answer already claiming that checkpoint.
    if matches!(merged["status"].as_str(), Some("waiting"))
        && latest["status"] == "running"
        && latest["resumeClaimId"] != merged["resumeClaimId"]
        && latest["wait"].is_null()
        && latest["resumeCheckpoint"].is_string()
        && latest["resumeCheckpoint"] == merged["checkpoint"]
    {
        merged["status"] = latest["status"].clone();
        merged["wait"] = Value::Null;
        merged["activeNode"] = Value::Null;
        merged["error"] = latest["error"].clone();
    }
    // These fields belong to public commands and may have changed while the
    // executor awaited a model/tool. Observations must not overwrite them.
    for field in [
        "name",
        "timeline",
        "timelineVersion",
        "timelineApproximate",
        "modelBindings",
        "modelRevision",
        "queue",
        "contextRef",
        "abortRequested",
        "resumeInput",
        "resumeCheckpoint",
        "resumeClaimId",
        "capabilityActivations",
        "import",
    ] {
        if let Some(value) = latest.get(field) {
            merged[field] = value.clone();
        }
    }
    // An abort may race the final empty-inbox check. Its durable command wins
    // over the resulting wait, while active effects still finish cancellation.
    if merged["abortRequested"] == true && merged["status"] == "waiting" {
        merged["status"] = json!("stopped");
        merged["activeNodes"] = json!([]);
        merged["activeNode"] = Value::Null;
    }
    // ADK applies resume input before creating the next checkpoint. Once that
    // checkpoint is persisted, the durable command payload is no longer needed.
    if merged["resumeCheckpoint"].is_string()
        && merged["checkpoint"].is_string()
        && merged["checkpoint"] != merged["resumeCheckpoint"]
    {
        merged["resumeInput"] = Value::Null;
        merged["resumeCheckpoint"] = Value::Null;
    }
    if run["context"]["loadedSkills"]
        .as_array()
        .is_some_and(|skills| !skills.is_empty())
    {
        session_store::restore_context(&b.content, &mut merged).await?;
    }
    for loaded in run["context"]["loadedSkills"]
        .as_array()
        .into_iter()
        .flatten()
    {
        record_loaded_skill(&mut merged, loaded.clone());
    }
    let consumed: Vec<String> = merged["consumedMessages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();
    let mut delivered = Vec::new();
    if let Some(queue) = merged["queue"].as_array_mut() {
        for message in queue.iter_mut() {
            if message["status"] == "pending"
                && message["id"]
                    .as_str()
                    .is_some_and(|id| consumed.iter().any(|seen| seen == id))
            {
                message["status"] = json!("consumed");
                delivered.push(message.clone());
            }
        }
    }
    // Start from the latest user-visible transcript, then append new execution
    // messages by identity/content. Commands only append user entries.
    let mut messages = latest["messages"].as_array().cloned().unwrap_or_default();
    for message in run["messages"].as_array().into_iter().flatten() {
        if !messages.contains(message) {
            messages.push(message.clone());
        }
    }
    for message in delivered {
        if !messages.iter().any(|m| m["id"] == message["id"]) {
            messages.push(json!({"id":message["id"],"role":"user","text":message.get("originalText").unwrap_or(&message["text"])}));
        }
    }
    merged["messages"] = json!(messages);
    let mut event = event.clone();
    if event["type"] == "run_status" {
        event["status"] = merged["status"].clone();
    }
    persist_command(b, id, &merged, &event).await?;
    if let Some(service) = b.services.lock().await.get(id) {
        service.replace_queue(merged["queue"].as_array().cloned().unwrap_or_default());
    }
    Ok(())
}
// Caller holds writer, or is creating a new isolated run.
pub(crate) async fn persist_command(
    b: &ExecutionState,
    id: &str,
    run: &Value,
    event: &Value,
) -> anyhow::Result<()> {
    let started = std::time::Instant::now();
    let (latest, revision) = b.sync.latest(id).await?;
    let mut document = run.clone();
    for field in ["timeline", "timelineVersion", "timelineApproximate"] {
        if let Some(value) = latest.get(field) {
            document[field] = value.clone();
        }
    }
    let seq = b.sequence.fetch_add(1, Ordering::SeqCst) + 1;
    timeline::retain_activity_order(&mut document, &latest);
    timeline::reconcile(&mut document, event, seq);
    document["updatedAt"] = json!(now());
    let document = session_store::compact_run(&b.content, &document).await?;
    let prepared = session_store::prepare_delta(&b.content, &document, &latest).await?;
    let event = session_store::compact_event(&b.content, event).await?;
    let changes = b.sync.changes(&latest, &document).await?;
    let changes_ref = b.content.intern(&changes).await?;
    let encoded_ms = started.elapsed().as_secs_f64() * 1000.;
    let writing = std::time::Instant::now();
    let mut tx = b.writer_db.begin().await?;
    sqlx::query("INSERT INTO events(seq,run,document) VALUES(?,?,?)")
        .bind(seq)
        .bind(id)
        .bind(event.to_string())
        .execute(&mut *tx)
        .await?;
    session_store::write(&mut tx, id, &prepared).await?;
    sqlx::query("INSERT INTO run_changes(seq,run,base_revision,document) VALUES(?,?,?,?)")
        .bind(seq)
        .bind(id)
        .bind(revision)
        .bind(json!({"valueRef":changes_ref}).to_string())
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let commit_ms = writing.elapsed().as_secs_f64() * 1000.;
    if let Some(service) = b.services.lock().await.get(id)
        && let Some(activations) = document["capabilityActivations"].as_object()
    {
        for (path, ids) in activations {
            service.set_active_capabilities(path.clone(), serde_json::from_value(ids.clone())?);
        }
    }
    let publishing = std::time::Instant::now();
    b.sync.committed(id, document, seq).await;
    b.sync.record_metrics(id,json!({"events":1,"seq":seq,"encodeMs":encoded_ms,"commitMs":commit_ms,"publishMs":publishing.elapsed().as_secs_f64()*1000.})).await;
    if encoded_ms + commit_ms > 100. {
        eprintln!(
            "session persistence run={id} seq={seq} encode_ms={encoded_ms:.2} commit_ms={commit_ms:.2}"
        );
    }
    Ok(())
}

/// Observe in bounded batches. Only the final projection is written; the journal
/// and replay operations retain the sequence of every individual occurrence.
pub(crate) async fn persist_batch(
    b: &ExecutionState,
    id: &str,
    events: &mut Vec<Value>,
) -> anyhow::Result<()> {
    if events.is_empty() {
        return Ok(());
    }
    let waiting = std::time::Instant::now();
    let _guard = b.writer.lock().await;
    let writer_wait_ms = waiting.elapsed().as_secs_f64() * 1000.;
    let (previous, mut revision) = b.sync.latest(id).await?;
    let mut document = previous.clone();
    let mut rows = Vec::new();
    let started = std::time::Instant::now();
    for event in events.iter() {
        let seq = b.sequence.fetch_add(1, Ordering::SeqCst) + 1;
        let before = document.clone();
        apply_observation(&b.content, &mut document, event).await?;
        timeline::reconcile(&mut document, event, seq);
        document["updatedAt"] = json!(now());
        document = session_store::compact_run(&b.content, &document).await?;
        let changes = b.sync.changes(&before, &document).await?;
        let reference = b.content.intern(&changes).await?;
        let compact = session_store::compact_event(&b.content, event).await?;
        rows.push((
            seq,
            revision,
            compact.to_string(),
            json!({"valueRef":reference}).to_string(),
        ));
        revision = seq;
    }
    let prepared = session_store::prepare_delta(&b.content, &document, &previous).await?;
    let encode_ms = started.elapsed().as_secs_f64() * 1000.;
    let transaction_start = std::time::Instant::now();
    let mut tx = b.writer_db.begin().await?;
    for (seq, base, event, changes) in &rows {
        sqlx::query("INSERT INTO events(seq,run,document) VALUES(?,?,?)")
            .bind(seq)
            .bind(id)
            .bind(event)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO run_changes(seq,run,base_revision,document) VALUES(?,?,?,?)")
            .bind(seq)
            .bind(id)
            .bind(base)
            .bind(changes)
            .execute(&mut *tx)
            .await?;
    }
    session_store::write(&mut tx, id, &prepared).await?;
    tx.commit().await?;
    let commit_ms = transaction_start.elapsed().as_secs_f64() * 1000.;
    let publishing = std::time::Instant::now();
    b.sync.committed(id, document, revision).await;
    b.sync.record_metrics(id,json!({"events":rows.len(),"seq":revision,"writerWaitMs":writer_wait_ms,"encodeMs":encode_ms,"commitMs":commit_ms,"publishMs":publishing.elapsed().as_secs_f64()*1000.})).await;
    if writer_wait_ms + encode_ms + commit_ms > 100. {
        eprintln!(
            "session batch run={id} events={} writer_wait_ms={writer_wait_ms:.2} encode_ms={encode_ms:.2} commit_ms={commit_ms:.2}",
            rows.len()
        );
    }
    events.clear();
    Ok(())
}

pub(crate) async fn apply_observation(
    store: &ContentStore,
    run: &mut Value,
    event: &Value,
) -> anyhow::Result<()> {
    session_store::restore_progress(store, run, event).await?;
    if event["type"] == "skill_loaded" {
        session_store::restore_context(store, run).await?;
    }
    apply_activity(run, event);
    Ok(())
}
pub(crate) fn apply_activity(run: &mut Value, event: &Value) {
    if event["type"] == "checkpoint_committed" {
        if event["threadId"] == run["id"] {
            run["checkpoint"] = event["checkpointId"].clone();
            run["stateRef"] = event["stateRef"].clone();
        }
        // Consumption is monotone across the parent and its child checkpoints.
        let mut consumed = run["consumedMessages"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for value in event["consumedMessages"].as_array().into_iter().flatten() {
            if !consumed.contains(value) {
                consumed.push(value.clone());
            }
        }
        run["consumedMessages"] = json!(consumed);
        return;
    }
    if event["type"] != "node_activity" {
        apply_tool_activity(run, event);
        return;
    }
    if let Some(message) = timeline::assistant_message(event) {
        if !run["messages"].is_array() {
            run["messages"] = json!([]);
        }
        if let Some(messages) = run["messages"].as_array_mut()
            && !messages.iter().any(|m| m["id"] == event["occurrenceId"])
        {
            messages.push(message);
        }
    }
    if !run["activities"].is_array() {
        run["activities"] = json!([]);
    }
    if let Some(activities) = run["activities"].as_array_mut() {
        if let Some(previous) = activities
            .iter_mut()
            .find(|a| a["occurrenceId"] == event["occurrenceId"])
        {
            let model_selection = previous.get("modelSelection").cloned();
            let original = previous.clone();
            *previous = event.clone();
            for field in [
                "inputRef",
                "stateRef",
                "startedSeq",
                "endedSeq",
                "input",
                "gapBeforeMs",
            ] {
                if previous.get(field).is_none()
                    && let Some(value) = original.get(field)
                {
                    previous[field] = value.clone();
                }
            }
            if let Some(selection) = model_selection {
                previous["modelSelection"] = selection;
            }
        } else {
            let mut activity = event.clone();
            let started = event["startedAt"].as_u64().unwrap_or_default();
            if let Some(ended) = activities
                .iter()
                .filter(|a| a["path"] == a["node"] && a["status"] != "waiting")
                .filter_map(|a| a["endedAt"].as_u64())
                .max()
                && started >= ended
            {
                activity["gapBeforeMs"] = json!(started - ended);
            }
            activities.push(activity);
        }
    }
    let active: Vec<_> = run["activities"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|a| a["status"] == "running" && a["path"] == a["node"])
        .map(|a| a["node"].clone())
        .collect();
    run["activeNodes"] = json!(active);
    run["activeNode"] = active.first().cloned().unwrap_or(Value::Null);
}
pub(crate) fn record_loaded_skill(run: &mut Value, loaded: Value) {
    if !run["context"].is_object() {
        run["context"] = json!({});
    }
    if !run["context"]["loadedSkills"].is_array() {
        run["context"]["loadedSkills"] = json!([]);
    }
    if let Some(items) = run["context"]["loadedSkills"].as_array_mut()
        && !items
            .iter()
            .any(|v| v["path"] == loaded["path"] && v["hash"] == loaded["hash"])
    {
        items.push(loaded);
    }
}
pub(crate) fn apply_tool_activity(run: &mut Value, event: &Value) {
    let kind = event["type"].as_str().unwrap_or_default();
    if matches!(
        kind,
        "model_request" | "inference_request_capture" | "inference_request_dispatched"
    ) {
        if let Some(snapshot) = run["contextSnapshots"].as_array_mut().and_then(|items| {
            items
                .iter_mut()
                .find(|snapshot| snapshot["invocationId"] == event["invocationId"])
        }) {
            if kind == "model_request" {
                snapshot["requestRef"] = event["requestRef"].clone();
            }
            if kind == "inference_request_capture" {
                snapshot["rawRef"] = event["rawRef"].clone();
                snapshot["requestBoundary"] = event["boundary"].clone();
                if snapshot["requestStatus"] != "sent" {
                    snapshot["requestStatus"] = json!("prepared");
                }
            }
            if kind == "inference_request_dispatched" {
                snapshot["requestStatus"] = json!("sent");
            }
        }
        return;
    }
    if kind == "context_snapshot" {
        if !run["contextSnapshots"].is_array() {
            run["contextSnapshots"] = json!([]);
        }
        if let Some(snapshots) = run["contextSnapshots"].as_array_mut()
            && event["snapshot"]["invocationId"].is_string()
            && !snapshots
                .iter()
                .any(|snapshot| snapshot["invocationId"] == event["snapshot"]["invocationId"])
        {
            snapshots.push(event["snapshot"].clone());
        }
        return;
    }
    if kind == "skill_loaded" {
        record_loaded_skill(run, event.clone());
        return;
    }
    if kind == "model_selection" {
        if let Some(activity) = run["activities"].as_array_mut().and_then(|items| {
            items
                .iter_mut()
                .rev()
                .find(|a| a["path"] == event["nodePath"] || a["node"] == event["nodePath"])
        }) {
            activity["modelSelection"] = event["selection"].clone();
        }
        return;
    }
    if !["tool_call", "tool_result", "tool_progress"].contains(&kind) {
        return;
    }
    let Some(id) = event["callId"].as_str().or_else(|| event["id"].as_str()) else {
        return;
    };
    if !run["toolActivities"].is_array() {
        run["toolActivities"] = json!([]);
    }
    let Some(items) = run["toolActivities"].as_array_mut() else {
        return;
    };
    let index = items.iter().position(|a|a["callId"]==id).unwrap_or_else(|| {
        items.push(json!({"callId":id,"nodePath":event["nodePath"],"name":event["name"],"status":"running","output":""})); items.len()-1
    });
    let item = &mut items[index];
    for field in [
        "origin",
        "startedAt",
        "endedAt",
        "durationMs",
        "receiptRef",
        "fullOutputRef",
    ] {
        if let Some(value) = event.get(field).filter(|v| !v.is_null()) {
            item[field] = value.clone();
        }
    }
    if kind == "tool_call" {
        item["arguments"] = event["arguments"].clone();
    }
    if kind == "tool_result" {
        item["status"] = event["status"].clone();
        item["result"] = event["result"].clone();
        item["error"] = event["error"].clone();
    }
    if kind == "tool_progress" {
        if let Some(content) = event["content"].as_str() {
            item["output"] = json!(content);
            item["truncated"] = event["truncated"].clone();
            return;
        }
        let mut output = item["output"].as_str().unwrap_or_default().to_owned();
        output.push_str(
            event["text"]
                .as_str()
                .or_else(|| event["chunk"].as_str())
                .unwrap_or_default(),
        );
        if output.len() > 51200 {
            let mut keep = output.len() - 51200;
            while !output.is_char_boundary(keep) {
                keep += 1;
            }
            output.drain(..keep);
        }
        item["output"] = json!(output);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_checkpoints_extend_consumption_without_replacing_parent_checkpoint() {
        let mut run = json!({
            "id": "parent", "checkpoint": "parent-old", "stateRef": "state-old",
            "consumedMessages": ["first"]
        });
        apply_activity(
            &mut run,
            &json!({
                "type": "checkpoint_committed", "threadId": "child",
                "checkpointId": "child-new", "stateRef": "child-state",
                "consumedMessages": ["first", "second"]
            }),
        );
        assert_eq!(run["checkpoint"], "parent-old");
        assert_eq!(run["stateRef"], "state-old");
        assert_eq!(run["consumedMessages"], json!(["first", "second"]));

        apply_activity(
            &mut run,
            &json!({
                "type": "checkpoint_committed", "threadId": "parent",
                "checkpointId": "parent-new", "stateRef": "state-new",
                "consumedMessages": ["second", "third"]
            }),
        );
        assert_eq!(run["checkpoint"], "parent-new");
        assert_eq!(run["stateRef"], "state-new");
        assert_eq!(run["consumedMessages"], json!(["first", "second", "third"]));
    }

    #[test]
    fn completed_activity_preserves_captured_fields_and_clears_active_node() {
        let mut run = json!({"activities": [{
            "type": "node_activity", "occurrenceId": "occurrence", "path": "agent",
            "node": "agent", "status": "running", "inputRef": "input-ref",
            "stateRef": "state-ref", "startedSeq": 3, "endedSeq": 4,
            "input": {"question": "fixture"}, "gapBeforeMs": 20,
            "modelSelection": {"model": "fixture-model"}
        }]});
        apply_activity(
            &mut run,
            &json!({
                "type": "node_activity", "occurrenceId": "occurrence", "path": "agent",
                "node": "agent", "status": "completed", "endedSeq": 7
            }),
        );
        let activity = &run["activities"][0];
        assert_eq!(run["activities"].as_array().unwrap().len(), 1);
        assert_eq!(activity["inputRef"], "input-ref");
        assert_eq!(activity["stateRef"], "state-ref");
        assert_eq!(activity["startedSeq"], 3);
        assert_eq!(activity["endedSeq"], 7);
        assert_eq!(activity["input"], json!({"question": "fixture"}));
        assert_eq!(activity["gapBeforeMs"], 20);
        assert_eq!(
            activity["modelSelection"],
            json!({"model": "fixture-model"})
        );
        assert_eq!(run["activeNodes"], json!([]));
        assert!(run["activeNode"].is_null());
    }

    #[test]
    fn loaded_skills_deduplicate_by_path_and_hash_together() {
        let mut run = json!({});
        let first = json!({"path": "skills/review", "hash": "one"});
        record_loaded_skill(&mut run, first.clone());
        record_loaded_skill(&mut run, first.clone());
        let changed = json!({"path": "skills/review", "hash": "two"});
        record_loaded_skill(&mut run, changed.clone());
        let relocated = json!({"path": "skills/other", "hash": "one"});
        record_loaded_skill(&mut run, relocated.clone());
        assert_eq!(
            run["context"]["loadedSkills"],
            json!([first, changed, relocated])
        );
    }

    #[test]
    fn late_request_capture_does_not_regress_dispatched_snapshot() {
        let mut run = json!({"contextSnapshots": [{"invocationId": "invocation"}]});
        apply_tool_activity(
            &mut run,
            &json!({
                "type": "inference_request_dispatched", "invocationId": "invocation"
            }),
        );
        apply_tool_activity(
            &mut run,
            &json!({
                "type": "inference_request_capture", "invocationId": "invocation",
                "rawRef": "raw-ref", "boundary": "request-body"
            }),
        );
        assert_eq!(run["contextSnapshots"][0]["requestStatus"], "sent");
        assert_eq!(run["contextSnapshots"][0]["rawRef"], "raw-ref");
        assert_eq!(
            run["contextSnapshots"][0]["requestBoundary"],
            "request-body"
        );
    }

    #[test]
    fn streamed_tool_output_keeps_utf8_tail_and_content_replaces_it() {
        let mut run = json!({});
        apply_tool_activity(
            &mut run,
            &json!({
                "type": "tool_progress", "callId": "call", "text": "é".repeat(25601)
            }),
        );
        assert_eq!(run["toolActivities"][0]["output"], "é".repeat(25600));
        apply_tool_activity(
            &mut run,
            &json!({
                "type": "tool_progress", "callId": "call", "chunk": "x"
            }),
        );
        assert_eq!(
            run["toolActivities"][0]["output"],
            format!("{}x", "é".repeat(25599))
        );
        apply_tool_activity(
            &mut run,
            &json!({
                "type": "tool_progress", "callId": "call", "content": "replacement",
                "truncated": true
            }),
        );
        assert_eq!(run["toolActivities"][0]["output"], "replacement");
        assert_eq!(run["toolActivities"][0]["truncated"], true);
    }
}
