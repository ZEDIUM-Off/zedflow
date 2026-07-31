use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    sync::Mutex,
};
use zedflow_orchestrator::{
    radius::RadiusPresence,
    types::{InstanceRecord, InstanceStatus},
};

static ENV: Mutex<()> = Mutex::new(());

fn server() -> (String, std::thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1/", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        let mut paths = Vec::new();
        for request_number in 0..8 {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            paths.push(line.split_whitespace().nth(1).unwrap().to_owned());
            let mut content_length = 0;
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some(value) = line.strip_prefix("Content-Length: ") {
                    content_length = value.trim().parse().unwrap();
                }
            }
            let mut body = vec![0; content_length];
            reader.read_exact(&mut body).unwrap();
            let (status, body) = match request_number {
                0 => (
                    "200 OK",
                    r#"{"id":"machine-1","heartbeatIntervalMs":60000,"expiresInMs":120000}"#,
                ),
                1 => (
                    "200 OK",
                    r#"{"id":"pi-1","heartbeatIntervalMs":60000,"expiresInMs":120000}"#,
                ),
                2..=4 => ("404 Not Found", "missing"),
                5 => (
                    "200 OK",
                    r#"{"id":"pi-2","heartbeatIntervalMs":60000,"expiresInMs":120000}"#,
                ),
                _ => ("204 No Content", ""),
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream
                .try_clone()
                .unwrap()
                .write_all(response.as_bytes())
                .unwrap();
        }
        paths
    });
    (url, handle)
}

#[test]
fn registration_recovers_pi_after_three_404s_and_disconnects() {
    let _guard = ENV.lock().unwrap();
    let (url, server) = server();
    let dir = std::env::temp_dir().join(format!("zedflow-radius-{}", std::process::id()));
    unsafe {
        std::env::set_var("PI_RADIUS_ORCHESTRATOR_URL", url);
        std::env::set_var("PI_RADIUS_API_KEY", "test-token");
        std::env::set_var("PI_ORCHESTRATOR_DIR", &dir);
    }
    let radius = RadiusPresence::default();
    assert_eq!(
        radius.start(Some("local".into())).unwrap().unwrap().id,
        "machine-1"
    );
    let instance = InstanceRecord {
        id: "local-pi".into(),
        status: InstanceStatus::Online,
        cwd: "/tmp".into(),
        created_at: "now".into(),
        last_seen_at: None,
        label: None,
        session_id: None,
        session_file: None,
        radius_pi_id: None,
    };
    let instance = radius.register_pi(instance).unwrap();
    for _ in 0..3 {
        radius.heartbeat_pi("local-pi").unwrap();
    }
    radius
        .disconnect_pi(&InstanceRecord {
            radius_pi_id: Some("pi-2".into()),
            ..instance
        })
        .unwrap();
    radius.stop().unwrap();
    assert_eq!(
        server.join().unwrap(),
        vec![
            "/v1/machines/register",
            "/v1/pis/register",
            "/v1/pis/pi-1/heartbeat",
            "/v1/pis/pi-1/heartbeat",
            "/v1/pis/pi-1/heartbeat",
            "/v1/pis/register",
            "/v1/pis/pi-2/disconnect",
            "/v1/machines/machine-1/disconnect"
        ]
    );
    unsafe {
        std::env::remove_var("PI_RADIUS_ORCHESTRATOR_URL");
        std::env::remove_var("PI_RADIUS_API_KEY");
        std::env::remove_var("PI_ORCHESTRATOR_DIR");
    }
}
