//! Public signaling + actual ICE/DTLS/SCTP transport, including reconnect cursors and fragmentation.
use anyhow::{Context, Result};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::watch;
use tower::ServiceExt;
use webrtc::{
    data_channel::{DataChannel, DataChannelEvent},
    peer_connection::{
        PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler, RTCIceGatheringState,
    },
};

struct Gather(watch::Sender<bool>);

#[async_trait::async_trait]
impl PeerConnectionEventHandler for Gather {
    async fn on_ice_gathering_state_change(&self, state: RTCIceGatheringState) {
        if state == RTCIceGatheringState::Complete {
            self.0.send_replace(true);
        }
    }
}

async fn receive(channel: &Arc<dyn DataChannel>) -> Result<Value> {
    let mut partials: HashMap<String, Vec<String>> = HashMap::new();
    tokio::time::timeout(Duration::from_secs(15), async {
        while let Some(event) = channel.poll().await {
            if let DataChannelEvent::OnMessage(message) = event {
                let value: Value = serde_json::from_slice(&message.data)?;
                if value["type"] != "chunk" {
                    return Ok(value);
                }
                let id = value["id"]
                    .as_str()
                    .context("fragment identity")?
                    .to_owned();
                let total = value["total"].as_u64().context("fragment count")? as usize;
                let index = value["index"].as_u64().context("fragment index")? as usize;
                let chunks = partials
                    .entry(id)
                    .or_insert_with(|| vec![String::new(); total]);
                chunks[index] = value["data"].as_str().context("fragment data")?.to_owned();
                if chunks.iter().all(|s| !s.is_empty()) {
                    return Ok(serde_json::from_str(&chunks.concat())?);
                }
            }
        }
        anyhow::bail!("data channel closed before receiving snapshot")
    })
    .await?
}

#[tokio::test]
async fn real_datachannel_replays_after_cursor_and_observes_waiting_run() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let db = SqlitePool::connect(&format!(
        "sqlite://{}?mode=rwc",
        directory.path().join("rtc.db").display()
    ))
    .await?;
    sqlx::query("CREATE TABLE runs(id TEXT PRIMARY KEY, document TEXT NOT NULL)")
        .execute(&db)
        .await?;
    sqlx::query("CREATE TABLE events(seq INTEGER PRIMARY KEY AUTOINCREMENT, run TEXT NOT NULL, document TEXT NOT NULL)").execute(&db).await?;
    let large = "🦀 sorties de nœud\n".repeat(10000);
    let mut run =
        json!({"id":"rtc-test","status":"running","activeNode":"model","state":{"large":large}});
    sqlx::query("INSERT INTO runs VALUES(?,?)")
        .bind("rtc-test")
        .bind(run.to_string())
        .execute(&db)
        .await?;
    for node in ["old", "model"] {
        sqlx::query("INSERT INTO events(run,document) VALUES(?,?)")
            .bind("rtc-test")
            .bind(json!({"NodeStart":{"node":node}}).to_string())
            .execute(&db)
            .await?;
    }
    zf_storage::session_store::initialize(&db).await?;
    let content = zf_storage::content_store::ContentStore::new(db.clone()).await?;
    let sync = zf_storage::session_sync::SessionSync::new(
        db.clone(),
        content.clone(),
        std::sync::Arc::new(zf_runtime::archive_validation::RuntimeArchiveValidation),
    );
    sync.seed("rtc-test").await?;
    let router = zf_serve::rtc::router(sync.clone())?;
    let (gather, mut gather_rx) = watch::channel(false);
    let peer = PeerConnectionBuilder::new()
        .with_handler(Arc::new(Gather(gather)))
        .with_udp_addrs(vec!["127.0.0.1:0"])
        .build()
        .await?;
    let channel = peer.create_data_channel("zedflow-events", None).await?;
    let offer = peer.create_offer(None).await?;
    peer.set_local_description(offer).await?;
    tokio::time::timeout(Duration::from_secs(10), gather_rx.wait_for(|v| *v)).await??;
    let mut offer = serde_json::to_value(peer.local_description().await.context("offer missing")?)?;
    offer["after"] = json!(1);
    let response = router
        .oneshot(
            Request::post("/api/runs/rtc-test/rtc")
                .header("content-type", "application/json")
                .body(Body::from(offer.to_string()))?,
        )
        .await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1_000_000).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let answer: webrtc::peer_connection::RTCSessionDescription = serde_json::from_slice(&bytes)?;
    let candidates: Vec<_> = answer
        .sdp
        .lines()
        .filter(|line| line.starts_with("a=candidate:"))
        .collect();
    assert!(
        !candidates.is_empty(),
        "answer needs concrete ICE candidates"
    );
    for candidate in candidates {
        let address = candidate
            .split_whitespace()
            .nth(4)
            .context("candidate address")?;
        assert!(
            !address.parse::<std::net::IpAddr>()?.is_unspecified(),
            "Chrome ignores unspecified ICE candidates: {candidate}"
        );
    }
    peer.set_remote_description(answer).await?;

    let first = receive(&channel).await?;
    assert_eq!(first["run"]["activeNode"], "model");
    assert_eq!(first["type"], "bootstrap");
    assert_eq!(first["run"]["state"], json!({}));
    assert!(first.get("events").is_none());
    assert_eq!(first["cursor"], 2);

    // The same subscription stays alive across a persisted human-input wait.
    run["status"] = json!("waiting");
    run["activeNode"] = json!("input");
    sqlx::query("UPDATE runs SET document=? WHERE id='rtc-test'")
        .bind(run.to_string())
        .execute(&db)
        .await?;
    let operations =
        json!([{"collection":"meta","value":{"status":"waiting","activeNode":"input"}}]);
    let reference = content.intern(&operations).await?;
    sqlx::query("INSERT INTO run_changes(seq,run,base_revision,document) VALUES(3,'rtc-test',2,?)")
        .bind(json!({"valueRef":reference}).to_string())
        .execute(&db)
        .await?;
    sync.committed("rtc-test", run.clone(), 3).await;
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let payload = receive(&channel).await?;
            if payload["type"] == "delta" && payload["ops"][0]["value"]["status"] == "waiting" {
                assert_eq!(payload["baseRevision"], 2);
                assert_eq!(payload["revision"], 3);
                assert!(payload.get("run").is_none());
                return Ok::<_, anyhow::Error>(());
            }
        }
    })
    .await??;
    channel.close().await?;
    peer.close().await?;
    db.close().await;
    Ok(())
}
