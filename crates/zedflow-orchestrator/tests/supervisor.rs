use serde_json::json;
use std::{
    fs,
    io::{BufRead, BufReader, Read},
    net::TcpListener,
    os::unix::fs::PermissionsExt,
    sync::Mutex,
    thread,
    time::Duration,
};

static ENV_LOCK: Mutex<()> = Mutex::new(());
use zedflow_orchestrator::{supervisor::OrchestratorSupervisor, types::InstanceStatus};

#[tokio::test]
async fn syncs_session_metadata_and_marks_exited_process_error() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let dir = std::env::temp_dir().join(format!("zedflow-supervisor-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let command = dir.join("rpc.sh");
    fs::write(
        &command,
        r#"#!/bin/sh
prompt=0
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
  case "$line" in
    *'"type":"get_state"'*)
      printf '{"type":"response","id":"%s","success":true,"command":"get_state","data":{"sessionId":"session-1","sessionFile":"/tmp/session.jsonl"}}\n' "$id"
      [ "$prompt" -eq 1 ] && exit 0
      ;;
    *'"type":"prompt"'*)
      prompt=1
      printf '{"type":"response","id":"%s","success":true,"command":"prompt"}\n' "$id"
      ;;
  esac
done
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&command).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&command, permissions).unwrap();
    unsafe {
        std::env::set_var("PI_ORCHESTRATOR_DIR", &dir);
        std::env::set_var("PI_ORCHESTRATOR_RPC_COMMAND", &command);
    }

    let mut supervisor = OrchestratorSupervisor::new();
    let instance = supervisor
        .spawn_instance(dir.to_string_lossy().into_owned(), None)
        .await
        .unwrap();
    assert_eq!(instance.session_id.as_deref(), Some("session-1"));
    assert_eq!(instance.session_file.as_deref(), Some("/tmp/session.jsonl"));
    supervisor
        .handle_rpc(&instance.id, json!({"type":"prompt"}))
        .unwrap();

    for _ in 0..50 {
        if matches!(
            supervisor
                .get_instance(&instance.id)
                .unwrap()
                .unwrap()
                .status,
            InstanceStatus::Error
        ) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        supervisor
            .get_instance(&instance.id)
            .unwrap()
            .unwrap()
            .status,
        InstanceStatus::Error
    );
    assert!(
        supervisor
            .handle_rpc(&instance.id, json!({"type":"get_state"}))
            .unwrap()
            .is_none()
    );
    unsafe {
        std::env::remove_var("PI_ORCHESTRATOR_DIR");
        std::env::remove_var("PI_ORCHESTRATOR_RPC_COMMAND");
    }
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn machine_recovery_reregisters_live_pi_and_updates_live_record() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut paths = Vec::new();
        for (status, response) in [
            (
                "200 OK",
                r#"{"id":"machine-1","heartbeatIntervalMs":20,"expiresInMs":120000}"#,
            ),
            (
                "200 OK",
                r#"{"id":"pi-1","heartbeatIntervalMs":60000,"expiresInMs":120000}"#,
            ),
            ("404 Not Found", "{}"),
            ("404 Not Found", "{}"),
            ("404 Not Found", "{}"),
            (
                "200 OK",
                r#"{"id":"machine-2","heartbeatIntervalMs":60000,"expiresInMs":120000}"#,
            ),
            (
                "200 OK",
                r#"{"id":"pi-2","heartbeatIntervalMs":60000,"expiresInMs":120000}"#,
            ),
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
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                response.len()
            );
            std::io::Write::write_all(&mut stream, reply.as_bytes()).unwrap();
        }
        paths
    });
    let dir = std::env::temp_dir().join(format!(
        "zedflow-supervisor-radius-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&dir).unwrap();
    let command = dir.join("rpc.sh");
    fs::write(&command, "#!/bin/sh\nwhile IFS= read -r line; do id=$(printf '%s\\n' \"$line\" | sed -n 's/.*\"id\":\"\\([^\"]*\\)\".*/\\1/p'); printf '{\"type\":\"response\",\"id\":\"%s\",\"success\":true,\"command\":\"get_state\",\"data\":{\"sessionId\":\"session-1\"}}\\n' \"$id\"; done\n").unwrap();
    let mut permissions = fs::metadata(&command).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&command, permissions).unwrap();
    unsafe {
        std::env::set_var("PI_RADIUS_API_KEY", "test-token");
        std::env::set_var(
            "PI_RADIUS_ORCHESTRATOR_URL",
            format!("http://{address}/v1/"),
        );
        std::env::set_var("PI_ORCHESTRATOR_DIR", &dir);
        std::env::set_var("PI_ORCHESTRATOR_RPC_COMMAND", &command);
    }

    let mut supervisor = OrchestratorSupervisor::new();
    supervisor.start_radius().await.unwrap();
    let instance = supervisor
        .spawn_instance(dir.to_string_lossy().into_owned(), None)
        .await
        .unwrap();
    for _ in 0..100 {
        if supervisor
            .get_instance(&instance.id)
            .unwrap()
            .unwrap()
            .radius_pi_id
            .as_deref()
            == Some("pi-2")
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        supervisor
            .get_instance(&instance.id)
            .unwrap()
            .unwrap()
            .radius_pi_id
            .as_deref(),
        Some("pi-2")
    );
    assert_eq!(
        server.join().unwrap(),
        [
            "/v1/machines/register",
            "/v1/pis/register",
            "/v1/machines/machine-1/heartbeat",
            "/v1/machines/machine-1/heartbeat",
            "/v1/machines/machine-1/heartbeat",
            "/v1/machines/register",
            "/v1/pis/register"
        ]
    );
    unsafe {
        std::env::remove_var("PI_RADIUS_API_KEY");
        std::env::remove_var("PI_RADIUS_ORCHESTRATOR_URL");
        std::env::remove_var("PI_ORCHESTRATOR_DIR");
        std::env::remove_var("PI_ORCHESTRATOR_RPC_COMMAND");
    }
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn pi_heartbeat_404_reregistration_updates_live_record() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut paths = Vec::new();
        for (status, response) in [
            (
                "200 OK",
                r#"{"id":"machine-1","heartbeatIntervalMs":60000,"expiresInMs":120000}"#,
            ),
            (
                "200 OK",
                r#"{"id":"pi-1","heartbeatIntervalMs":20,"expiresInMs":120000}"#,
            ),
            ("404 Not Found", "{}"),
            ("404 Not Found", "{}"),
            ("404 Not Found", "{}"),
            (
                "200 OK",
                r#"{"id":"pi-2","heartbeatIntervalMs":60000,"expiresInMs":120000}"#,
            ),
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
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                response.len()
            );
            std::io::Write::write_all(&mut stream, reply.as_bytes()).unwrap();
        }
        paths
    });
    let dir = std::env::temp_dir().join(format!(
        "zedflow-supervisor-radius-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&dir).unwrap();
    let command = dir.join("rpc.sh");
    fs::write(&command, "#!/bin/sh\nwhile IFS= read -r line; do id=$(printf '%s\\n' \"$line\" | sed -n 's/.*\"id\":\"\\([^\"]*\\)\".*/\\1/p'); printf '{\"type\":\"response\",\"id\":\"%s\",\"success\":true,\"command\":\"get_state\",\"data\":{\"sessionId\":\"session-1\"}}\\n' \"$id\"; done\n").unwrap();
    let mut permissions = fs::metadata(&command).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&command, permissions).unwrap();
    unsafe {
        std::env::set_var("PI_RADIUS_API_KEY", "test-token");
        std::env::set_var(
            "PI_RADIUS_ORCHESTRATOR_URL",
            format!("http://{address}/v1/"),
        );
        std::env::set_var("PI_ORCHESTRATOR_DIR", &dir);
        std::env::set_var("PI_ORCHESTRATOR_RPC_COMMAND", &command);
    }

    let mut supervisor = OrchestratorSupervisor::new();
    supervisor.start_radius().await.unwrap();
    let instance = supervisor
        .spawn_instance(dir.to_string_lossy().into_owned(), None)
        .await
        .unwrap();
    for _ in 0..100 {
        if supervisor
            .get_instance(&instance.id)
            .unwrap()
            .unwrap()
            .radius_pi_id
            .as_deref()
            == Some("pi-2")
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        supervisor
            .get_instance(&instance.id)
            .unwrap()
            .unwrap()
            .radius_pi_id
            .as_deref(),
        Some("pi-2")
    );
    assert_eq!(
        server.join().unwrap(),
        [
            "/v1/machines/register",
            "/v1/pis/register",
            "/v1/pis/pi-1/heartbeat",
            "/v1/pis/pi-1/heartbeat",
            "/v1/pis/pi-1/heartbeat",
            "/v1/pis/register"
        ]
    );
    unsafe {
        std::env::remove_var("PI_RADIUS_API_KEY");
        std::env::remove_var("PI_RADIUS_ORCHESTRATOR_URL");
        std::env::remove_var("PI_ORCHESTRATOR_DIR");
        std::env::remove_var("PI_ORCHESTRATOR_RPC_COMMAND");
    }
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn spawn_metadata_failure_stops_and_removes_live_instance() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let dir = std::env::temp_dir().join(format!("zedflow-supervisor-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let command = dir.join("rpc.sh");
    fs::write(&command, "#!/bin/sh\nexit 0\n").unwrap();
    let mut permissions = fs::metadata(&command).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&command, permissions).unwrap();
    unsafe {
        std::env::set_var("PI_ORCHESTRATOR_DIR", &dir);
        std::env::set_var("PI_ORCHESTRATOR_RPC_COMMAND", &command);
    }

    let mut supervisor = OrchestratorSupervisor::new();
    assert!(
        supervisor
            .spawn_instance(dir.to_string_lossy().into_owned(), None)
            .await
            .is_err()
    );
    let instances = supervisor.list_instances().unwrap();
    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0].status, InstanceStatus::Stopped);
    assert!(
        supervisor
            .handle_rpc(&instances[0].id, json!({"type":"get_state"}))
            .unwrap()
            .is_none()
    );

    unsafe {
        std::env::remove_var("PI_ORCHESTRATOR_DIR");
        std::env::remove_var("PI_ORCHESTRATOR_RPC_COMMAND");
    }
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn spawn_radius_failure_stops_and_removes_live_instance() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut paths = Vec::new();
        for (status, response) in [
            (
                "200 OK",
                r#"{"id":"machine-1","heartbeatIntervalMs":60000,"expiresInMs":120000}"#,
            ),
            ("500 Internal Server Error", "{}"),
        ] {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            paths.push(first.split_whitespace().nth(1).unwrap().to_owned());
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
            }
            let reply = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                response.len()
            );
            std::io::Write::write_all(&mut stream, reply.as_bytes()).unwrap();
        }
        paths
    });
    let dir = std::env::temp_dir().join(format!("zedflow-supervisor-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let command = dir.join("rpc.sh");
    fs::write(&command, "#!/bin/sh\nwhile IFS= read -r line; do id=$(printf '%s\\n' \"$line\" | sed -n 's/.*\"id\":\"\\([^\"]*\\)\".*/\\1/p'); printf '{\"type\":\"response\",\"id\":\"%s\",\"success\":true,\"command\":\"get_state\",\"data\":{\"sessionId\":\"session-1\"}}\\n' \"$id\"; done\n").unwrap();
    let mut permissions = fs::metadata(&command).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&command, permissions).unwrap();
    unsafe {
        std::env::set_var("PI_RADIUS_API_KEY", "test-token");
        std::env::set_var(
            "PI_RADIUS_ORCHESTRATOR_URL",
            format!("http://{address}/v1/"),
        );
        std::env::set_var("PI_ORCHESTRATOR_DIR", &dir);
        std::env::set_var("PI_ORCHESTRATOR_RPC_COMMAND", &command);
    }

    let mut supervisor = OrchestratorSupervisor::new();
    supervisor.start_radius().await.unwrap();
    assert!(
        supervisor
            .spawn_instance(dir.to_string_lossy().into_owned(), None)
            .await
            .is_err()
    );
    let instances = supervisor.list_instances().unwrap();
    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0].status, InstanceStatus::Stopped);
    assert!(
        supervisor
            .handle_rpc(&instances[0].id, json!({"type":"get_state"}))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        server.join().unwrap(),
        ["/v1/machines/register", "/v1/pis/register"]
    );

    unsafe {
        std::env::remove_var("PI_RADIUS_API_KEY");
        std::env::remove_var("PI_RADIUS_ORCHESTRATOR_URL");
        std::env::remove_var("PI_ORCHESTRATOR_DIR");
        std::env::remove_var("PI_ORCHESTRATOR_RPC_COMMAND");
    }
    fs::remove_dir_all(dir).unwrap();
}
