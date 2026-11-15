use crate::Result;
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn events(v: &Value) -> f64 {
    v["events"].as_f64().unwrap_or_else(|| {
        ["triggers", "buttons", "pickups"]
            .iter()
            .filter_map(|k| v[k].as_f64())
            .sum()
    })
}

fn logs(v: &Value) -> Vec<String> {
    let baseline = v["log"].as_array();
    baseline
        .or_else(|| v["unusual"].as_array())
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|s| {
            if baseline.is_none() {
                s.split_once(": ").map_or(s, |(_, text)| text)
            } else {
                s
            }
        })
        .map(str::to_owned)
        .collect()
}

pub fn qbot(before: &Value, after: &Value) -> Result<Value> {
    let old = before
        .as_object()
        .ok_or("before must be a qbot map object")?;
    let new = after.as_object().ok_or("after must be a qbot map object")?;
    let mut changes = Vec::new();
    let mut missing = Vec::new();
    for (map, b) in old {
        let Some(a) = new.get(map) else {
            missing.push(map);
            continue;
        };
        let mut reasons = Vec::new();
        let mut deltas = serde_json::Map::new();
        for key in ["kills", "secrets", "events"] {
            let (bv, av) = if key == "events" {
                (events(b), events(a))
            } else {
                (
                    b[key]
                        .as_f64()
                        .ok_or_else(|| format!("{map}: missing numeric {key} before"))?,
                    a[key]
                        .as_f64()
                        .ok_or_else(|| format!("{map}: missing numeric {key} after"))?,
                )
            };
            deltas.insert(key.into(), json!(av - bv));
            if av < bv {
                reasons.push(format!("lost {key}"));
            }
        }
        if b["exited"] == true && a["exited"] != true {
            reasons.push("lost exit".into());
        }
        let mut new_items = serde_json::Map::new();
        for key in ["failed", "errors"] {
            let added: Vec<_> = a[key]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|v| !b[key].as_array().is_some_and(|items| items.contains(v)))
                .cloned()
                .collect();
            if !added.is_empty() {
                reasons.push(format!("new {key}"));
                new_items.insert(key.into(), json!(added));
            }
        }
        let known = logs(b);
        let added: Vec<_> = logs(a)
            .into_iter()
            .filter(|line| !known.contains(line))
            .collect();
        if !added.is_empty() {
            reasons.push("new developer log lines".into());
            new_items.insert("log".into(), json!(added));
        }
        changes.push(json!({"map":map,"deltas":deltas,"regression":!reasons.is_empty(),"reasons":reasons,"new_items":new_items}));
    }
    let regressions = changes.iter().filter(|v| v["regression"] == true).count();
    Ok(
        json!({"regressions":regressions,"missing_maps":missing,"added_maps":new.keys().filter(|k|!old.contains_key(*k)).collect::<Vec<_>>(),"maps":changes,"comparable":missing.is_empty(),"has_regressions":regressions>0 || !missing.is_empty()}),
    )
}

fn bench_rows(v: &Value) -> Result<BTreeMap<String, &Value>> {
    let rows = v.as_array().ok_or("expected a qbench report array")?;
    let mut out = BTreeMap::new();
    for r in rows {
        for k in ["game", "map", "engine"] {
            if !r[k].is_string() {
                return Err(format!("missing {k}"));
            }
        }
        for k in ["width", "height"] {
            if r[k].as_u64().is_none() {
                return Err(format!("missing {k}"));
            }
        }
        let key = json!([r["game"], r["map"], r["engine"], r["width"], r["height"]]).to_string();
        if out.insert(key, r).is_some() {
            return Err("duplicate qbench case".into());
        }
    }
    Ok(out)
}

pub fn qbench(before: &Value, after: &Value, threshold: f64) -> Result<Value> {
    let old = bench_rows(before)?;
    let new = bench_rows(after)?;
    let mut rows = Vec::new();
    let mut missing = Vec::new();
    for (key, b) in &old {
        let Some(a) = new.get(key) else {
            missing.push(key);
            continue;
        };
        let mut row = json!({"case":serde_json::from_str::<Value>(key).unwrap(),"before_fps":b["fps_median"],"after_fps":a["fps_median"],"before_runs":b["runs"],"after_runs":a["runs"]});
        match (b["fps_median"].as_f64(), a["fps_median"].as_f64()) {
            (Some(bf), Some(af)) if bf > 0.0 && af > 0.0 => {
                let change = (bf / af - 1.0) * 100.0;
                row["frame_time_change_percent"] = json!(change);
                row["fps_change_percent"] = json!((af / bf - 1.0) * 100.0);
                row["regression"] = json!(change > threshold);
            }
            _ => {
                row["error"] = json!("missing/failed FPS measurement");
                row["regression"] = json!(true);
            }
        }
        rows.push(row);
    }
    Ok(
        json!({"regressions":rows.iter().filter(|r|r["regression"] == true).count(),"threshold_percent":threshold,"missing_cases":missing,"added_cases":new.keys().filter(|k|!old.contains_key(*k)).collect::<Vec<_>>(),"cases":rows,"comparable":missing.is_empty(),"has_regressions":!missing.is_empty() || rows.iter().any(|r|r["regression"]==true),"settings_note":"Only game/map/engine/resolution are recorded by qbench. Verify GPU, engine settings and workload match before attributing a change."}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn baseline_events_and_missing_maps() {
        let b = json!({"e1m1":{"kills":10,"secrets":2,"events":6,"exited":true},"e1m2":{}});
        let a = json!({"e1m1":{"kills":9,"secrets":2,"triggers":2,"buttons":1,"pickups":3,"exited":false,"errors":["oops"]}});
        let r = qbot(&b, &a).unwrap();
        assert_eq!(r["regressions"], 1);
        assert_eq!(r["maps"][0]["deltas"]["events"], 0.0);
        assert_eq!(r["missing_maps"], json!(["e1m2"]));
        assert_eq!(r["maps"][0]["reasons"].as_array().unwrap().len(), 3);
    }
    #[test]
    fn developer_logs_match_baselines_without_timestamps() {
        let b = json!({"m":{"kills":1,"secrets":0,"events":0,"log":["monster in wall"]}});
        let mut a = json!({"m":{"kills":1,"secrets":0,"events":0,"unusual":["t=12.3 (x4): monster in wall"]}});
        assert_eq!(qbot(&b, &a).unwrap()["regressions"], 0);
        a["m"]["unusual"] = json!(["t=2.0: new warning"]);
        assert_eq!(qbot(&b, &a).unwrap()["regressions"], 1);
    }
    #[test]
    fn bench_matches_resolution_and_handles_failed_runs() {
        let b = json!([{"game":"id1","map":"e1m1","engine":"ours","width":1280,"height":720,"fps_median":100.0}]);
        let mut a = b.clone();
        a[0]["fps_median"] = json!(80.0);
        let r = qbench(&b, &a, 5.0).unwrap();
        assert_eq!(r["cases"][0]["frame_time_change_percent"], 25.0);
        assert_eq!(r["regressions"], 1);
        a[0]["width"] = json!(640);
        assert_eq!(
            qbench(&b, &a, 5.0).unwrap()["missing_cases"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        a = b.clone();
        a[0]["fps_median"] = Value::Null;
        assert_eq!(qbench(&b, &a, 5.0).unwrap()["regressions"], 1);
        assert!(qbench(&json!([b[0], b[0]]), &a, 5.0).is_err());
    }
}
