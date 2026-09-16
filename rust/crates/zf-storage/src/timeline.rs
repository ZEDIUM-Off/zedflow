//! Durable conversational order, projected in the event/document transaction.
//!
//! The array is authoritative: several messages can share one event sequence.
//! Tool progress changes a block in place instead of changing its position.
use serde_json::{Value, json};

const VERSION: u64 = 1;
const OUTPUT_LIMIT: usize = 51_200;

/// Project one event after merging the latest persisted timeline into `run`.
/// `seq` is the sequence returned by the event insert in the same transaction.
pub fn reconcile(run: &mut Value, event: &Value, seq: i64) {
    if !run.is_object() {
        return;
    }
    consume_visible_messages(run, event);
    anchor_activity(run, event, seq);
    let mut entries = match run["timeline"].take() {
        Value::Array(entries) => entries,
        _ => Vec::new(),
    };
    for (index, message) in run["messages"].as_array().into_iter().flatten().enumerate() {
        insert_message(&mut entries, message, index, seq);
    }
    project_tool(&mut entries, event, seq);
    project_output(&mut entries, event, seq);
    anchor_messages(&mut entries, event);
    run["timeline"] = json!(entries);
    run["timelineVersion"] = json!(VERSION);
}

/// Executor snapshots do not contain SQL sequences. Preserve the anchors owned
/// by persistence when their newer execution data replaces an activity.
pub fn retain_activity_order(run: &mut Value, persisted: &Value) {
    let previous: std::collections::HashMap<_, _> = persisted["activities"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| a["occurrenceId"].as_str().map(|id| (id, a)))
        .collect();
    for activity in run["activities"].as_array_mut().into_iter().flatten() {
        if let Some(previous) = activity["occurrenceId"]
            .as_str()
            .and_then(|id| previous.get(id))
        {
            for field in ["startedSeq", "endedSeq"] {
                if let Some(value) = previous.get(field) {
                    activity[field] = value.clone();
                }
            }
        }
    }
}

fn anchor_activity(run: &mut Value, event: &Value, seq: i64) {
    if event["type"] != "node_activity" || !event["occurrenceId"].is_string() {
        return;
    }
    if let Some(activity) = run["activities"].as_array_mut().and_then(|activities| {
        activities
            .iter_mut()
            .find(|activity| activity["occurrenceId"] == event["occurrenceId"])
    }) {
        if event["status"] == "running" && activity["startedSeq"].is_null() {
            activity["startedSeq"] = json!(seq);
        } else if event["status"] != "running" && activity["endedSeq"].is_null() {
            activity["endedSeq"] = json!(seq);
        }
    }
}

fn event_origin(event: &Value) -> Option<Value> {
    if event["origin"]["nodePath"].is_string() && event["origin"]["occurrenceId"].is_string() {
        return Some(event["origin"].clone());
    }
    if event["type"] == "node_activity" && event["occurrenceId"].is_string() {
        let path = event["path"]
            .as_str()
            .or_else(|| event["nodePath"].as_str())?;
        return Some(json!({"nodePath":path,"occurrenceId":event["occurrenceId"]}));
    }
    None
}

