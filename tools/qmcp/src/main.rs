//! Checkout-scoped MCP tools over newline-delimited JSON-RPC on stdio.
mod artifacts;
mod capture;
mod compare;
mod data;
mod schema;

use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

type Result<T> = std::result::Result<T, String>;

struct Server {
    root: PathBuf,
}

impl Server {
    fn call(&self, name: &str, args: &Value) -> Result<Value> {
        schema::validate(name, args)?;
        match name {
            "list_artifacts" => self.list(args),
            "capture_screenshot" => self.capture(args),
            "read_report" => self.report(args).map(|v| json!({"report": v})),
            "compare_qbot" => compare::qbot(
                &self.read_json(schema::string(args, "before")?)?,
                &self.read_json(schema::string(args, "after")?)?,
            ),
            "compare_qbench" => compare::qbench(
                &self.read_json(schema::string(args, "before")?)?,
                &self.read_json(schema::string(args, "after")?)?,
                args["threshold_percent"].as_f64().unwrap_or(5.0),
            ),
            "inspect_map" => self.map(args),
            "list_maps" | "list_mods" => self.data_list(name, args),
            "read_screenshot" => self.screenshot(schema::string(args, "path")?),
            _ => Err(format!("unknown tool: {name}")),
        }
    }

    fn dispatch(&self, req: &Value) -> Value {
        match req["method"].as_str().unwrap_or("") {
            "initialize" => {
                let requested = req["params"]["protocolVersion"].as_str().unwrap_or("");
                let version = if ["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"]
                    .contains(&requested)
                {
                    requested
                } else {
                    "2025-06-18"
                };
                json!({"result":{"protocolVersion":version,"capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"quake-tools","version":env!("CARGO_PKG_VERSION")},"instructions":"Inspect existing Quake artifacts and game data. Paths are relative to the configured checkout; list_artifacts discovers reports and PNGs. Comparisons do not run benchmarks or modify baselines."}})
            }
            "ping" => json!({"result":{}}),
            "tools/list" => json!({"result":{"tools":schema::tools()}}),
            "tools/call" => {
                let p = &req["params"];
                let empty = json!({});
                let a = p.get("arguments").unwrap_or(&empty);
                match self.call(p["name"].as_str().unwrap_or(""), a) {
                    Ok(v) if v.get("content").is_some() => json!({"result":v}),
                    Ok(v) => {
                        json!({"result":{"content":[{"type":"text","text":v.to_string()}],"structuredContent":v,"isError":false}})
                    }
                    Err(e) => {
                        json!({"result":{"content":[{"type":"text","text":e}],"isError":true}})
                    }
                }
            }
            _ => json!({"error":{"code":-32601,"message":"Method not found"}}),
        }
    }
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let root = match (args.next().as_deref(), args.next(), args.next()) {
        (Some("--root"), Some(p), None) => PathBuf::from(p),
        _ => {
            eprintln!("usage: qmcp --root CHECKOUT");
            std::process::exit(2);
        }
    }
    .canonicalize()?;
    if !root.join("tools/qdata.py").is_file() {
        return Err("root must be a quake-source-port checkout".into());
    }
    let server = Server { root };
    let mut out = io::stdout().lock();
    // Bound individual requests; all tools have bounded outputs too.
    let mut input = io::stdin().lock();
    loop {
        let mut bytes = Vec::new();
        let count = std::io::Read::take(&mut input, 1_048_577).read_until(b'\n', &mut bytes)?;
        if count == 0 {
            break;
        }
        if count > 1_048_576 {
            return Err("request exceeds 1 MiB".into());
        }
        let response = match serde_json::from_slice::<Value>(&bytes) {
            Ok(req) if req["jsonrpc"] != "2.0" || !req.is_object() => {
                json!({"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"Invalid Request"}})
            }
            Ok(req) => {
                if req.get("id").is_none() {
                    continue;
                }
                let mut v = server.dispatch(&req);
                v["jsonrpc"] = json!("2.0");
                v["id"] = req["id"].clone();
                v
            }
            Err(_) => {
                json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}})
            }
        };
        serde_json::to_writer(&mut out, &response)?;
        writeln!(out)?;
        out.flush()?;
    }
    Ok(())
}
