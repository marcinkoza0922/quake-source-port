use super::*;

impl Host {
    /// Water reflections are on (planar or ray traced): the player's own
    /// body is then drawn into them like into gun portal views.
    pub(super) fn reflections_on(&self) -> bool {
        self.cv("r_enhancedwater") != 0.0 && self.cv("r_water_reflections").round() >= 1.0
    }

    pub(super) fn render(&mut self, view: Option<qrender::View>, gun: Option<qrender::RenderEntity>, ft: f32) {
        self.sync_shader();
        let (w, h) = self.r.size();
        let mut c = Canvas::new(w, h);
        self.con.notify_time = self.cv("con_notifytime").max(0.0) as f64;
        self.con.modern = self.cv("con_modern") != 0.0;
        self.con.alpha = self.cv("scr_conalpha");
        self.sync_shell_env();
        self.sync_ui_text();
        c.ui = Some(self.ui_text.clone());
        let mut ents = Vec::new();
        let mut dyn_verts = Vec::new();
        let mut parts = Vec::new();
        let mut dls = Vec::new();
        let mut fte = (Vec::new(), Vec::new(), Vec::new());
        let mut blend = [0.0; 4];
        let mut splits = Vec::new();
        let mut goggles = qrender::Goggles::default();
        let mut view = view.map(|mut v| {
            v.waterwarp = self.waterwarp(v.origin, self.cl.time);
            v
        });
        let mut csqc_sbar = true;
        let mut csqc_crosshair = true;
        if let Some(v) = &view {
            let fs = &self.server.core.fs;
            let cv = self.view_cvars();
            self.view.replaced_models = self.fte.replaced_models(&self.cl, fs);
            self.view.with_self = !self.cl.portals.is_empty() || self.reflections_on();
            self.view.dodge_pose = self.server.level.as_ref()
                .and_then(|l| l.dodges.get(self.cl.viewentity.saturating_sub(1)).map(|d| d.pose(l.vm.time)))
                .unwrap_or_default();
            self.view.third_person = self.camera().third_person();
            self.view.vwep.enabled = self.cv("cl_vwep") != 0.0;
            self.view.vwep.server = if self.view.vwep.enabled { self.server.weapon_models() } else { None };
            self.view.entities(&mut self.cl, &cv, &mut self.level, fs, &mut self.r, &mut ents);
            // player 1's own body belongs to player 1's portal views only
            for e in ents.iter_mut().filter(|e| e.portal_only) {
                e.views = 1;
            }
            let debris_on = self.cv("r_debris") != 0.0;
            let mut brushes = None;
            let teleporters = self.impacts.prepare(
                &self.server.core.cvars, &mut self.cl, &self.level, &mut brushes, v.origin.into(), &self.debris,
            );
            let gravity = self.cv("sv_gravity");
            let set = crate::debris::Settings {
                quality: crate::debris::Quality::from_cvar(self.cv("r_debris_quality")),
                ragdolls: false, // feature disabled (hidden from menus)
                start: crate::debris::Start::from_cvar(self.cv("r_debris_ragdollstart")),
                max_ragdolls: self.cv("r_debris_maxragdolls").clamp(1.0, 128.0) as usize,
                gibs: self.cv("r_debris_gibs") != 0.0,
                shrapnel: self.cv("r_debris_shrapnel") != 0.0,
                movers: self.cv("r_debris_movers") != 0.0,
                force: self.cv("r_debris_force").clamp(0.0, 6.0),
                kill_lift: false,
            };
            self.debris.update(
                debris_on,
                crate::debris::Frame {
                    cl: &mut self.cl,
                    lv: &mut self.level,
                    fs: &self.server.core.fs,
                    r: &mut self.r,
                    view: v.origin.into(),
                    gravity: if gravity > 0.0 { gravity } else { 800.0 },
                    set,
                },
            );
            if teleporters || crate::blood::Settings::read(&self.server.core.cvars).enabled
                || (crate::impacts::impacts_on(&self.server.core.cvars)
                    && (self.cv("r_impact_nails") != 0.0 || self.cv("r_impact_bullets") != 0.0
                        || self.cv("r_impact_scorches") != 0.0 || self.cv("r_teleportwindows") != 0.0))
            {
                self.debris.track_shooters(&self.cl, &self.level, self.cl.time);
            }
            let fte_frustum = self.cull_fte_for_view().then(|| {
                qrender::ViewFrustum::from_view(v, (w, h), self.cv("r_renderscale"))
            });
            self.debris.entities(&mut ents, &mut dyn_verts, fte_frustum.as_ref());
            if let Some(ghost) = self.ghost_entity(v, cv.lerp_models) {
                ents.push(ghost);
            }
            if self.cv("r_drawentities") == 0.0 {
                ents.clear();
            }
            let cable_held = self.toy_view_models(v, &cv, &mut ents);
            crate::addons::sticky_draw::bombs(&self.cl, &mut self.level, &self.server.core.fs, &mut self.r, &mut ents, &mut dls);
            self.boomerang_fx.entities(self.cl.time, &mut self.level, &self.server.core.fs, &mut self.r, self.snd.as_ref(), &mut ents);
            let cable_grav = if self.cv("sv_gravity") > 0.0 { self.cv("sv_gravity") } else { 800.0 };
            self.cable_fx.entities(&self.cl, self.cl.viewentity.saturating_sub(1), cable_held, self.cl.time, cable_grav, &mut self.level, &self.server.core.fs, &mut self.r, &mut ents);
            if let Some(g) = gun {
                ents.push(g);
            }
            goggles = self.goggles_render(&mut ents);
            if self.cv("r_particles") != 0.0 {
                crate::view::particles(&self.cl, &mut parts);
            }
            // Portal and split-screen views reuse geometry from other camera positions.
            let cull_particles = self.cull_fte_for_view().then(|| {
                qrender::view_fov_tangents(v, (w, h), self.cv("r_renderscale"))
            });
            fte = self.fte.frame(
                &mut self.cl,
                &self.level,
                &mut brushes,
                &self.server.core.fs,
                &mut self.r,
                self.snd.as_mut(),
                v,
                cull_particles,
            );
            self.portal_fx.frame(self.cl.time, v, &mut self.r, self.snd.as_ref(), &mut fte);
            self.rail_fx.frame(self.cl.time, v, &mut self.r, &mut fte);
            self.grenade_arc_frame(v, &mut fte);
            self.hook_fx.frame(&self.cl, self.cl.viewentity.saturating_sub(1), v, &mut self.r, &mut fte);
            let cable_metal = self.cv("addon_cable_metal") != 0.0;
            self.cable_fx.frame(&self.cl, self.cl.viewentity.saturating_sub(1), cable_held, cable_metal, &self.level, v, &mut self.r, &mut fte);
            self.boomerang_fx.frame(self.cl.time, &self.cl, v, &mut self.r, &mut fte);
            let grav = self.cv("sv_gravity");
            let grav = if grav > 0.0 { grav } else { 800.0 };
            let gel_quality = (self.cv("addon_gel_quality").round() as i32).clamp(0, 1) as u32;
            let washer_quality = (self.cv("addon_washer_quality").round() as i32).clamp(0, 1) as u32;
            self.gel_fx.frame(&self.cl, &self.level, &mut self.r, v, grav, self.snd.as_ref(), gel_quality, &mut fte);
            self.water_fx.frame(&self.cl, self.cl.viewentity.saturating_sub(1), v, grav, washer_quality, &mut self.r, &mut fte);
            for p in self.water_fx.wash_points(self.cl.time) {
                self.blood.wash(p, crate::addons::water_fx::WASH_RADIUS);
            }
            self.blood.frame(
                crate::blood::Settings::read(&self.server.core.cvars), &self.cl, &self.level, &mut brushes,
                &self.server.core.fs, &mut self.r, &ents, &dyn_verts,
                v.origin.into(), &self.debris, fte_frustum.as_ref(), &mut fte,
            );
            self.hitfx.frame(
                &self.server.core.cvars, &self.cl, &self.level, &mut brushes, &self.server.core.fs,
                &mut self.r, v, &self.debris, fte_frustum.as_ref(), &mut fte,
            );
            // Projectile pellets already draw their own streaks; hitscan ones would stack.
            let pellets = self.cv("custom_active") != 0.0 && self.cv("addon_projectile_shotguns") != 0.0
                && self.cv("deathmatch") == 0.0;
            let hitscan = false && !pellets; // shotgun tracers disabled (hidden from menus)
            self.tracers.frame(hitscan, &self.cl, &self.debris, v, &mut self.r, &mut fte);
            let bb = crate::bboxes::Settings::read(&self.server.core.cvars);
            if bb.mode != 0 && self.cv("r_drawentities") != 0.0 {
                let eye = Vec3::from(v.origin);
                let boxes = self.server.edict_boxes(self.cl.viewentity as u32, (bb.mode.abs() >= 2).then_some(eye));
                let fwd = qcore::math::angle_vectors(Vec3::from(v.angles)).0;
                self.bboxes.frame(&bb, &boxes, eye, fwd, &mut self.r, &mut fte);
            }
            self.bboxes.pointfile_frame(Vec3::from(v.origin), &mut self.r, &mut fte);
            self.impacts.frame(
                &self.server.core.cvars, &self.cl, &self.level, &mut brushes, &self.server.core.fs,
                &mut self.r, &mut ents, &dyn_verts, v.origin.into(), &self.debris, fte_frustum.as_ref(), &mut fte,
            );
            crate::view::dlights(&self.cl, &mut dls);
            dls.extend_from_slice(&self.test_lights);
            let contents = self.contents(v.origin);
            blend = self.view.blend(&self.cl, contents, ft, cv.flash, cv.cshift);
            if !self.guests.is_empty() {
                splits = self.split_views(*v, blend, ft, &mut ents);
            }
            self.impacts.hide_explosion_sprites(&self.cl, &mut ents);
        }
        // Full CSQC (CSQC_UpdateView): runs after engine entities/dlights are collected
        // so addentities(MASK_ENGINE) can include them, and draws 2D HUD into `c`.
        if view.is_some() && splits.is_empty() && self.csqc.handles_view() {
            (csqc_sbar, csqc_crosshair) = self.csqc_update_view(&mut view, &mut ents, &mut dls, &mut c, ft);
        }
        // 2D
        let glyphs = self.glyphs();
        let pad_ui = self.pad_ui();
        let wheel_key = crate::menu::keys_for(&self.input.bindings, "+weaponwheel")
            .into_iter()
            .find(|&k| crate::glyphs::is_pad_key(k))
            .unwrap_or(qinput::keys::K_XBUTTON);
        let equip_key = self.equip_wheel_key();
        let bullet_meter = self.cv("addon_bullet_time_meter") != 0.0;
        self.update_metrics(ft);
        let mut metrics_drawn = false;
        let cur_speed = self.update_speed();
        let speed = self.speedometer_text(cur_speed);
        let reserve = self.hud_reserve();
        let rays: Vec<_> = std::iter::once(&self.cl).chain(self.guests.iter().filter(|g| g.cl.signon == 4).map(|g| &g.cl))
            .map(|cl| {
                let origin = cl.entities.get(cl.viewentity).map_or([0.0; 3], |e| e.origin);
                let eye = Vec3::from_array(origin) + Vec3::new(0.0, 0.0, cl.viewheight);
                (cl.viewentity as u32, eye, qcore::math::angle_vectors(Vec3::from_array(cl.viewangles)).0)
            }).collect();
        let target_colors: Vec<bool> = rays.into_iter().map(|(id, eye, dir)|
            self.server.reticle_on_target(id, eye, dir)).collect();
        let addon_entries: Vec<_> = crate::wheel::inventory(&self.server.core.cvars, &self.allowed_toys())
            .into_iter().filter(|e| matches!(e.weapon, crate::wheel::Weapon::Addon(_))).collect();
        let hud = self.hud_frame(&c, &addon_entries, cur_speed);
        self.hud.fade.update(&self.hud.layout, &self.cl, (&addon_entries, self.toy.held), self.realtime);
        let teamplay = self.server.core.cvars.value_of("teamplay");
        let captions = self.caption_place().filter(|_| self.cv("cl_captions") != 0.0 && !hud.edit);
        let demo_bar = self.demo_bar();
        let demo_peek = self.demo_peek();
        let exporting = self.exporting();
        {
            let fs = &self.server.core.fs;
            let pics = &mut self.pics;
            let r = &mut self.r;
            let mut pf = |n: &str| pics.get(fs, r, n);
            if view.is_some() {
                match self.cl.intermission {
                    // GAME_DEATHMATCH: the ranking instead of level stats.
                    1 if self.cl.gametype == 1 => crate::sbar::deathmatch_overlay(&mut c, &mut pf, &self.cl),
                    1 => crate::sbar::intermission(&mut c, &mut pf, &self.cl),
                    2 => {
                        crate::sbar::finale(&mut c, &mut pf);
                        let n = ((self.cl.time - self.cl.completed_time) * self.server.core.cvars.value_of("scr_printspeed") as f64) as usize;
                        self.con.draw_center(&mut c, n);
                    }
                    3 => self.con.draw_center(&mut c, usize::MAX),
                    // Split-screen: each player's HUD in their own cell.
                    _ if !splits.is_empty() => {
                        let live: Vec<usize> = (0..self.guests.len()).filter(|&k| self.guests[k].cl.signon == 4).collect();
                        for (i, sv) in splits.iter().enumerate() {
                            let [x, y, w, h] = sv.rect;
                            let mut cc = Canvas::new(w, h);
                            cc.ui = c.ui.clone();
                            let room = crate::hud::draw::Room { vw: cc.vw(), ..hud.room };
                            let cell_sizes = super::hud_frame::sizes(&cc, &self.hud.layout, &room);
                            let cell_rects = crate::hud::place::place_all(&self.hud.layout.elems, &cell_sizes, cc.vw(), cc.vh(), None);
                            let cell_reserve = crate::hud::reserve_bottom(&self.hud.layout, &cell_rects, cc.vh());
                            let damage_view = if i == 0 { &self.view } else { &self.guests[live[i - 1]].view };
                            let damage_height = cc.vh() - cell_reserve;
                            damage_view.draw_damage(&mut cc, damage_height, self.server.core.cvars.value_of("hud_damageindicator") != 0.0, self.server.core.cvars.value_of("v_flashscale"));
                            let feedback = if i == 0 { &self.feedback } else { &self.guests[live[i - 1]].feedback };
                            let marker = (feedback.remaining(self.realtime), feedback.kill);
                            let (cl, fade, face, locked, active, held_addon) = if i == 0 {
                                (&self.cl, &mut self.hud.fade, self.view.face_anim_time > self.cl.time, self.input.lockon.target.is_some(), self.input.lockon.active, self.toy.held)
                            } else {
                                let Some(g) = live.get(i - 1).and_then(|&k| self.guests.get_mut(k)) else { continue };
                                let face = g.view.face_anim_time > g.cl.time;
                                (&g.cl, &mut g.hud_fade, face, g.input.lockon.target.is_some(), g.input.lockon.active, g.toy.held)
                            };
                            if i > 0 {
                                fade.update(&self.hud.layout, cl, (&addon_entries, held_addon), self.realtime);
                            }
                            if cl.intermission == 0 {
                                let vh = cc.vh() - cell_reserve;
                                let color = reticle::color(&self.server.core.cvars, target_colors[i], marker.1 && marker.0 > 0.0);
                                draw_crosshair(&mut cc, vh, &self.server.core.cvars, locked, active, color);
                                hitmarker::draw_hitmarker(&mut cc, vh, &self.server.core.cvars, marker.0, color);
                            }
                            if !hud.edit {
                                let ctx = hud.ctx(cl, face, teamplay, &addon_entries, held_addon, self.show_scores, None);
                                crate::hud::draw_layout(&mut cc, &mut pf, &self.hud.layout, &cell_rects, &ctx, Some(fade), self.realtime, crate::hud::kind::Kind::is_player);
                            }
                            if self.show_scores && cl.gametype == 1 {
                                crate::sbar::deathmatch_overlay(&mut cc, &mut pf, cl);
                            }
                            let con = if i == 0 { &self.con } else { &self.guests[live[i - 1]].center };
                            if con.realtime < con.center_time && !self.con.open {
                                let mut cp = crate::console::Console::new();
                                cp.centerprint.clone_from(&con.centerprint);
                                let at = self.hud.layout.elems.iter().position(|e| e.kind == crate::hud::kind::Kind::Center);
                                if let Some(k) = at.filter(|&k| k < cell_rects.len()) {
                                    cp.center_place = crate::hud::MsgPlace::of(&self.hud.layout.elems[k], cell_rects[k]);
                                }
                                cp.draw_center(&mut cc, usize::MAX);
                            }
                            let (wheel, wcl) = if i == 0 { (&self.wheel, &self.cl) } else { (&self.guests[live[i - 1]].wheel, &self.guests[live[i - 1]].cl) };
                            self.bullet_time.draw(&mut cc, bullet_meter);
                            wheel.draw(&mut cc, &mut pf, &glyphs, wcl, pad_ui, wheel_key);
                            crate::draw::translate(&mut cc.list, x as f32, y as f32);
                            c.list.append(&mut cc.list);
                        }
                    }
                    _ => {
                        let scene_h = c.vh() - reserve;
                        let viewsize = self.server.core.cvars.value_of("viewsize");
                        self.view.draw_damage(&mut c, scene_h, self.server.core.cvars.value_of("hud_damageindicator") != 0.0, self.server.core.cvars.value_of("v_flashscale"));
                        if viewsize < 130.0 && csqc_crosshair {
                            let color = reticle::color(&self.server.core.cvars, target_colors[0], self.feedback.kill && self.feedback.remaining(self.realtime) > 0.0);
                            draw_crosshair(&mut c, scene_h, &self.server.core.cvars, self.input.lockon.target.is_some(), self.input.lockon.active, color);
                            hitmarker::draw_hitmarker(&mut c, scene_h, &self.server.core.cvars, self.feedback.remaining(self.realtime), color);
                        }
                        let face = self.view.face_anim_time > self.cl.time;
                        let player_pos = Vec3::from_array(self.cl.entities.get(self.cl.viewentity).map_or([0.0; 3], |e| e.origin));
                        let automap_allowed = self.cl.gametype != 1 && self.server.core.cvars.value_of("deathmatch") == 0.0;
                        let hud_start = c.list.len();
                        let simple_csqc_hud = self.csqc.active() && !self.csqc.handles_view() && self.csqc.handles_hud();
                        if !hud.edit && viewsize < 120.0 && csqc_sbar && !simple_csqc_hud {
                            let am = crate::hud::draw::AutomapCtx {
                                map: &self.automap,
                                settings: crate::automap::Automap::settings(&self.server.core.cvars),
                                player_pos,
                                player_yaw: self.cl.viewangles[1],
                                allowed: automap_allowed,
                            };
                            let ctx = hud.ctx(&self.cl, face, teamplay, &addon_entries, self.toy.held, self.show_scores, Some(am));
                            crate::hud::draw_layout(&mut c, &mut pf, &self.hud.layout, &self.hud.rects, &ctx, Some(&self.hud.fade), self.realtime, |_| true);
                        } else if !hud.edit && viewsize < 120.0 && simple_csqc_hud {
                            // Simple CSQC HUD runs after releasing the local `pf` borrow below.
                        }
                        metrics_drawn = true;
                        // the demo bar takes the bottom of the screen: lift the HUD over it
                        if let Some(b) = &demo_bar {
                            let dy = -super::demo_bar::lift(b.pad) * c.scale;
                            crate::draw::translate(&mut c.list[hud_start..], 0.0, dy);
                        }
                        if self.show_scores && self.cl.gametype == 1 {
                            crate::sbar::deathmatch_overlay(&mut c, &mut pf, &self.cl);
                        }
                        self.bullet_time.draw(&mut c, bullet_meter);
                        self.wheel.draw(&mut c, &mut pf, &glyphs, &self.cl, pad_ui, wheel_key);
                        self.goggles.wheel.draw(&mut c, &glyphs, &self.goggles.equip, pad_ui, equip_key);
                        if let Some((at, ts, color)) = captions {
                            self.captions.draw(&mut c, self.realtime, at, ts, color);
                        }
                        if self.show_scores && !hud.edit && !self.menu.active() && !self.automap.fullscreen && automap_allowed {
                            let has_status_bar = self.hud.layout.elems.iter().any(|e| e.kind == crate::hud::kind::Kind::StatusBar);
                            let top_reserve = if has_status_bar { 10.0 } else { 30.0 };
                            let header = crate::automap::HeaderInfo::default();
                            self.automap.draw(
                                &mut c,
                                &self.server.core.cvars,
                                player_pos,
                                self.cl.viewangles[1],
                                top_reserve,
                                reserve,
                                header,
                                pad_ui,
                                &glyphs,
                            );
                        }
                    }
                }
                // QC accepts the exit after 2 s (5 s in deathmatch).
                let wait = if self.cl.gametype == 1 { 5.0 } else { 2.0 };
                if self.cl.intermission == 1 && self.cl.time - self.cl.completed_time >= wait && !self.menu.active() {
                    let s = if pad_ui { qcore::t!("Press any button to continue...") } else { qcore::t!("Press any key to continue...") };
                    c.center_320x200();
                    let w = c.ptext_w(s);
                    let x = ((320.0 - w) / 2.0).floor();
                    c.fill(x - 6.0, 185.0, w + 12.0, 14.0, [0.0, 0.0, 0.0, 0.6]);
                    crate::menu::gold(&mut c, x, 188.0, s);
                    c.reset_origin();
                }
            }
            let conback = pf("gfx/conback.lmp");
            let ingame = view.is_some() && self.cl.intermission == 0;
            if ingame && self.automap.fullscreen && !self.menu.active()
                && self.cl.gametype != 1 && self.server.core.cvars.value_of("deathmatch") == 0.0
            {
                let player_pos = Vec3::from_array(self.cl.entities.get(self.cl.viewentity).map_or([0.0; 3], |e| e.origin));
                let header = crate::automap::HeaderInfo {
                    title: &self.cl.levelname,
                    kills: self.cl.stats[qnet::protocol::STAT_MONSTERS],
                    total_kills: self.cl.stats[qnet::protocol::STAT_TOTALMONSTERS],
                    secrets: self.cl.stats[qnet::protocol::STAT_SECRETS],
                    total_secrets: self.cl.stats[qnet::protocol::STAT_TOTALSECRETS],
                };
                self.automap.draw(&mut c, &self.server.core.cvars, player_pos, self.cl.viewangles[1], 0.0, reserve, header, pad_ui, &glyphs);
            }
            // Split screen draws center prints in each player's cell.
            let center = ingame && !self.menu.active() && !self.automap.fullscreen && splits.is_empty();
            if !(self.clean_shot && self.screenshot.is_some()) && self.thumb.is_none() {
                // exported video keeps centerprints but not console messages
                self.con.draw(&mut c, conback, ingame && !self.menu.active() && !self.automap.fullscreen && !exporting, center);
            }
            if self.server.core.sv.paused && view.is_some() {
                let p = pf("gfx/pause.lmp");
                if let Some((_, pw, _)) = p {
                    c.pic(((c.vw() - pw as f32) / 2.0).floor(), 48.0, p);
                }
            }
            if self.cl.intermission == 0 {
                touch::draw_touch(&mut c, &self.input, &self.server.core.cvars);
            }
            if let Some(b) = &demo_bar {
                super::demo_bar::draw(&mut c, b, &glyphs);
            }
            if let Some(p) = &demo_peek {
                super::demo_bar::draw_peek(&mut c, p, &glyphs);
            }
            self.menu.mods.online.poll();
            // the HUD editor's preview, under its dialog
            if hud.edit {
                let player_pos = Vec3::from_array(self.cl.entities.get(self.cl.viewentity).map_or([0.0; 3], |e| e.origin));
                let am = crate::hud::draw::AutomapCtx {
                    map: &self.automap,
                    settings: crate::automap::Automap::settings(&self.server.core.cvars),
                    player_pos,
                    player_yaw: self.cl.viewangles[1],
                    allowed: true,
                };
                let ctx = hud.ctx(&self.cl, false, teamplay, &addon_entries, None, false, Some(am));
                crate::hud::draw_layout(&mut c, &mut pf, &self.hud.layout, &self.hud.rects, &ctx, None, self.realtime, |_| true);
            }
            self.menu.draw(&mut c, &mut pf, &self.server.core.cvars, self.realtime);
            // outside the in-game HUD (menus, intermission, split screen) the
            // metrics go over everything
            if !metrics_drawn && !hud.edit {
                let ctx = hud.ctx(&self.cl, false, teamplay, &addon_entries, None, false, None);
                crate::hud::draw_layout(&mut c, &mut pf, &self.hud.layout, &self.hud.rects, &ctx, None, self.realtime, crate::hud::kind::Kind::is_metric);
            }
            if let Some(s) = speed.filter(|_| ingame && !self.menu.active()) {
                // Just below the crosshair, centred (ironwail's CANVAS_CROSSHAIR).
                c.reset_origin();
                let ofs = self.cv("scr_showspeed_ofs");
                let y = (c.vh() / 2.0 + 4.0 + ofs).clamp(0.0, c.vh() - 8.0);
                c.ptext((c.vw() / 2.0 - c.ptext_w(&s) / 2.0).floor(), y.floor(), &s, crate::draw::WHITE);
            }
        }
        // Simple CSQC HUD (CSQC_DrawHud / CSQC_DrawScores): drawn in-game when no menu is open.
        if view.is_some()
            && self.cl.intermission == 0
            && splits.is_empty()
            && !hud.edit
            && !self.menu.active()
            && self.cv("viewsize") < 120.0
            && self.csqc.active()
            && !self.csqc.handles_view()
            && self.csqc.handles_hud()
        {
            self.csqc_draw_hud(&mut c, ft);
        }
        let styles: Vec<i32> = (0..qnet::protocol::MAX_LIGHTSTYLES).map(|i| self.cl.lightstyle_value(i)).collect();
        // r_fog 0 hides fog without touching the map/mod values, so it comes back
        // as soon as the option is turned on again.
        let fog_on = self.cv("r_fog") != 0.0;
        let fog = qrender::Fog { density: if fog_on { self.cl.fog.density } else { 0.0 }, color: self.cl.fog.color };
        let v = |n: &str| self.cv(n);
        let enhanced_alpha = v("r_enhancedwater") != 0.0 && v("r_watertranslucency") != 0.0;
        let settings = Settings {
            gamma: v("gamma").clamp(0.3, 3.0),
            contrast: v("contrast").clamp(0.5, 3.0),
            // As ironwail/vkQuake: the map's worldspawn `wateralpha`, else
            // `r_wateralpha` (default 1; mods set it too: AD's progs stuffs
            // its maps' `water_alpha`). Translucent Enhanced Water lets
            // `r_wateralpha` override the map (and VIS: translucent liquids
            // then skip leaf PVS). Other liquid types retain map overrides
            // and zero-to-water fallback.
            wateralpha: if enhanced_alpha { v("r_wateralpha") } else { self.level.liquid_alpha[0].unwrap_or(v("r_wateralpha")) }.clamp(0.0, 1.0),
            // ironwail sets r_wateralpha's map alpha to 1 at load where VIS
            // can't see through water; the map's own key applies anyway.
            wateralpha_vised_only: !enhanced_alpha && self.level.liquid_alpha[0].is_none(),
            lavaalpha: self.level.liquid_alpha[1].or(Some(v("r_lavaalpha"))).filter(|&a| a > 0.0).map(|a| a.min(1.0)),
            slimealpha: self.level.liquid_alpha[2].or(Some(v("r_slimealpha"))).filter(|&a| a > 0.0).map(|a| a.min(1.0)),
            telealpha: self.level.liquid_alpha[3].or(Some(v("r_telealpha"))).filter(|&a| a > 0.0).map(|a| a.min(1.0)),
            teleport_windows: v("r_teleportwindows") != 0.0,
            portal_quality: (v("r_portal_quality").round() as i32).clamp(0, 2) as u32,
            gun_portal_max: v("addon_portal_max").clamp(0.0, 4.0) as u32,
            gun_portal_dist: v("addon_portal_dist").max(0.0),
            gun_portal_depth: v("addon_portal_depth").clamp(0.0, 6.0) as u32,
            skyfog: v("r_skyfog"),
            dynamic: v("r_dynamic") != 0.0,
            // cheat-protected as in ironwail: single player only
            fullbright: v("r_fullbright") != 0.0 && self.cl.maxclients <= 1,
            sky_wind: self.level.sky_wind.uniform(self.cl.time, v("r_skywind")),
            vsync: v("vid_vsync") != 0.0,
            msaa: v("r_msaa").max(1.0) as u32,
            render_scale: v("r_renderscale").clamp(0.25, 2.0),
            texture_linear: v("r_texfilter") != 0.0,
            anisotropy: v("r_anisotropy").clamp(1.0, 16.0),
            texture_mode: qrender::parse_texture_mode(self.cvs("gl_texturemode")),
            lod_bias: {
                let s = self.cvs("gl_lodbias");
                if s.eq_ignore_ascii_case("auto") { None } else { s.trim().parse().ok() }
            },
            supersample: v("vid_fsaamode") != 0.0,
            dither: v("r_dither").max(0.0),
            softemu: v("r_softemu").clamp(0.0, 3.0) as u32,
            softemu_metric: v("r_softemu_metric") as i32,
            lightmap_banding: v("r_softemu_lightmap_banding"),
            mdl_warp: v("r_softemu_mdl_warp"),
            dither_screen: v("r_softemu_dither_screen"),
            dither_texture: v("r_softemu_dither_texture"),
            colorblind: v("r_colorblind").clamp(0.0, 3.0) as u32,
            soft_particle_distance: if soft_particles(v("r_particle_quality")) > 0 { v("r_softparticledistance").max(0.01) } else { 0.0 },
            soft_particle_quality: u32::from(soft_particles(v("r_particle_quality")) >= 2),
            particle_style: if v("r_particles").round() as i32 == 2 { 2 } else { 1 },
            ssao: if (v("r_ssao").round() as i32).clamp(0, 3) > 0 { v("r_ssao_strength").max(0.0) } else { 0.0 },
            ssao_quality: (v("r_ssao").round() as i32).clamp(0, 3).saturating_sub(1).min(2) as u32,
            ssao_radius: v("r_ssao_radius"),
            edge_blur: if self.bullet_time.active && v("a11y_reducemotion") == 0.0 { 0.65 } else { 0.0 },
            // Reduce Motion turns motion blur off
            motion_blur: if v("a11y_reducemotion") != 0.0 || v("r_motionblur") == 0.0 {
                0.0
            } else {
                v("r_motionblur_amount").clamp(0.0, 4.0)
            },
            motion_blur_quality: (v("r_motionblur_quality").round() as i32).clamp(0, 1) as u32,
            // planar reflections (Water Settings, under the Enhanced Water master)
            water_reflections: if v("r_enhancedwater") != 0.0 { (v("r_water_reflections").round() as i32).clamp(0, 2) as u32 } else { 0 },
            water_reflection_amount: v("r_water_reflections_amount").clamp(0.0, 1.5),
            bloom: if v("r_bloom") != 0.0 { v("r_bloom_intensity").clamp(0.0, 4.0) } else { 0.0 },
            bloom_size: v("r_bloom_size").clamp(0.5, 2.0),
            bloom_bright: v("r_bloom_bright").clamp(0.0, 1.0),
            bloom_quality: (v("r_bloom_quality").round() as i32).clamp(0, 1) as u32,
            rt_dlights: v("r_shadow_realtime_dlight") != 0.0,
            dlight_shadows: v("r_shadow_realtime_dlight_shadows") != 0.0,
            shadow_quality: (v("r_shadow_quality").round() as i32).clamp(0, 2) as u32,
            water_ripples: if v("r_enhancedwater") != 0.0 && v("r_waterripples") != 0.0 { v("r_waterripples_amount").clamp(0.0, 4.0) } else { 0.0 },
            water_splashes: v("r_enhancedwater") != 0.0 && v("r_watersplash") != 0.0,
            water_caustics: if v("r_enhancedwater") != 0.0 && v("r_watercaustics") != 0.0 { v("r_watercaustics_amount").clamp(0.0, 4.0) } else { 0.0 },
            water_quality: (v("r_water_quality").round() as i32).clamp(0, 1) as u32,
            gel_quality: (v("addon_gel_quality").round() as i32).clamp(0, 1) as u32,
            shader_scope: v("r_shader_scope").clamp(0.0, 1.0) as u32,
            shader_scale: v("r_shader_scale").clamp(0.0, 1.0) as u32,
            hdr_lightmaps: v("r_hdr_lightmaps") != 0.0,
            deluxemapping: v("r_deluxemapping") != 0.0,
            deluxemapping_specular: v("r_deluxemapping_specular").clamp(0.0, 2.0),
            goggles,
            ..Settings::default()
        };
        self.upload_ui_text();
        self.last_settings = Some(settings);
        let impacts = std::mem::take(&mut self.cl.shot_impacts);
        self.cl.blood_hits.clear();
        self.cl.weapon_impacts.clear();
        self.cl.impact_effects.clear();
        let gun_portals = crate::addons::portal_draw::gun_portals(&self.cl.portals, self.cv("addon_portal_view") != 0.0);
        let frame = qrender::RenderFrame {
            gun_portals: &gun_portals,
            teleport_ripples: self.impacts.teleport_ripples(),
            time: self.cl.time,
            view,
            entities: &ents,
            particles: &parts,
            fte_vertices: &fte.0,
            fte_indices: &fte.1,
            fte_batches: &fte.2,
            dlights: &dls,
            lightstyles: &styles,
            fog,
            blend,
            draw2d: &c.list,
            settings,
            shot_impacts: &impacts,
            dyn_verts: &dyn_verts,
            splits: &splits,
            player: self.cl.entities.get(self.cl.viewentity).filter(|_| self.cl.viewentity > 0).map(|e| e.origin),
        };
        if let Err(e) = self.r.render(&frame) {
            self.con.print(&format!("render error: {e:?}\n"));
        }
        if view.is_some() {
            self.fte.recycle_geometry(fte);
        }
    }
}

impl Host {
    /// Sends the UI text atlas to the renderer when it changed.
    pub(super) fn upload_ui_text(&mut self) {
        let mut ui = self.ui_text.borrow_mut();
        if ui.dirty
            && let Some(pic) = ui.pic
        {
            if let Err(e) = self.r.update_pic(pic, &ui.px) {
                self.con.print(&format!("ui text upload: {e:?}\n"));
            }
            ui.after_upload();
        }
    }
}

/// Scripted particle count scale for `r_particle_quality` (low halves it).
pub(super) fn particle_density(quality: f32) -> f32 {
    if quality.round() <= 0.0 { 0.5 } else { 1.0 }
}

/// Soft particle level (0 off, 1 low, 2 high) for `r_particle_quality`.
pub(super) fn soft_particles(quality: f32) -> i32 {
    quality.round().clamp(0.0, 2.0) as i32
}
