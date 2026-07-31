use std::{
    io::{BufRead, BufReader, Read},
    net::TcpListener,
    thread,
};
use zedflow_orchestrator::{
    radius::RadiusPresence,
    types::{InstanceRecord, InstanceStatus},
};

#[tokio::test]
async fn registers_and_disconnects_radius_resources() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut paths = Vec::new();
        for response in [
            r#"{"id":"machine-1","heartbeatIntervalMs":60000,"expiresInMs":120000}"#,
            r#"{"id":"pi-1","heartbeatIntervalMs":60000,"expiresInMs":120000}"#,
            "{}",
            "{}",
        ] {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            paths.push(first.split_whitespace().nth(1).unwrap().to_owned());
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
    radius.disconnect_pi(&instance).await.unwrap();
    radius.stop().await.unwrap();
    assert_eq!(
        server.join().unwrap(),
        [
            "/v1/machines/register",
            "/v1/pis/register",
            "/v1/pis/pi-1/disconnect",
            "/v1/machines/machine-1/disconnect"
        ]
    );
    unsafe {
        std::env::remove_var("PI_RADIUS_API_KEY");
        std::env::remove_var("PI_RADIUS_ORCHESTRATOR_URL");
        std::env::remove_var("PI_ORCHESTRATOR_DIR");
    }
}
