use std::env;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::process;

fn main() {
    let port = env::var("VINROUGE_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .expect("VINROUGE_PORT environment variable not set");

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: vu list | schema | current | validate '<script>' | update <id> '<script>' | create <label> '<script>' [control_id] [control_ref] | run <id>");
        process::exit(1);
    }

    let action = &args[1];
    let payload = match action.as_str() {
        "list" => serde_json::json!({"action": "list"}),
        "schema" => serde_json::json!({"action": "schema"}),
        "current" => serde_json::json!({"action": "current"}),
        "run" => {
            if args.len() < 3 {
                eprintln!("Usage: vu run <id>");
                process::exit(1);
            }
            serde_json::json!({"action": "run", "id": args[2]})
        }
        "validate" => {
            if args.len() < 3 {
                eprintln!("Usage: vu validate '<script>'");
                process::exit(1);
            }
            serde_json::json!({"action": "validate", "script_text": args[2]})
        }
        "update" => {
            if args.len() < 4 {
                eprintln!("Usage: vu update <id> '<script>'");
                process::exit(1);
            }
            serde_json::json!({"action": "update", "id": args[2], "script_text": args[3]})
        }
        "create" => {
            if args.len() < 4 {
                eprintln!("Usage: vu create <label> '<script>' [control_id] [control_ref]");
                process::exit(1);
            }
            serde_json::json!({
                "action": "create",
                "label": args[2],
                "script_text": args[3],
                "control_id": args.get(4).cloned().unwrap_or_default(),
                "control_ref": args.get(5).cloned().unwrap_or_default(),
            })
        }
        _ => {
            eprintln!("Unknown action: {}", action);
            eprintln!("Usage: vu list | schema | current | validate '<script>' | update <id> '<script>' | create <label> '<script>' [control_id] [control_ref] | run <id>");
            process::exit(1);
        }
    };

    let msg = format!("{}\n", serde_json::to_string(&payload).unwrap());

    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap_or_else(|e| {
        eprintln!(
            "vu: cannot connect to VinRouge IPC server (port {}). Is the terminal open? ({})",
            port, e
        );
        process::exit(1);
    });

    stream.write_all(msg.as_bytes()).unwrap();
    stream.flush().unwrap();

    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if buf.contains(&b'\n') {
                    break;
                }
            }
            Err(e) => {
                eprintln!("vu: read error: {}", e);
                process::exit(1);
            }
        }
    }

    let response = String::from_utf8(buf).unwrap_or_else(|_| {
        eprintln!("vu: invalid UTF-8 response");
        process::exit(1);
    });

    print!("{}", response.trim_end());
}
