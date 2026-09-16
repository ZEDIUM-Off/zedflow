//! WebRTC subscription transport. ADK execution and response commands stay in the application API.
use anyhow::{Context, Result, bail};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Semaphore, watch};
use webrtc::{
    data_channel::{DataChannel, DataChannelEvent},
    peer_connection::{
        PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler, RTCConfigurationBuilder,
        RTCIceGatheringState, RTCIceServer, RTCIceTransportPolicy, RTCPeerConnectionState,
        RTCSdpType, RTCSessionDescription,
    },
};
use zf_storage::session_sync::SessionSync;

const CHUNK_BYTES: usize = 12 * 1024;
const NEGOTIATION_TIMEOUT: Duration = Duration::from_secs(30);

fn local_udp_addresses() -> Result<Vec<SocketAddr>> {
    // webrtc-rs advertises the exact bound address. A wildcard bind produces
    // unusable 0.0.0.0 candidates that Chrome correctly ignores.
    let mut addresses: Vec<_> = if_addrs::get_if_addrs()
        .context("enumerate local ICE interfaces")?
        .into_iter()
        .filter(|interface| {
            (interface.is_oper_up()
                || interface.is_loopback()
                || interface.oper_status == if_addrs::IfOperStatus::Unknown)
                && !interface.is_link_local()
                && !interface.ip().is_unspecified()
                && !interface.ip().is_multicast()
        })
        .map(|interface| SocketAddr::new(interface.ip(), 0))
        .collect();
    addresses.sort_unstable();
    addresses.dedup();
    if addresses.is_empty() {
        bail!("no usable local interface for ICE");
    }
    Ok(addresses)
}

#[derive(Clone)]
struct RtcBackend {
    sync: SessionSync,
    config: ClientConfig,
    peers: Arc<Semaphore>,
}

/// ICE credentials are explicitly provided for clients, never model-provider credentials.
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientConfig {
    ice_servers: Vec<IceServer>,
    ice_transport_policy: String,
}

#[derive(Clone, Deserialize, Serialize)]
struct IceServer {
    urls: Vec<String>,
    #[serde(default)]
    username: String,
    #[serde(default)]
    credential: String,
}

impl ClientConfig {
    fn from_env() -> Result<Self> {
        let servers = std::env::var("ZEDFLOW_ICE_SERVERS").unwrap_or_else(|_| "[]".into());
        let ice_servers: Vec<IceServer> = serde_json::from_str(&servers)
            .context("ZEDFLOW_ICE_SERVERS must be a JSON array of ICE servers")?;
        for server in &ice_servers {
            RTCIceServer::from(server)
                .urls()
                .context("invalid ICE server configuration")?;
        }
        let policy = std::env::var("ZEDFLOW_ICE_TRANSPORT_POLICY").unwrap_or_else(|_| "all".into());
        if !matches!(policy.as_str(), "all" | "relay") {
            bail!("ZEDFLOW_ICE_TRANSPORT_POLICY must be all or relay");
        }
        if policy == "relay"
            && !ice_servers.iter().any(|s| {
                s.urls
                    .iter()
                    .any(|u| u.starts_with("turn:") || u.starts_with("turns:"))
            })
        {
            bail!("relay transport requires a TURN server in ZEDFLOW_ICE_SERVERS");
        }
        Ok(Self {
            ice_servers,
            ice_transport_policy: policy,
        })
    }
}

impl From<&IceServer> for RTCIceServer {
    fn from(value: &IceServer) -> Self {
        Self {
            urls: value.urls.clone(),
            username: value.username.clone(),
            credential: value.credential.clone(),
        }
    }
}

/// Returns a router with independent state, ready to merge into the daemon router.
pub fn router(sync: SessionSync) -> Result<Router> {
    let backend = RtcBackend {
        sync,
        config: ClientConfig::from_env()?,
        peers: Arc::new(Semaphore::new(32)),
    };
    Ok(Router::new()
        .route("/api/rtc/config", get(config))
        .route("/api/runs/{id}/rtc", post(offer))
        .with_state(backend))
}

async fn config(State(backend): State<RtcBackend>) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CACHE_CONTROL,
        "no-store".parse().expect("static header"),
    );
    (headers, Json(backend.config))
}

