//! Per-view GPU contributions; kept separate from aggregate stage totals.
use super::Result;
use std::fmt::Write as _;

#[derive(Clone, Debug, Default)]
pub(super) struct View {
    pub frames: u64,
    pub stages: Vec<(String, f64)>,
}

pub(super) fn parse(s: &str) -> Option<Vec<View>> {
    let Some(start) = s.find("\"gpu_views_us\": [") else { return Some(Vec::new()) };
    let body = &s[start..];
    let body = body.split_once('[')?.1.split_once(']')?.0;
    let mut views = Vec::new();
    for object in body.split('}').filter_map(|part| part.split_once('{').map(|p| p.1)) {
        let mut view = View::default();
        for kv in object.split(',') {
            let (name, value) = kv.split_once(':')?;
            let name = name.trim().trim_matches('"');
            let value = value.trim();
            match name {
                "frames" => view.frames = value.parse().ok()?,
                "total" => { let _: f64 = value.parse().ok()?; }
                _ => view.stages.push((name.into(), value.parse().ok()?)),
            }
        }
        views.push(view);
    }
    Some(views)
}

pub(super) fn json(views: &[View]) -> String {
    let objects: Vec<_> = views.iter().map(|view| {
        let stages: Vec<_> = view.stages.iter().map(|(name, us)| format!("\"{name}\": {us:.2}")).collect();
        let total: f64 = view.stages.iter().map(|v| v.1).sum();
        format!("{{\"frames\": {}, \"total\": {total:.2}, {}}}", view.frames, stages.join(", "))
    }).collect();
    format!("[{}]", objects.join(", "))
}

pub(super) fn write(md: &mut String, rows: &[&Result]) {
    if rows.iter().all(|row| row.run.views.is_empty()) { return; }
    let _ = writeln!(md, "\n### GPU scene views\n\nTimes are contributions in µs per profiled GPU frame, including frames where a view was absent. View stages sum to the aggregate stages; shared UI/present is excluded. Destinations isolates portal/teleporter draws, including nested views, for each scene view. These intervals include GPU stalls and rendering boundaries.\n");
    let _ = writeln!(md, "| scenario | view | active frames | scene total | destinations | AO | particles+vm | post |\n|---|--:|--:|--:|--:|--:|--:|--:|");
    for row in rows {
        for (vi, view) in row.run.views.iter().enumerate() {
            let stage = |name| view.stages.iter().find(|(k, _)| k == name).map_or(0.0, |v| v.1);
            let total: f64 = view.stages.iter().map(|v| v.1).sum();
            let ao: f64 = view.stages.iter().filter(|(k, _)| k.starts_with("ao-")).fold(0.0, |sum, v| sum + v.1);
            let _ = writeln!(md, "| {} | {} | {} | {total:.0} | {:.0} | {ao:.0} | {:.0} | {:.0} |",
                row.scenario, vi + 1, view.frames, stage("destinations"), stage("particles+vm"), stage("post"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::Run;

    #[test]
    fn parses_and_reports_views_separately_from_aggregate() {
        let old = r#"{"gpu_us": {"frames": 2, "total": 25, "world": 20, "post": 5}}"#;
        assert!(parse(old).unwrap().is_empty());
        let views = r#""gpu_views_us": [{"frames": 2, "total": 10, "world": 8, "post": 2}, {"frames": 1, "total": 15, "world": 12, "post": 3}]"#;
        let v = parse(views).unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[1].frames, 1);
        assert_eq!(v[1].stages, [("world".into(), 12.0), ("post".into(), 3.0)]);
        assert_eq!(parse(&format!("\"gpu_views_us\": {}", json(&v))).unwrap()[1].stages, v[1].stages);
        let result = Result { scenario: "coop", group: "ultimate", map: "test".into(), what: "", run: Run { views: v, ..Run::default() } };
        let mut md = String::new();
        write(&mut md, &[&result]);
        assert!(md.contains("| coop | 1 | 2 | 10 | 0 | 0 | 0 | 2 |"));
        assert!(md.contains("| coop | 2 | 1 | 15 | 0 | 0 | 0 | 3 |"));
        assert!(parse(r#""gpu_views_us": [{"frames": "broken"}]"#).is_none());
    }
}
