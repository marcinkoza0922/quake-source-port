use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

#[test]
fn protocol_and_artifact_boundaries() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_qmcp"))
        .args(["--root", root.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut out = BufReader::new(child.stdout.take().unwrap());
    let mut request = |v: Value| -> Value {
        writeln!(input, "{v}").unwrap();
        input.flush().unwrap();
        let mut line = String::new();
        out.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    };
    let r = request(
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}),
    );
    assert_eq!(r["result"]["serverInfo"]["name"], "quake-tools");
    let r = request(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}));
    assert_eq!(r["result"]["tools"].as_array().unwrap().len(), 9);
    for (name, args) in [
        ("read_report", json!({"path":"Cargo.toml"})),
        ("read_report", json!({"path":"/etc/passwd"})),
        ("inspect_map", json!({"map":"../bad"})),
        ("list_artifacts", json!({"limit":201})),
        ("compare_qbench", json!({"before":4,"after":"a"})),
    ] {
        let r = request(
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":name,"arguments":args}}),
        );
        assert_eq!(r["result"]["isError"], true, "{r}");
    }
    // A JSON-looking symlink must not expose arbitrary checkout or home files.
    #[cfg(unix)]
    {
        let link = root.join(format!("target/qmcp-escape-{}.json", std::process::id()));
        std::os::unix::fs::symlink(root.join("Cargo.toml"), &link).unwrap();
        let r = request(
            json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"read_report","arguments":{"path":link.to_str().unwrap()}}}),
        );
        std::fs::remove_file(link).unwrap();
        assert_eq!(r["result"]["isError"], true);
    }
    let r = request(
        json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"capture_screenshot","arguments":{"map":"e1m1","camera":[1,2]}}}),
    );
    assert_eq!(r["result"]["isError"], true);
    writeln!(input, "{{not json").unwrap();
    input.flush().unwrap();
    let mut line = String::new();
    out.read_line(&mut line).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&line).unwrap()["error"]["code"],
        -32700
    );
    drop(input);
    assert!(child.wait().unwrap().success());
}
