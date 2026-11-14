//! Session-only render filtering; collision and gameplay remain untouched.
use crate::Renderer;

impl Renderer {
    /// Debug isolation overrides the host camera during rendering.
    pub fn has_camera_override(&self) -> bool {
        self.isolated_portal.is_some()
    }

    /// Console interface for identifying and hiding level geometry.
    pub fn debug_visibility(&mut self, args: &[String]) -> String {
        const HELP: &str = "r_visibility list [texture substring] | hide/show face <id> | hide/show texture <substring> | hide/show entity <render key> | isolate <portal index> [camera distance] | reset\n";
        let Some(w) = self.world.as_mut() else {
            return "No level loaded.\n".into();
        };
        if args.first().is_some_and(|s| s == "reset") {
            self.isolated_portal = None;
            self.isolated_distance = None;
            w.cpu.hidden_surfaces.clear();
            self.hidden_entities.clear();
            return "All debug-hidden elements restored.\n".into();
        }
        if (2..=3).contains(&args.len()) && args[0] == "isolate" {
            let Ok(index) = args[1].parse::<usize>() else {
                return HELP.into();
            };
            if index >= w.portals.len() {
                return "Invalid portal index.\n".into();
            }
            self.isolated_distance = if let Some(distance) = args.get(2) {
                match distance.parse::<f32>() {
                    Ok(d) if d.is_finite() && (4.0..=4096.0).contains(&d) => Some(d),
                    _ => return "Camera distance must be finite and between 4 and 4096.\n".into(),
                }
            } else {
                None
            };
            let portal = &w.portals[index];
            self.isolated_portal = Some(index);
            return format!(
                "Isolated portal {index}: source obstacles hidden; destination retained. center {:?}, normal {:?}, bounds {:?} {:?}\n",
                portal.center, portal.normal, portal.mins, portal.maxs
            );
        }
        if args.first().is_some_and(|s| s == "list") {
            let pattern = args.get(1).map_or("", String::as_str).to_ascii_lowercase();
            let mut out = String::new();
            for (i, s) in w.cpu.surfaces.iter().enumerate() {
                let name = w
                    .cpu
                    .texture_names
                    .get(s.texture as usize)
                    .map_or("", String::as_str);
                if !name.to_ascii_lowercase().contains(&pattern) {
                    continue;
                }
                out.push_str(&format!(
                    "face {i} texture {name} bounds {:?} {:?} {}\n",
                    s.mins,
                    s.maxs,
                    if w.cpu.hidden_surfaces.contains(&(i as u32)) {
                        "hidden"
                    } else {
                        "visible"
                    }
                ));
            }
            let mut entities: Vec<_> = self.hidden_entities.iter().copied().collect();
            entities.sort_unstable();
            out.push_str(&format!("Hidden entity keys: {entities:?}\n"));
            return out;
        }
        if args.len() != 3 || !matches!(args[0].as_str(), "hide" | "show") {
            return HELP.into();
        }
        let hide = args[0] == "hide";
        let ids: Vec<u32> = match args[1].as_str() {
            "entity" => {
                let Ok(id) = args[2].parse::<u32>() else {
                    return HELP.into();
                };
                if id == 0 {
                    return "Entity key 0 is shared; choose a nonzero render key.\n".into();
                }
                if hide {
                    self.hidden_entities.insert(id);
                } else {
                    self.hidden_entities.remove(&id);
                }
                return format!("Entity {id}: {}\n", if hide { "hidden" } else { "visible" });
            }
            "face" => match args[2].parse::<u32>() {
                Ok(id) if (id as usize) < w.cpu.surfaces.len() => vec![id],
                _ => return "Invalid BSP face index.\n".into(),
            },
            "texture" => {
                let pattern = args[2].to_ascii_lowercase();
                w.cpu
                    .surfaces
                    .iter()
                    .enumerate()
                    .filter_map(|(i, s)| {
                        w.cpu
                            .texture_names
                            .get(s.texture as usize)
                            .is_some_and(|n| n.to_ascii_lowercase().contains(&pattern))
                            .then_some(i as u32)
                    })
                    .collect()
            }
            _ => return HELP.into(),
        };
        for &id in &ids {
            if hide {
                w.cpu.hidden_surfaces.insert(id);
            } else {
                w.cpu.hidden_surfaces.remove(&id);
            }
        }
        format!(
            "{} faces {}.\n",
            ids.len(),
            if hide { "hidden" } else { "restored" }
        )
    }
}
