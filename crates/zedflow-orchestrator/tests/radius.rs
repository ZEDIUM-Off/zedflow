use std::{
    ffi::OsString,
    io::{BufRead, BufReader, Read},
    net::TcpListener,
    sync::{Mutex, OnceLock},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use zedflow_orchestrator::{
    radius::{RadiusPresence, radius_access_token},
    types::{InstanceRecord, InstanceStatus},
};

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct EnvGuard(Vec<(&'static str, Option<OsString>)>);

impl EnvGuard {
    fn new(names: &[&'static str]) -> Self {
        Self(
            names
                .iter()
                .map(|&name| (name, std::env::var_os(name)))
                .collect(),
        )
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (name, value) in &self.0 {
            unsafe {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }
}

#[test]
fn reads_oauth_credentials_from_pi_config_agent_directory() {
    let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    let _environment =
        EnvGuard::new(&["PI_CONFIG_DIR", "PI_ORCHESTRATOR_DIR", "PI_RADIUS_API_KEY"]);
    let temp = std::env::temp_dir().join(format!(
        "zedflow-radius-auth-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(temp.join("agent")).unwrap();
    std::fs::write(
        temp.join("agent/auth.json"),
        r#"{"radius":{"type":"oauth","access":"stored-token"}}"#,
    )
    .unwrap();
    unsafe {
        std::env::set_var("PI_CONFIG_DIR", &temp);
        std::env::set_var("PI_ORCHESTRATOR_DIR", temp.join("orchestrator"));
        std::env::remove_var("PI_RADIUS_API_KEY");
    }

    assert_eq!(radius_access_token().unwrap(), "stored-token");
    std::fs::remove_dir_all(temp).unwrap();
}

#[tokio::test]
async fn accepts_empty_2xx_heartbeat_and_disconnect_responses() {
    let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    let _environment = EnvGuard::new(&[
        "PI_RADIUS_API_KEY",
        "PI_RADIUS_ORCHESTRATOR_URL",
        "PI_ORCHESTRATOR_DIR",
    ]);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut paths = Vec::new();
        let deadline = Instant::now() + Duration::from_millis(250);
        while Instant::now() < deadline {
            let Ok((mut stream, _)) = listener.accept() else {
                thread::sleep(Duration::from_millis(1));
                continue;
            };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            let Some(path) = first.split_whitespace().nth(1).map(str::to_owned) else {
                continue;
            };
            paths.push(path.clone());
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some(value) = line
                    .strip_prefix("content-length:")
                    .or_else(|| line.strip_prefix("Content-Length:"))
                {
                    length = value.trim().parse().unwrap();
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let response = match path.as_str() {
                "/v1/machines/register" => {
                    r#"{"id":"machine-1","heartbeatIntervalMs":1,"expiresInMs":120000}"#
                }
                "/v1/pis/register" => {
                    r#"{"id":"pi-1","heartbeatIntervalMs":1,"expiresInMs":120000}"#
                }
                _ => "",
            };
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.len(),
                response
            );
            std::io::Write::write_all(&mut stream, reply.as_bytes()).unwrap();
        }
        paths
    });
    let temp = std::env::temp_dir().join(format!("zedflow-radius-{}", std::process::id()));
    unsafe {
        std::env::set_var("PI_RADIUS_API_KEY", "test-token");
        std::env::set_var(
            "PI_RADIUS_ORCHESTRATOR_URL",
            format!("http://{address}/v1/"),
        );
        std::env::set_var("PI_ORCHESTRATOR_DIR", &temp);
    }
    let radius = RadiusPresence::default();
    assert_eq!(
        radius
            .start(Some("laptop".into()))
            .await
            .unwrap()
            .unwrap()
            .id,
        "machine-1"
    );
    let instance = radius
        .register_pi(InstanceRecord {
            id: "local-1".into(),
            status: InstanceStatus::Online,
            cwd: "/tmp".into(),
            created_at: "0".into(),
            last_seen_at: None,
            label: None,
            session_id: None,
            session_file: None,
            radius_pi_id: None,
        })
        .await
        .unwrap();
    assert_eq!(instance.radius_pi_id.as_deref(), Some("pi-1"));
    tokio::time::sleep(Duration::from_millis(50)).await;
    radius.disconnect_pi(&instance).await.unwrap();
    radius.stop().await.unwrap();
    let paths = server.join().unwrap();
    assert!(
        paths
            .iter()
            .filter(|path| path.as_str() == "/v1/machines/machine-1/heartbeat")
            .count()
            >= 2
    );
    assert!(
        paths
            .iter()
            .filter(|path| path.as_str() == "/v1/pis/pi-1/heartbeat")
            .count()
            >= 2
    );
    assert!(paths.iter().any(|path| path == "/v1/pis/pi-1/disconnect"));
    assert!(
        paths
            .iter()
            .any(|path| path == "/v1/machines/machine-1/disconnect")
    );
}
