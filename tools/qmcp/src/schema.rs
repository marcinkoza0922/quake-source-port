use crate::Result;
use serde_json::{Value, json};

pub fn string<'a>(a: &'a Value, key: &str) -> Result<&'a str> {
    a[key]
        .as_str()
        .ok_or_else(|| format!("{key} must be a string"))
}

pub fn tools() -> Vec<Value> {
    let text = json!({"type":"string"});
    let pair = json!({"before":text,"after":text});
    let definitions = [
        (
            "list_artifacts",
            "Find JSON reports, baselines and PNG screenshots under target/ and tools/qbot/. Paginated, newest first.",
            json!({"kind":{"type":"string","enum":["all","reports","screenshots"]},"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":200}}),
            vec![],
        ),
        (
            "read_report",
            "Read a JSON report or qbot baseline. Optional map selects a single qbot map.",
            json!({"path":text,"map":text}),
            vec!["path"],
        ),
        (
            "compare_qbot",
            "Compare qbot reports/baselines: lost kills/secrets/events/exits, new failures/errors/logs and missing maps. No baseline writes.",
            pair,
            vec!["before", "after"],
        ),
        (
            "compare_qbench",
            "Compare qbench FPS by scene/profile/engine/resolution (Part 1 reports; old flat reports by game/map/engine/resolution). Positive frame-time change means slower; also p99 frame-time change and which machine fields differ. Reports missing/added cases and failed runs; threshold defaults to 5 percent.",
            json!({"before":text,"after":text,"threshold_percent":{"type":"number","minimum":0}}),
            vec!["before", "after"],
        ),
        (
            "list_mods",
            "List installed Quake game directories through qdata.",
            json!({}),
            vec![],
        ),
        (
            "list_maps",
            "List maps and their source archive. game accepts qdata mount lists, e.g. +hipnotic,mymod.",
            json!({"game":text,"include_base":{"type":"boolean"}}),
            vec![],
        ),
        (
            "inspect_map",
            "Return indexed entity key/value pairs, preserving duplicate keys. Filter by classname substring and qdata key filters (key=value or key==exact). Paginated.",
            json!({"map":text,"game":text,"classname":text,"keys":{"type":"array","items":text},"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":200}}),
            vec!["map"],
        ),
        (
            "read_screenshot",
            "Return an existing PNG as MCP image content (maximum 8 MiB). Discover paths using list_artifacts.",
            json!({"path":text}),
            vec!["path"],
        ),
        (
            "capture_screenshot",
            "Run the existing release engine headlessly and return a fresh PNG. Saves under target/qmcp; autosaves disabled. Uses current graphics config. Optional camera is x y z pitch yaw roll. 45 second wall timeout.",
            json!({"map":text,"game":text,"camera":{"type":"array","items":{"type":"number"},"minItems":6,"maxItems":6},"width":{"type":"integer","minimum":64,"maximum":1920},"height":{"type":"integer","minimum":64,"maximum":1080}}),
            vec!["map"],
        ),
    ];
    definitions.into_iter().map(|(name, description, properties, required)| json!({"name":name,"description":description,"inputSchema":{"type":"object","properties":properties,"required":required,"additionalProperties":false},"annotations":{"readOnlyHint":name != "capture_screenshot","destructiveHint":false,"idempotentHint":name != "capture_screenshot","openWorldHint":false}})).collect()
}

pub fn validate(name: &str, a: &Value) -> Result<()> {
    let t = tools()
        .into_iter()
        .find(|t| t["name"] == name)
        .ok_or("unknown tool")?;
    let obj = a.as_object().ok_or("arguments must be an object")?;
    let s = &t["inputSchema"];
    for key in s["required"].as_array().unwrap() {
        if !obj.contains_key(key.as_str().unwrap()) {
            return Err(format!("missing {key}"));
        }
    }
    for (key, v) in obj {
        let p = s["properties"]
            .get(key)
            .ok_or_else(|| format!("unknown argument {key}"))?;
        let valid = match p["type"].as_str().unwrap() {
            "string" => v.is_string(),
            "boolean" => v.is_boolean(),
            "number" => v.is_number(),
            "integer" => v.as_u64().is_some(),
            "array" => v.as_array().is_some_and(|a| {
                a.len() <= 32
                    && p["minItems"].as_u64().is_none_or(|n| a.len() >= n as usize)
                    && p["maxItems"].as_u64().is_none_or(|n| a.len() <= n as usize)
                    && a.iter().all(|v| {
                        if p["items"]["type"] == "number" {
                            v.is_number()
                        } else {
                            v.is_string()
                        }
                    })
            }),
            _ => false,
        };
        if !valid {
            return Err(format!("invalid type for {key}"));
        }
        if let Some(n) = v.as_f64()
            && (p["minimum"].as_f64().is_some_and(|min| n < min)
                || p["maximum"].as_f64().is_some_and(|max| n > max))
        {
            return Err(format!("{key} out of range"));
        }
        if p["enum"]
            .as_array()
            .is_some_and(|options| !options.contains(v))
        {
            return Err(format!("invalid {key}"));
        }
    }
    Ok(())
}