fn anchor_messages(entries: &mut [Value], event: &Value) {
    let Some(origin) = event_origin(event) else {
        return;
    };
    if event["type"] != "node_activity" || event["status"] != "completed" {
        return;
    }
    let mut ids = Vec::new();
    if assistant_message(event).is_some() {
        ids.push(format!(
            "message:{}",
            event["occurrenceId"].as_str().unwrap_or_default()
        ));
    }
    if matches!(event["kind"].as_str(), Some("steering" | "inbox")) {
        ids.extend(
            event["output"]["__zedflow:consumedMessages"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(|id| format!("message:{id}")),
        );
    }
    for entry in entries.iter_mut().filter(|entry| {
        ids.iter()
            .any(|id| entry["id"] == *id || entry["sourceId"] == *id)
    }) {
        entry["origin"] = origin.clone();
    }
}

fn message_id(message: &Value, index: usize) -> String {
    message["id"].as_str().map_or_else(
        || format!("message:legacy:{index}"),
        |id| format!("message:{id}"),
    )
}

fn insert_message(entries: &mut Vec<Value>, message: &Value, index: usize, seq: i64) {
    if !matches!(message["role"].as_str(), Some("user" | "assistant")) {
        return;
    }
    let id = message_id(message, index);
    if entries
        .iter()
        .any(|entry| entry["id"] == id || entry["sourceId"] == id)
    {
        return;
    }
    entries.push(json!({
        "id":id, "seq":seq, "kind":"message", "role":message["role"],
        "text":message["text"],
    }));
}

fn consume_visible_messages(run: &mut Value, event: &Value) {
    if event["type"] != "node_activity"
        || event["status"] != "completed"
        || !matches!(event["kind"].as_str(), Some("steering" | "inbox"))
    {
        return;
    }
    let delivered: Vec<Value> = event["output"]["__zedflow:consumedMessages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|id| {
            run["queue"]
                .as_array()?
                .iter()
                .find(|message| message["id"] == *id && message["status"] != "cancelled")
        })
        .map(|message| {
            json!({"id":message["id"],"role":"user","text":message.get("originalText").unwrap_or(&message["text"])})
        })
        .collect();
    if delivered.is_empty() {
        return;
    }
    if !run["messages"].is_array() {
        run["messages"] = json!([]);
    }
    if let Some(messages) = run["messages"].as_array_mut() {
        for message in delivered {
            if !messages
                .iter()
                .any(|existing| existing["id"] == message["id"])
            {
                messages.push(message);
            }
        }
    }
    // The checkpoint, not an observation preceding it, owns queue consumption.
    // Changing its status here could lose the input after an uncheckpointed crash.
}

/// The explicit output node owns final replies. A split model node can also
/// publish the text that accompanies tool calls, identified by its occurrence.
/// The observer supplies only a field reference; the text stays in its output.
pub fn assistant_message(event: &Value) -> Option<Value> {
    if event["type"] != "node_activity" || event["status"] != "completed" {
        return None;
    }
    let field = match event["kind"].as_str() {
        Some("output") => "response",
        Some("model") => event["messageField"].as_str()?,
        _ => return None,
    };
    let id = event["occurrenceId"].as_str()?;
    let text = event["output"][field]
        .as_str()
        .filter(|text| !text.is_empty())?;
    Some(json!({"id":id,"role":"assistant","text":text}))
}

fn project_output(entries: &mut Vec<Value>, event: &Value, seq: i64) {
    let Some(message) = assistant_message(event) else {
        return;
    };
    insert_message(entries, &message, 0, seq);
}

fn project_tool(entries: &mut Vec<Value>, event: &Value, seq: i64) {
    let kind = event["type"].as_str().unwrap_or_default();
    if !matches!(kind, "tool_call" | "tool_progress" | "tool_result") {
        return;
    }
    let Some(call_id) = event["callId"].as_str().or_else(|| event["id"].as_str()) else {
        return;
    };
    let id = format!("tool:{call_id}");
    let index = match entries.iter().position(|entry| entry["id"] == id) {
        Some(index) => index,
        None => {
            entries.push(json!({"id":id,"seq":seq,"kind":"tool","activity":{
                "callId":call_id,"nodePath":event["nodePath"],"name":event["name"],
                "status":"running","output":"",
            }}));
            entries.len() - 1
        }
    };
    if entries[index]["updatedSeq"]
        .as_i64()
        .is_some_and(|updated| updated >= seq)
    {
        return;
    }
    entries[index]["updatedSeq"] = json!(seq);
    if let Some(origin) = event_origin(event) {
        entries[index]["origin"] = origin.clone();
        entries[index]["activity"]["origin"] = origin;
    }
    let activity = &mut entries[index]["activity"];
    for field in [
        "nodePath",
        "name",
        "startedAt",
        "endedAt",
        "durationMs",
        "receiptRef",
        "fullOutputRef",
    ] {
        if let Some(value) = event.get(field).filter(|value| !value.is_null()) {
            activity[field] = value.clone();
        }
    }
    match kind {
        "tool_call" => activity["arguments"] = event["arguments"].clone(),
        "tool_result" => {
            activity["status"] = event.get("status").cloned().unwrap_or_else(|| {
                json!(if event["error"].is_null() {
                    "completed"
                } else {
                    "failed"
                })
            });
            activity["result"] = event["result"].clone();
            activity["error"] = event["error"].clone();
        }
        "tool_progress" => {
            if let Some(content) = event["content"].as_str() {
                activity["output"] = json!(content);
                activity["truncated"] = event["truncated"].clone();
            } else {
                let mut content = activity["output"].as_str().unwrap_or_default().to_owned();
                content.push_str(
                    event["text"]
                        .as_str()
                        .or_else(|| event["chunk"].as_str())
                        .unwrap_or_default(),
                );
                if content.len() > OUTPUT_LIMIT {
                    let mut offset = content.len() - OUTPUT_LIMIT;
                    while !content.is_char_boundary(offset) {
                        offset += 1;
                    }
                    content.drain(..offset);
                    activity["truncated"] = json!(true);
                }
                activity["output"] = json!(content);
            }
        }
        _ => {}
    }
}

