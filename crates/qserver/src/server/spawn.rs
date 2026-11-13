use super::*;

impl Server {
    /// SV_SpawnServer. Errors (missing map/progs, QC errors while spawning) are
    /// also recorded as the level error.
    pub fn spawn_server(&mut self, mapname: &str) -> Result<(), String> {
        let r = self.spawn_inner(mapname);
        if let Err(e) = &r {
            self.core.print(&format!("Couldn't spawn server {mapname}: {e}\n"));
            self.core.sv.active = false;
            if self.core.sv.error.is_none() {
                self.core.sv.error = Some(VmError::new(e.clone()));
            }
        }
        r
    }

    pub(super) fn spawn_inner(&mut self, mapname: &str) -> Result<(), String> {
        let c = &mut self.core;
        c.changelevel_issued = false;
        if c.cvars.value_of("coop") != 0.0 {
            c.cvars.set("deathmatch", "0");
        }
        let skill = ((c.cvars.value_of("skill") + 0.5) as i32).clamp(0, 3);
        c.cvars.set_value("skill", skill as f32);

        self.level = None;
        let maxclients = c.maxclients();
        let protocol = match c.cvars.value_of("sv_protocol") as u32 {
            PROTOCOL_NETQUAKE => PROTOCOL_NETQUAKE,
            PROTOCOL_RMQ => PROTOCOL_RMQ,
            _ => PROTOCOL_FITZQUAKE,
        };
        let progs = c.fs.open("progs.dat").ok_or("couldn't load progs.dat")?;
        let max_edicts = (c.cvars.value_of("max_edicts") as usize).clamp(256, 32000);
        let mut vm = Vm::load(&progs, max_edicts, maxclients).map_err(|e| e.to_string())?;
        let fields = Fields::new(&vm).map_err(|e| e.to_string())?;
        let globals = Globals::new(&vm).map_err(|e| e.to_string())?;

        let modelname = format!("maps/{mapname}.bsp");
        let data = c.fs.open(&modelname).ok_or_else(|| format!("{modelname} not found"))?;
        let bsp = qformats::Bsp::parse(&data).map_err(|e| format!("{modelname}: {e:?}"))?;
        let cm = Arc::new(CollisionModel::from_bsp(&bsp));
        let mut world = World::new(cm.clone());

        let has_global = |vm: &Vm, n: &str, v: f32| vm.global_ofs(n).is_some_and(|o| vm.g_f(o) == v);
        let isqex = has_global(&vm, "EF_QUADLIGHT", 16.0)
            && (has_global(&vm, "EF_PENTLIGHT", 32.0) || has_global(&vm, "EF_PENTALIGHT", 32.0));
        let parse_client_command = vm.progs.find_function("SV_ParseClientCommand");

        c.sv = Sv {
            name: mapname.to_string(),
            modelname: modelname.clone(),
            loading: true,
            protocol,
            protocolflags: if protocol == PROTOCOL_RMQ { PRFL_INT32COORD | PRFL_SHORTANGLE } else { 0 },
            model_precache: vec![String::new(), modelname.clone()],
            models: vec![None, None],
            sound_precache: vec![String::new()],
            particle_precache: vec![String::new()],
            lightstyles: vec![Vec::new(); MAX_LIGHTSTYLES],
            datagram: SizeBuf::new(MAX_DATAGRAM),
            reliable_datagram: SizeBuf::new(MAX_DATAGRAM),
            signon: vec![SizeBuf::new(SIGNON_SIZE)],
            fields,
            globals,
            ext: vec![EdictExt::default(); vm.max_edicts()],
            vis: Vis::new(cm.clone(), &bsp),
            effects_mask: if isqex { -1 } else { !(16 | 32 | 64) },
            parse_client_command,
            ..Sv::default()
        };
        let sub0 = cm.submodels.first();
        c.sv.models[1] = Some(ModelInfo {
            name: modelname.clone(),
            brush: true,
            mins: sub0.map_or(Vec3::ZERO, |m| m.mins),
            maxs: sub0.map_or(Vec3::ZERO, |m| m.maxs),
        });
        for (i, m) in cm.submodels.iter().enumerate().skip(1) {
            let name = format!("*{i}");
            c.sv.model_precache.push(name.clone());
            c.sv.models.push(Some(ModelInfo { name, brush: true, mins: m.mins, maxs: m.maxs }));
            world.set_model(1 + i, Some((cm.clone(), i)));
        }
        for cl in &mut c.clients {
            cl.spawned = false;
        }

        vm.time = 1.0;
        vm.seed_random(0x1234_5678);
        // world edict
        vm.edict_mut(0).fill(0);
        let f = &c.sv.fields;
        let h = vm.strings.engine(modelname.as_bytes());
        vm.set_ent_i(0, f.model, h);
        vm.set_ent_f(0, f.modelindex, 1.0);
        vm.set_ent_f(0, f.solid, SOLID_BSP as f32);
        vm.set_ent_f(0, f.movetype, MOVETYPE_PUSH as f32);
        let g = &c.sv.globals;
        let coop = c.cvars.value_of("coop");
        if coop != 0.0 {
            vm.set_g_f(g.coop, coop);
        } else {
            vm.set_g_f(g.deathmatch, c.cvars.value_of("deathmatch"));
        }
        let h = vm.strings.engine(mapname.as_bytes());
        vm.set_g_i(g.mapname, h);
        vm.set_g_f(g.serverflags, c.serverflags);

        let mut ents = bsp.entities.clone();
        if c.cvars.value_of("external_ents") != 0.0
            && let Some(e) = c.fs.open_companion(&format!("maps/{mapname}.bsp"), &format!("maps/{mapname}.ent"))
        {
            ents = qcore::text::decode(&e);
        }
        let filter = SpawnFilter {
            deathmatch: c.cvars.value_of("deathmatch") != 0.0,
            skill,
            nomonsters: c.cvars.value_of("nomonsters") != 0.0,
        };
        let r = {
            let mut h = Ctx::new(&mut *c, &mut world);
            vm.load_from_file(&mut h, &ents, filter)
        };
        if let Err(e) = r {
            c.host_error(e);
        }
        if let Some(e) = &c.sv.error {
            return Err(e.to_string());
        }
        c.sv.active = true;
        c.sv.loading = false;
        vm.protect_world = true;
        self.level = Some(Level { vm, world, assist: Vec::new(), portals: Default::default(), stickies: Default::default(), falls: Default::default() });

        // run two frames to allow everything to settle
        self.physics(0.1);
        self.physics(0.1);
        if let Some(e) = &self.core.sv.error {
            return Err(e.to_string());
        }
        self.create_baseline();
        for i in 0..self.core.maxclients() {
            if self.core.clients[i].active {
                self.send_serverinfo(i);
            }
        }
        for i in 0..self.core.maxclients() {
            if self.core.clients[i].active && self.core.clients[i].local {
                self.local_signon(i);
            }
        }
        Ok(())
    }
}