#[derive(Deserialize)]
struct Offer {
    #[serde(flatten)]
    description: RTCSessionDescription,
    #[serde(default)]
    after: i64,
}

async fn offer(
    State(backend): State<RtcBackend>,
    Path(run): Path<String>,
    Json(offer): Json<Offer>,
) -> Result<Json<RTCSessionDescription>, (StatusCode, Json<Value>)> {
    negotiate(backend, run, offer)
        .await
        .map(Json)
        .map_err(|error| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("{error:#}")})),
            )
        })
}

async fn negotiate(
    backend: RtcBackend,
    run: String,
    offer: Offer,
) -> Result<RTCSessionDescription> {
    if offer.description.sdp_type != RTCSdpType::Offer || offer.after < 0 {
        bail!("expected a WebRTC offer and a nonnegative event cursor");
    }
    backend.sync.latest(&run).await?;
    let permit = backend
        .peers
        .try_acquire_owned()
        .context("WebRTC connection limit reached")?;
    let (gathered, mut gather_rx) = watch::channel(false);
    let (closed, mut close_rx) = watch::channel(false);
    let (opened, mut open_rx) = watch::channel(false);
    let handler = Arc::new(SubscriptionHandler {
        sync: backend.sync,
        run,
        after: offer.after,
        gathered,
        closed: closed.clone(),
        opened,
        channel_claimed: AtomicBool::new(false),
    });
    let configuration = RTCConfigurationBuilder::new()
        .with_ice_servers(
            backend
                .config
                .ice_servers
                .iter()
                .map(RTCIceServer::from)
                .collect(),
        )
        .with_ice_transport_policy(if backend.config.ice_transport_policy == "relay" {
            RTCIceTransportPolicy::Relay
        } else {
            RTCIceTransportPolicy::All
        })
        .build();
    let peer: Arc<dyn PeerConnection> = Arc::new(
        PeerConnectionBuilder::new()
            .with_configuration(configuration)
            .with_handler(handler)
            .with_udp_addrs(local_udp_addresses()?)
            .with_data_channel_send_buffer_limit(256 * 1024)
            .build()
            .await
            .context("create WebRTC peer")?,
    );

    // This task owns the peer lifetime even if the signaling HTTP request is cancelled.
    // The handler does not retain the peer, avoiding an Arc cycle.
    let lifetime_peer = Arc::clone(&peer);
    tokio::spawn(async move {
        let _permit = permit;
        let opened = tokio::time::timeout(NEGOTIATION_TIMEOUT, async {
            tokio::select! {
                _ = wait_true(&mut open_rx) => true,
                _ = wait_true(&mut close_rx) => false,
            }
        })
        .await
        .unwrap_or(false);
        if opened {
            let _ = close_rx.wait_for(|value| *value).await;
        }
        closed.send_replace(true);
        if let Err(error) = lifetime_peer.close().await {
            eprintln!("WebRTC close failed: {error}");
        }
    });

    let answer = async {
        peer.set_remote_description(offer.description)
            .await
            .context("apply WebRTC offer")?;
        let answer = peer
            .create_answer(None)
            .await
            .context("create WebRTC answer")?;
        peer.set_local_description(answer)
            .await
            .context("apply local WebRTC answer")?;
        tokio::time::timeout(Duration::from_secs(15), gather_rx.wait_for(|value| *value))
            .await
            .context("ICE gathering timed out; check the configured STUN/TURN servers")??;
        peer.local_description()
            .await
            .context("WebRTC answer is missing")
    }
    .await;
    if answer.is_err() {
        // close() releases sockets immediately; the lifetime task will observe Closed.
        if let Err(error) = peer.close().await {
            eprintln!("WebRTC negotiation cleanup failed: {error}");
        }
    }
    answer
}

struct SubscriptionHandler {
    sync: SessionSync,
    run: String,
    after: i64,
    gathered: watch::Sender<bool>,
    closed: watch::Sender<bool>,
    opened: watch::Sender<bool>,
    channel_claimed: AtomicBool,
}

#[async_trait::async_trait]
impl PeerConnectionEventHandler for SubscriptionHandler {
    async fn on_ice_gathering_state_change(&self, state: RTCIceGatheringState) {
        if state == RTCIceGatheringState::Complete {
            self.gathered.send_replace(true);
        }
    }

