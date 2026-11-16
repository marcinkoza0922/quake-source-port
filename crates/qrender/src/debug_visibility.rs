//! r_visibility: session-only render filtering for both backends; collision
//! and gameplay remain untouched.
use crate::portals::Portal;
use crate::world::World;
use std::collections::HashSet;

/// Portal isolation state (Vulkan backend: see-through portals only).
pub(crate) struct Isolate<'a> {
    pub(crate) portals: &'a [Portal],
    pub(crate) portal: &'a mut Option<usize>,
    pub(crate) distance: &'a mut Option<f32>,
}

#[cfg(qvk)]
impl crate::Renderer {
    /// Debug isolation overrides the host camera during rendering.
    pub fn has_camera_override(&self) -> bool {
        self.isolated_portal.is_some()
    }

    /// Console interface for identifying and hiding level geometry.
    pub fn debug_visibility(&mut self, args: &[String]) -> String {
        let Some(w) = self.world.as_mut() else {
            return "No level loaded.\n".into();
        };
        let isolate = Isolate { portals: &w.portals, portal: &mut self.isolated_portal, distance: &mut self.isolated_distance };
        command(&mut w.cpu, &mut self.hidden_entities, Some(isolate), args)
    }
}

/// Runs `r_visibility args` on the level `w`. Without `isolate` the
/// backend has no portal views to isolate.
pub(crate) fn command(w: &mut World, hidden_entities: &mut HashSet<u32>, mut isolate: Option<Isolate>, args: &[String]) -> String {
    const HELP: &str = "r_visibility list [texture substring] | hide/show face <id> | hide/show texture <substring> | hide/show entity <render key> | isolate <portal index> [camera distance] | reset\n";
    if args.first().is_some_and(|s| s == "reset") {
        if let Some(iso) = &mut isolate {
            *iso.portal = None;
            *iso.distance = None;
        }
        w.hidden_surfaces.clear();
        hidden_entities.clear();
        return "All debug-hidden elements restored.\n".into();
    }
    if (2..=3).contains(&args.len()) && args[0] == "isolate" {
        let Some(iso) = isolate else {
            return "Portal isolation needs see-through portals, which this renderer does not draw.\n".into();
        };
        let Ok(index) = args[1].parse::<usize>() else {
            return HELP.into();
        };
        if index >= iso.portals.len() {
            return "Invalid portal index.\n".into();
        }
        *iso.distance = if let Some(distance) = args.get(2) {
            match distance.parse::<f32>() {
                Ok(d) if d.is_finite() && (4.0..=4096.0).contains(&d) => Some(d),
                _ => return "Camera distance must be finite and between 4 and 4096.\n".into(),
            }
        } else {
            None
        };
        let portal = &iso.portals[index];
        *iso.portal = Some(index);
        return format!(
            "Isolated portal {index}: source obstacles hidden; destination retained. center {:?}, normal {:?}, bounds {:?} {:?}\n",
            portal.center, portal.normal, portal.mins, portal.maxs
        );
    }
    if args.first().is_some_and(|s| s == "list") {
        let pattern = args.get(1).map_or("", String::as_str).to_ascii_lowercase();
        let mut out = String::new();
        for (i, s) in w.surfaces.iter().enumerate() {
            let name = w
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
                if w.hidden_surfaces.contains(&(i as u32)) {
                    "hidden"
                } else {
                    "visible"
                }
            ));
        }
        let mut entities: Vec<_> = hidden_entities.iter().copied().collect();
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
                hidden_entities.insert(id);
            } else {
                hidden_entities.remove(&id);
            }
            return format!("Entity {id}: {}\n", if hide { "hidden" } else { "visible" });
        }
        "face" => match args[2].parse::<u32>() {
            Ok(id) if (id as usize) < w.surfaces.len() => vec![id],
            _ => return "Invalid BSP face index.\n".into(),
        },
        "texture" => {
            let pattern = args[2].to_ascii_lowercase();
            w
                .surfaces
                .iter()
                .enumerate()
                .filter_map(|(i, s)| {
                    w
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
            w.hidden_surfaces.insert(id);
        } else {
            w.hidden_surfaces.remove(&id);
        }
    }
    format!(
        "{} faces {}.\n",
        ids.len(),
        if hide { "hidden" } else { "restored" }
    )
}