/// Add a timeline to an old run without changing its messages or tool records.
/// Exact event anchors are used where available; inferred positions are marked.
/// Calling this again is a no-op, including after a run has subsequently resumed.
pub fn migrate(run: &mut Value, events: &[(i64, Value)]) {
    if !run.is_object() || run["timelineVersion"] == VERSION {
        return;
    }
    let mut ordered: Vec<_> = events.iter().collect();
    ordered.sort_by_key(|(seq, _)| *seq);
    let mut replay = json!({"messages":[],"queue":run["queue"],"timeline":[]});
    for (seq, event) in &ordered {
        consume_visible_messages(&mut replay, event);
        reconcile(&mut replay, event, *seq);
    }
    let mut entries = match replay["timeline"].take() {
        Value::Array(entries) => entries,
        _ => Vec::new(),
    };
    let mut approximate = false;
    let messages = run["messages"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    // Early documents omitted message IDs. Match their output observations once,
    // in transcript order, without collapsing repeated identical responses.
    let known: std::collections::HashSet<_> = messages
        .iter()
        .filter_map(|message| message["id"].as_str())
        .map(|id| format!("message:{id}"))
        .collect();
    for (index, message) in messages
        .iter()
        .enumerate()
        .filter(|(_, message)| message["id"].is_null() && message["role"] == "assistant")
    {
        if let Some(entry) = entries.iter_mut().find(|entry| {
            entry["kind"] == "message"
                && entry["role"] == "assistant"
                && entry["text"] == message["text"]
                && entry.get("sourceId").is_none()
                && !entry["id"].as_str().is_some_and(|id| known.contains(id))
        }) {
            entry["sourceId"] = entry["id"].clone();
            entry["id"] = json!(message_id(message, index));
            entry["approximate"] = json!(true);
            approximate = true;
        }
    }
    let end_seq = ordered.last().map_or(0, |(seq, _)| *seq);
    let mut previous_message: Option<String> = None;
    for (index, message) in messages.iter().enumerate() {
        let id = message_id(message, index);
        if !entries.iter().any(|entry| entry["id"] == id) {
            // Unanchored user input belongs after the preceding transcript entry;
            // an unanchored response belongs before the next known transcript entry.
            // The original message order is preserved even when event data is absent.
            let lower = previous_message
                .as_ref()
                .and_then(|previous| entries.iter().position(|entry| entry["id"] == *previous))
                .map_or(0, |position| position + 1);
            let upper = messages
                .iter()
                .enumerate()
                .skip(index + 1)
                .find_map(|(next_index, next)| {
                    let next_id = message_id(next, next_index);
                    entries.iter().position(|entry| entry["id"] == next_id)
                })
                .unwrap_or(entries.len())
                .max(lower);
            let position = if message["role"] == "user" {
                lower
            } else {
                upper
            };
            let seq = if position == 0 {
                0
            } else {
                entries
                    .get(position - 1)
                    .and_then(|entry| entry["seq"].as_i64())
                    .unwrap_or(end_seq)
            };
            let mut entry = Vec::new();
            insert_message(&mut entry, message, index, seq);
            if let Some(mut entry) = entry.pop() {
                entry["approximate"] = json!(true);
                entries.insert(position.min(entries.len()), entry);
                approximate = true;
            }
        }
        previous_message = Some(id);
    }
    for (index, activity) in run["toolActivities"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let id = activity["callId"]
            .as_str()
            .map_or_else(|| format!("tool:legacy:{index}"), |id| format!("tool:{id}"));
        if let Some(entry) = entries.iter_mut().find(|entry| entry["id"] == id) {
            // The final snapshot can contain results drained after the last event.
            entry["activity"] = activity.clone();
        } else {
            entries.push(
                json!({"id":id,"seq":end_seq,"kind":"tool","activity":activity,"approximate":true}),
            );
            approximate = true;
        }
    }
    run["timeline"] = json!(entries);
    run["timelineVersion"] = json!(VERSION);
    run["timelineApproximate"] = json!(approximate);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(id: &str, text: &str) -> Value {
        json!({"type":"node_activity","kind":"output","status":"completed","occurrenceId":id,"output":{"response":text}})
    }
    fn call(id: &str) -> Value {
        json!({"type":"tool_call","callId":id,"nodePath":"tools","name":"exec","arguments":{"command":"echo bonjour"}})
    }
    fn ids(run: &Value) -> Vec<&str> {
        run["timeline"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["id"].as_str().unwrap())
            .collect()
    }

    #[test]
    fn intermediate_model_messages_are_occurrence_identified_without_promoting_other_outputs() {
        let mut event = json!({"type":"node_activity","kind":"model","status":"completed","path":"child/model","occurrenceId":"first","messageField":"prediction","output":{"prediction":"Je vérifie.","hasToolCalls":true}});
        let mut run = json!({"messages":[]});
        reconcile(&mut run, &event, 1);
        reconcile(&mut run, &event, 1);
        reconcile(&mut run, &call("tool"), 2);
        event["occurrenceId"] = json!("second");
        reconcile(&mut run, &event, 3);
        assert_eq!(ids(&run), ["message:first", "tool:tool", "message:second"]);
        assert_eq!(
            run["timeline"][0]["origin"],
            json!({"nodePath":"child/model","occurrenceId":"first"})
        );
        assert_eq!(run["timeline"][0]["text"], run["timeline"][2]["text"]);

        // The marker is emitted only for successful model text plus calls.
        // Historical agents and final no-call model outputs stay silent here.
        event["kind"] = json!("agent");
        assert!(assistant_message(&event).is_none());
        event["kind"] = json!("model");
        event["status"] = json!("interrupted");
        assert!(assistant_message(&event).is_none());
        event["status"] = json!("completed");
        event.as_object_mut().unwrap().remove("messageField");
        assert!(assistant_message(&event).is_none());
    }

    #[test]
    fn tool_lifecycle_keeps_its_position_before_the_final_answer_across_turns() {
        let mut run = json!({"messages":[{"id":"u1","role":"user","text":"Commence"}]});
        reconcile(&mut run, &json!({"type":"run_status"}), 1);
        reconcile(&mut run, &call("c1"), 2);
        reconcile(
            &mut run,
            &json!({"type":"tool_progress","callId":"c1","content":"bon"}),
            3,
        );
        reconcile(
            &mut run,
            &json!({"type":"tool_progress","callId":"c1","content":"bonjour"}),
            4,
        );
        reconcile(
            &mut run,
            &json!({"type":"tool_result","callId":"c1","status":"completed","result":{"content":"bonjour"}}),
            5,
        );
        run["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":"a1","role":"assistant","text":"Fini"}));
        reconcile(&mut run, &output("a1", "Fini"), 6);
        run["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":"u2","role":"user","text":"Encore"}));
        reconcile(&mut run, &json!({"type":"run_status"}), 7);
        reconcile(&mut run, &call("c2"), 8);
        reconcile(&mut run, &output("a2", "Fini"), 9);
        assert_eq!(
            ids(&run),
            [
                "message:u1",
                "tool:c1",
                "message:a1",
                "message:u2",
                "tool:c2",
                "message:a2"
            ]
        );
        assert_eq!(run["timeline"][1]["seq"], 2);
        assert_eq!(run["timeline"][1]["activity"]["output"], "bonjour");
        assert_eq!(run["timeline"][1]["activity"]["status"], "completed");
        let restored = serde_json::from_slice::<Value>(&serde_json::to_vec(&run).unwrap()).unwrap();
        assert_eq!(restored["timeline"], run["timeline"]);
    }

    #[test]
    fn steering_is_visible_before_the_next_response_without_advancing_checkpoint_consumption() {
        let mut run = json!({"messages":[],"queue":[{"id":"q","kind":"steering","status":"pending","text":"expanded skill","originalText":"/skill:review"}]});
        reconcile(&mut run, &call("c"), 10);
        reconcile(
            &mut run,
            &json!({"type":"node_activity","kind":"steering","status":"completed","path":"child/steer","output":{"__zedflow:consumedMessages":["q"]}}),
            11,
        );
        reconcile(&mut run, &output("a", "Reviewed"), 12);
        assert_eq!(ids(&run), ["tool:c", "message:q", "message:a"]);
        assert_eq!(run["timeline"][1]["text"], "/skill:review");
        assert_eq!(run["queue"][0]["status"], "pending");
        run["queue"][0]["status"] = json!("consumed");
        reconcile(
            &mut run,
            &json!({"type":"node_activity","kind":"inbox","status":"completed","output":{"__zedflow:consumedMessages":["q"]}}),
            13,
        );
        assert_eq!(ids(&run), ["tool:c", "message:q", "message:a"]);
        assert_eq!(run["messages"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn repeated_text_and_namespace_collisions_do_not_remove_distinct_entries() {
        let mut run = json!({"messages":[{"id":"same","role":"user","text":"oui"},{"id":"other","role":"user","text":"oui"},{"role":"user","text":"oui"},{"role":"user","text":"oui"}]});
        reconcile(&mut run, &call("same"), 1);
        reconcile(&mut run, &call("same"), 2);
        assert_eq!(run["timeline"].as_array().unwrap().len(), 5);
        assert_eq!(
            ids(&run),
            [
                "message:same",
                "message:other",
                "message:legacy:2",
                "message:legacy:3",
                "tool:same"
            ]
        );
    }

    #[test]
    fn oversized_delta_output_remains_valid_utf8_and_bounded() {
        let mut run = json!({});
        reconcile(&mut run, &call("c"), 1);
        reconcile(
            &mut run,
            &json!({"type":"tool_progress","callId":"c","text":"🦀".repeat(20_000)}),
            2,
        );
        let output = run["timeline"][0]["activity"]["output"].as_str().unwrap();
        assert!(output.len() <= OUTPUT_LIMIT);
        assert!(output.ends_with('🦀'));
        assert_eq!(run["timeline"][0]["activity"]["truncated"], true);
    }

    #[test]
    fn replayed_progress_does_not_append_twice_or_regress_a_completed_tool() {
        let mut run = json!({});
        let progress = json!({"type":"tool_progress","callId":"c","text":"bonjour"});
        reconcile(&mut run, &call("c"), 1);
        reconcile(&mut run, &progress, 2);
        reconcile(&mut run, &progress, 2);
        reconcile(
            &mut run,
            &json!({"type":"tool_result","callId":"c","status":"completed","result":"ok"}),
            3,
        );
        reconcile(&mut run, &call("c"), 1);
        assert_eq!(run["timeline"][0]["activity"]["output"], "bonjour");
        assert_eq!(run["timeline"][0]["activity"]["status"], "completed");
        assert_eq!(run["timeline"][0]["seq"], 1);
    }

    #[test]
    fn legacy_replay_keeps_tool_order_messages_and_snapshot_only_results() {
        let mut run = json!({"messages":[{"id":"u","role":"user","text":"Question"},{"id":"a","role":"assistant","text":"Réponse"},{"role":"user","text":"Encore"},{"role":"assistant","text":"Réponse"}],"toolActivities":[{"callId":"c","name":"exec","status":"completed","result":{"content":"drained result"}},{"callId":"missing","name":"read","status":"completed"}],"queue":[]});
        let original_messages = run["messages"].clone();
        let original_tools = run["toolActivities"].clone();
        migrate(&mut run, &[(3, output("a", "Réponse")), (2, call("c"))]);
        assert_eq!(
            ids(&run),
            [
                "message:u",
                "tool:c",
                "message:a",
                "message:legacy:2",
                "message:legacy:3",
                "tool:missing"
            ]
        );
        assert_eq!(
            run["timeline"][1]["activity"]["result"]["content"],
            "drained result"
        );
        assert_eq!(run["messages"], original_messages);
        assert_eq!(run["toolActivities"], original_tools);
        assert_eq!(run["timelineApproximate"], true);
        let once = run.clone();
        migrate(&mut run, &[]);
        assert_eq!(run, once);
    }

    #[test]
    fn legacy_messages_without_ids_match_distinct_output_occurrences_once() {
        let mut run = json!({"messages":[{"role":"user","text":"Question"},{"role":"assistant","text":"Identique"},{"role":"user","text":"Encore"},{"role":"assistant","text":"Identique"}]});
        migrate(
            &mut run,
            &[
                (1, call("c1")),
                (2, output("a1", "Identique")),
                (3, call("c2")),
                (4, output("a2", "Identique")),
            ],
        );
        assert_eq!(
            ids(&run),
            [
                "message:legacy:0",
                "tool:c1",
                "message:legacy:1",
                "message:legacy:2",
                "tool:c2",
                "message:legacy:3"
            ]
        );
        assert_eq!(run["timeline"][2]["sourceId"], "message:a1");
        assert_eq!(run["timeline"][5]["sourceId"], "message:a2");
        reconcile(
            &mut run,
            &json!({"type":"run_status","status":"running"}),
            5,
        );
        reconcile(&mut run, &output("a1", "Identique"), 6);
        assert_eq!(run["timeline"].as_array().unwrap().len(), 6);
    }

    #[test]
    fn empty_and_cancelled_queue_events_do_not_invent_user_messages() {
        let mut run = json!({"messages":[],"queue":[{"id":"cancelled","role":"user","text":"Ignore","status":"cancelled"}]});
        reconcile(
            &mut run,
            &json!({"type":"node_activity","kind":"steering","status":"completed","output":{"__zedflow:consumedMessages":["cancelled","missing"]}}),
            1,
        );
        assert!(run["timeline"].as_array().unwrap().is_empty());
    }

    #[test]
    fn exact_origins_survive_interleaved_passages_and_tool_progress() {
        let first = json!({"type":"node_activity","kind":"tool","status":"running","path":"child/tools","occurrenceId":"pass-1"});
        let second = json!({"type":"node_activity","kind":"tool","status":"running","path":"other/tools","occurrenceId":"pass-2"});
        let mut run = json!({"activities":[first,second]});
        reconcile(&mut run, &first, 10);
        reconcile(&mut run, &second, 11);
        let origin = json!({"nodePath":"child/tools","occurrenceId":"pass-1"});
        let mut tool = call("c");
        tool["origin"] = origin.clone();
        reconcile(&mut run, &tool, 12);
        reconcile(
            &mut run,
            &json!({"type":"tool_progress","callId":"c","content":"update","origin":origin}),
            14,
        );
        assert_eq!(run["timeline"][0]["origin"], origin);
        assert_eq!(run["timeline"][0]["seq"], 12);
        assert_eq!(run["activities"][0]["startedSeq"], 10);
        assert_eq!(run["activities"][1]["startedSeq"], 11);
        let persisted = run.clone();
        run["activities"][0] =
            json!({"occurrenceId":"pass-1","status":"completed","output":{"ok":true}});
        retain_activity_order(&mut run, &persisted);
        let mut completed = first;
        completed["status"] = json!("completed");
        reconcile(&mut run, &completed, 15);
        assert_eq!(run["activities"][0]["startedSeq"], 10);
        assert_eq!(run["activities"][0]["endedSeq"], 15);
        assert_eq!(run["activities"][0]["output"]["ok"], true);
        assert_eq!(run["timeline"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn output_and_consumed_input_anchor_to_their_actual_passage_only() {
        let mut run = json!({"messages":[{"id":"initial","role":"user","text":"hello"}],"queue":[{"id":"q","text":"next","status":"pending"}]});
        let mut response = output("response-pass", "done");
        response["path"] = json!("child/output");
        reconcile(&mut run, &response, 6);
        assert!(run["timeline"][0].get("origin").is_none());
        assert_eq!(
            run["timeline"][1]["origin"],
            json!({"nodePath":"child/output","occurrenceId":"response-pass"})
        );
        reconcile(
            &mut run,
            &json!({"type":"node_activity","kind":"steering","status":"completed","path":"child/steering","occurrenceId":"input-pass","output":{"__zedflow:consumedMessages":["q"]}}),
            7,
        );
        assert_eq!(run["timeline"][2]["origin"]["occurrenceId"], "input-pass");
        let restored: Value = serde_json::from_slice(&serde_json::to_vec(&run).unwrap()).unwrap();
        assert_eq!(restored["timeline"], run["timeline"]);
    }
}