    async fn on_connection_state_change(&self, state: RTCPeerConnectionState) {
        if matches!(
            state,
            RTCPeerConnectionState::Disconnected
                | RTCPeerConnectionState::Failed
                | RTCPeerConnectionState::Closed
        ) {
            self.closed.send_replace(true);
        }
    }

    async fn on_data_channel(&self, channel: Arc<dyn DataChannel>) {
        if channel.label().await.as_deref() != Ok("zedflow-events")
            || self.channel_claimed.swap(true, Ordering::AcqRel)
        {
            if let Err(error) = channel.close().await {
                eprintln!("Cannot reject WebRTC data channel: {error}");
            }
            return;
        }
        let sync = self.sync.clone();
        let run = self.run.clone();
        let after = self.after;
        let closed = self.closed.clone();
        let opened = self.opened.clone();
        tokio::spawn(async move {
            if let Err(error) =
                subscribe(&channel, &sync, &run, after, &opened, closed.subscribe()).await
            {
                let _ = channel
                    .send_text(&json!({"error":format!("{error:#}")}).to_string())
                    .await;
                eprintln!("WebRTC subscription failed: {error:#}");
            }
            closed.send_replace(true);
        });
    }
}

async fn subscribe(
    channel: &Arc<dyn DataChannel>,
    sync: &SessionSync,
    run: &str,
    mut after: i64,
    opened: &watch::Sender<bool>,
    mut closed: watch::Receiver<bool>,
) -> Result<()> {
    loop {
        tokio::select! {
            _ = sync.shutdown.cancelled() => return Ok(()),
            _ = wait_true(&mut closed) => return Ok(()),
            event = channel.poll() => match event {
                Some(DataChannelEvent::OnOpen) => { opened.send_replace(true); break; }
                Some(DataChannelEvent::OnClose) | None => return Ok(()),
                _ => {}
            }
        }
    }
    let mut changed = sync.subscribe();
    let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut ready = true;
    loop {
        if ready {
            let payload = sync.payload(run, Some(after)).await?;
            let next = payload["cursor"].as_i64().context("missing sync cursor")?;
            tokio::time::timeout(
                Duration::from_secs(10),
                send_payload(channel, &payload.to_string()),
            )
            .await
            .context("WebRTC client is not consuming events")??;
            after = next;
            let (_, head) = sync.head(run).await?;
            if after < head {
                continue;
            }
            ready = false;
        }
        tokio::select! {
            _=sync.shutdown.cancelled()=>return Ok(()),
            _=wait_true(&mut closed)=>return Ok(()),
            event=channel.poll()=>if matches!(event,Some(DataChannelEvent::OnClose)|None){return Ok(());},
            result=changed.changed()=> { if result.is_err(){return Ok(());} ready=true; },
            _=heartbeat.tick()=>ready=true,
        }
    }
}

async fn wait_true(receiver: &mut watch::Receiver<bool>) {
    let _ = receiver.wait_for(|value| *value).await;
}

fn chunks(text: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        let mut end = remaining.len().min(CHUNK_BYTES);
        while !remaining.is_char_boundary(end) {
            end -= 1;
        }
        result.push(&remaining[..end]);
        remaining = &remaining[end..];
    }
    result
}

async fn send_payload(channel: &Arc<dyn DataChannel>, text: &str) -> Result<()> {
    if text.len() <= CHUNK_BYTES {
        channel.send_text(text).await?;
    } else {
        let pieces = chunks(text);
        let id = uuid::Uuid::new_v4().to_string();
        for (index, piece) in pieces.iter().enumerate() {
            let packet =
                json!({"type":"chunk","id":id,"index":index,"total":pieces.len(),"data":piece});
            channel.send_text(&packet.to_string()).await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_unicode_snapshot_round_trips_under_message_limit() {
        let text = json!({"output":"🦀 réponse\n\"".repeat(15000)}).to_string();
        let pieces = chunks(&text);
        assert!(pieces.len() > 1);
        assert_eq!(pieces.concat(), text);
        for (index, piece) in pieces.iter().enumerate() {
            let packet =
                json!({"type":"chunk","id":"test","index":index,"total":pieces.len(),"data":piece})
                    .to_string();
            assert!(packet.len() < 64 * 1024);
        }
    }
}
