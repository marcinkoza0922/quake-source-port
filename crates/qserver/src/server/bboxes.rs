//! Edict bounding boxes for `r_showbboxes` (ironwail's R_ShowBoundingBoxes):
//! the server-side half, a snapshot of every live edict's box and the
//! properties the client colours and filters by.
use super::*;

/// What kind of model an edict uses; picks its box colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxModel {
    None,
    Brush,
    Alias,
    Sprite,
}

/// One edict's world-space box.
#[derive(Debug, Clone)]
pub struct EdictBox {
    pub num: u32,
    pub mins: Vec3,
    pub maxs: Vec3,
    /// mins == maxs: drawn as a point cross.
    pub point: bool,
    pub model: BoxModel,
    pub health: f32,
    pub nextthink: f32,
    pub classname: String,
    pub target: String,
    pub targetname: String,
    /// Touches the viewer's fat PVS (always true without `pvs_from`).
    pub in_pvs: bool,
    /// Edicts this one refers to through entity fields (other than `.chain`).
    pub links: Vec<u32>,
}

impl Server {
    /// Live Quake trigger bounds. SV_TouchLinks overlaps AABBs even for
    /// brush triggers, so these show the actual contact volumes.
    pub fn trigger_boxes(&self) -> Vec<(Vec3, Vec3, bool)> {
        let Some(l) = self.level.as_ref().filter(|_| self.active() && self.core.maxclients() == 1)
            else { return Vec::new() };
        let (vm, f) = (&l.vm, &self.core.sv.fields);
        (1..vm.num_edicts() as u32).filter(|&e| !vm.is_free(e)).filter_map(|e| {
            let class = vm.classname(e);
            if !class.starts_with("trigger_") {
                return None;
            }
            let origin = Vec3::from(vm.ent_v(e, f.origin));
            let mins = Vec3::from(vm.ent_v(e, f.mins));
            let maxs = Vec3::from(vm.ent_v(e, f.maxs));
            (mins != maxs).then_some((origin + mins, origin + maxs, class == "trigger_secret"))
        }).collect()
    }

    /// Every live edict but the world and `skip` (the viewing player), or
    /// nothing when no single-player level is running. With `pvs_from`,
    /// each box records whether it is in the fat PVS of that point.
    pub fn edict_boxes(&self, skip: u32, pvs_from: Option<Vec3>) -> Vec<EdictBox> {
        let Some(l) = &self.level else { return Vec::new() };
        if !self.core.sv.active || self.core.maxclients() > 1 {
            return Vec::new();
        }
        let vm = &l.vm;
        let f = &self.core.sv.fields;
        let str_field = |e: u32, name: &str| {
            vm.field_ofs(name).and_then(|o| vm.ent_str(e, o).ok())
                .map(|s| String::from_utf8_lossy(s).into_owned()).unwrap_or_default()
        };
        let mut pvs = Vec::new();
        if let Some(org) = pvs_from {
            self.core.sv.vis.fat_pvs(org, &mut pvs);
        }
        let entity_fields: Vec<usize> = vm.progs.fielddefs.iter()
            .filter(|d| d.etype() == qvm::EType::Entity && d.ofs as usize != f.chain)
            .map(|d| d.ofs as usize).collect();
        (1..vm.num_edicts() as u32)
            .filter(|&e| e != skip && !vm.is_free(e))
            .map(|e| {
                let origin = Vec3::from(vm.ent_v(e, f.origin));
                let (mins, maxs) = (Vec3::from(vm.ent_v(e, f.mins)), Vec3::from(vm.ent_v(e, f.maxs)));
                let mi = vm.ent_f(e, f.modelindex) as i32;
                let model = match self.core.sv.models.get(mi.max(0) as usize).and_then(|m| m.as_ref()) {
                    _ if mi <= 0 => BoxModel::None,
                    Some(m) if m.brush => BoxModel::Brush,
                    Some(m) if m.name.ends_with(".spr") || m.name.ends_with(".spr32") => BoxModel::Sprite,
                    Some(_) => BoxModel::Alias,
                    None => BoxModel::None,
                };
                let links = entity_fields.iter().map(|&o| vm.ent_i(e, o) as u32)
                    .filter(|&t| t != 0 && t != e && t != skip && !vm.is_free(t)).collect();
                let in_pvs = pvs_from.is_none() || match self.core.sv.ext.get(e as usize) {
                    Some(x) if !x.leafs.is_empty() && x.leafs.len() < qworld::world::MAX_ENT_LEAFS =>
                        crate::vis::leafs_in_pvs(&x.leafs, &pvs),
                    _ => self.core.sv.vis.cm.box_in_pvs(origin + mins, origin + maxs, &pvs),
                };
                EdictBox {
                    num: e,
                    mins: origin + mins,
                    maxs: origin + maxs,
                    point: mins == maxs,
                    model,
                    health: vm.ent_f(e, f.health),
                    nextthink: vm.ent_f(e, f.nextthink),
                    classname: vm.classname(e),
                    target: str_field(e, "target"),
                    targetname: str_field(e, "targetname"),
                    in_pvs,
                    links,
                }
            })
            .collect()
    }
}
