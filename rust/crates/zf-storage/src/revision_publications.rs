//! Atomic persistence of validated revision publications. Payloads and outcomes
//! are opaque JSON; runtime owns their interpretation and compatibility checks.
use crate::content_store::ContentStore;
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// Storage coordinates for one already validated definition payload.
pub struct DefinitionWrite<'a> {
    pub run_id: &'a str,
    pub instance: &'a str,
    pub hash: &'a str,
    pub value: &'a Value,
}

/// Storage coordinates for one already validated runtime graph payload.
pub struct RuntimeGraphWrite<'a> {
    pub run_id: &'a str,
    pub value: &'a Value,
}

fn key(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

/// Commit all immutable definitions, heads and an optional replay receipt together.
/// The caller supplies the digest of the canonical publication arguments, including
/// the historical flow-only encoding. Replaying a matching receipt returns its
/// original opaque outcome without moving any head or altering active step pins.
///
/// # Errors
/// Returns an error on persistence failure or reuse of a batch identity with a
/// different digest. No content cache entry is marked committed before SQL commit.
pub async fn publish(
    store: &ContentStore,
    batch_id: Option<&str>,
    digest: &str,
    publications: &[DefinitionWrite<'_>],
    runtime_graphs: &[RuntimeGraphWrite<'_>],
    outcomes: &Value,
) -> Result<Value> {
    let mut prepared = Vec::new();
    for publication in publications {
        let definition = store.prepare(publication.value)?;
        let head = store.prepare(
            &json!({"instance":publication.instance,"definitionRef":definition.reference}),
        )?;
        prepared.push((definition, head));
    }
    let mut prepared_graphs = Vec::new();
    for publication in runtime_graphs {
        let definition = store.prepare(publication.value)?;
        let head = store.prepare(&json!({"graphRef":definition.reference}))?;
        prepared_graphs.push((definition, head));
    }
    let receipt = batch_id
        .map(|id| store.prepare(&json!({"batchId":id,"digest":digest,"compatibilities":outcomes})))
        .transpose()?;
    let mut tx = store.pool().begin_with("BEGIN IMMEDIATE").await?;
    if let Some(id) = batch_id {
        let prior:Option<String>=sqlx::query_scalar("SELECT value_ref FROM zf_records WHERE scope='__definition-publications' AND kind='batches' AND key=?").bind(id).fetch_optional(&mut *tx).await?;
        if let Some(reference) = prior {
            tx.rollback().await?;
            let prior = store.resolve(&reference).await?;
            ensure!(
                prior["digest"] == digest,
                "Publication batch identity reused with different arguments"
            );
            return Ok(prior["compatibilities"].clone());
        }
    }
    for (publication, (definition, head)) in publications.iter().zip(&prepared) {
        store.persist_in(&mut tx, definition).await?;
        store.persist_in(&mut tx, head).await?;
        sqlx::query("INSERT INTO zf_records(scope,kind,key,value_ref) VALUES(?,'revision-definitions',?,?) ON CONFLICT(scope,kind,key) DO NOTHING")
            .bind(publication.run_id).bind(key(&format!("{}\0{}",publication.instance,publication.hash))).bind(&definition.reference).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO zf_records(scope,kind,key,value_ref) VALUES(?,'revision-heads',?,?) ON CONFLICT(scope,kind,key) DO UPDATE SET value_ref=excluded.value_ref")
            .bind(publication.run_id).bind(key(publication.instance)).bind(&head.reference).execute(&mut *tx).await?;
    }
    for (publication, (definition, head)) in runtime_graphs.iter().zip(&prepared_graphs) {
        store.persist_in(&mut tx, definition).await?;
        store.persist_in(&mut tx, head).await?;
        sqlx::query("INSERT INTO zf_records(scope,kind,key,value_ref) VALUES(?,'runtime-graph-definitions',?,?) ON CONFLICT(scope,kind,key) DO NOTHING")
            .bind(publication.run_id).bind(&definition.reference).bind(&definition.reference).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO zf_records(scope,kind,key,value_ref) VALUES(?,'runtime-graph-heads','current',?) ON CONFLICT(scope,kind,key) DO UPDATE SET value_ref=excluded.value_ref")
            .bind(publication.run_id).bind(&head.reference).execute(&mut *tx).await?;
    }
    if let (Some(id), Some(receipt)) = (batch_id, &receipt) {
        store.persist_in(&mut tx, receipt).await?;
        sqlx::query("INSERT INTO zf_records(scope,kind,key,value_ref) VALUES('__definition-publications','batches',?,?)").bind(id).bind(&receipt.reference).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    if let Some(receipt) = &receipt {
        store.mark_committed(receipt);
    }
    for (definition, head) in prepared.iter().chain(&prepared_graphs) {
        store.mark_committed(definition);
        store.mark_committed(head);
    }
    Ok(outcomes.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn store() -> ContentStore {
        ContentStore::new(
            sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect("sqlite::memory:")
                .await
                .unwrap(),
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn mixed_publication_preserves_exact_values_and_replay_never_rewinds_heads() {
        let store = store().await;
        let definition = json!({"source":"é漢🙂\r\n", "extension":{"large":18446744073709551615u64}, "hash":"v1"});
        let graph = json!({"flows":{"root":definition},"future":[null,{},[]]});
        let outcomes =
            json!([{"kind":"live"},{"kind":"sequentialBoundary","reasons":["structure"]}]);
        let definitions = [DefinitionWrite {
            run_id: "run",
            instance: "root",
            hash: "v1",
            value: &definition,
        }];
        let graphs = [RuntimeGraphWrite {
            run_id: "run",
            value: &graph,
        }];
        let pin = json!({"hash":"already-selected","step":2});
        store
            .put_record("run", "revision-steps", "pin", &pin)
            .await
            .unwrap();
        assert_eq!(
            publish(
                &store,
                Some("batch"),
                "digest",
                &definitions,
                &graphs,
                &outcomes
            )
            .await
            .unwrap(),
            outcomes
        );
        assert_eq!(
            store
                .record("run", "revision-definitions", &key("root\0v1"))
                .await
                .unwrap()
                .unwrap(),
            definition
        );
        let head = store
            .record("run", "runtime-graph-heads", "current")
            .await
            .unwrap()
            .unwrap();
        let graph_ref = head["graphRef"].as_str().unwrap();
        assert_eq!(store.resolve(graph_ref).await.unwrap(), graph);
        assert_eq!(
            store
                .record("run", "runtime-graph-definitions", graph_ref)
                .await
                .unwrap()
                .unwrap(),
            graph
        );
        let next = json!({"hash":"v2","source":"next"});
        let next_graph = json!({"flows":{"root":next}});
        publish(
            &store,
            Some("next"),
            "next-digest",
            &[DefinitionWrite {
                run_id: "run",
                instance: "root",
                hash: "v2",
                value: &next,
            }],
            &[RuntimeGraphWrite {
                run_id: "run",
                value: &next_graph,
            }],
            &json!([]),
        )
        .await
        .unwrap();
        let advanced = store
            .records("run")
            .await
            .unwrap()
            .into_iter()
            .map(|r| (r.kind, r.key, r.value_ref))
            .collect::<Vec<_>>();
        assert_eq!(
            publish(
                &store,
                Some("batch"),
                "digest",
                &definitions,
                &graphs,
                &json!(["new-outcome"])
            )
            .await
            .unwrap(),
            outcomes
        );
        assert_eq!(
            store
                .records("run")
                .await
                .unwrap()
                .into_iter()
                .map(|r| (r.kind, r.key, r.value_ref))
                .collect::<Vec<_>>(),
            advanced
        );
        assert!(
            publish(
                &store,
                Some("batch"),
                "different",
                &definitions,
                &graphs,
                &outcomes
            )
            .await
            .unwrap_err()
            .to_string()
            .contains("reused")
        );
        assert_eq!(
            store
                .record("run", "revision-steps", "pin")
                .await
                .unwrap()
                .unwrap(),
            pin
        );
    }

    #[tokio::test]
    async fn failure_rolls_back_both_heads_content_and_receipt_without_poisoning_cache() {
        let store = store().await;
        let definition = json!({"hash":"v1","source":"exact"});
        let graph = json!({"flows":{"root":definition}});
        let definitions = [DefinitionWrite {
            run_id: "run",
            instance: "root",
            hash: "v1",
            value: &definition,
        }];
        let graphs = [RuntimeGraphWrite {
            run_id: "run",
            value: &graph,
        }];
        sqlx::query("CREATE TRIGGER reject_graph_head BEFORE INSERT ON zf_records WHEN NEW.kind='runtime-graph-heads' BEGIN SELECT RAISE(ABORT,'injected publication failure'); END").execute(store.pool()).await.unwrap();
        assert!(
            publish(
                &store,
                Some("batch"),
                "digest",
                &definitions,
                &graphs,
                &json!([])
            )
            .await
            .is_err()
        );
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM zf_records")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(count, 0);
        let content: i64 = sqlx::query_scalar("SELECT count(*) FROM zf_content")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(content, 0);
        sqlx::query("DROP TRIGGER reject_graph_head")
            .execute(store.pool())
            .await
            .unwrap();
        publish(
            &store,
            Some("batch"),
            "digest",
            &definitions,
            &graphs,
            &json!([]),
        )
        .await
        .unwrap();
        let head = store
            .record("run", "revision-heads", &key("root"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            store
                .resolve(head["definitionRef"].as_str().unwrap())
                .await
                .unwrap(),
            definition
        );
        assert!(
            store
                .record("__definition-publications", "batches", "batch")
                .await
                .unwrap()
                .is_some()
        );
    }

    #[tokio::test]
    async fn historical_receipt_remains_replayable_without_new_definitions() {
        let store = store().await;
        let outcomes = json!([{"kind":"live"}]);
        store.put_record("__definition-publications","batches","legacy",&json!({"batchId":"legacy","digest":"legacy-flow-only-digest","compatibilities":outcomes})).await.unwrap();
        let definition = json!({"source":"must never be installed"});
        assert_eq!(
            publish(
                &store,
                Some("legacy"),
                "legacy-flow-only-digest",
                &[DefinitionWrite {
                    run_id: "run",
                    instance: "root",
                    hash: "old",
                    value: &definition
                }],
                &[],
                &json!([])
            )
            .await
            .unwrap(),
            outcomes
        );
        assert!(store.records("run").await.unwrap().is_empty());
    }
}
