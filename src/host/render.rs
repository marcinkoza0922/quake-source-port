use super::*;

impl Host {
    pub(super) fn render(&mut self, view: Option<qrender::View>, gun: Option<qrender::RenderEntity>, ft: f32) {
        self.sync_shader();
        let (w, h) = self.r.size();
        let mut c = Canvas::new(w, h);
        self.con.notify_time = self.cv("con_notifytime").max(0.0) as f64;
        self.con.text_bg = self.cv("scr_textbg") != 0.0;
        self.con.text_scale = self.cv("scr_textscale");
        self.con.center_pos = self.cv("scr_centerpos").clamp(0.0, 2.0) as u8;
        self.con.notify_pos = self.cv("con_notifypos").clamp(0.0, 2.0) as u8;
        self.con.modern = self.cv("con_modern") != 0.0;
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
        let view = view.map(|mut v| {
            v.waterwarp = self.waterwarp(v.origin, self.cl.time);
            v
        });
        if let Some(v) = &view {
            let fs = &self.server.core.fs;
            let cv = self.view_cvars();
            self.view.replaced_models = self.fte.replaced_models(&self.cl, fs);
            self.view.with_self = !self.cl.portals.is_empty();
            self.view.dodge_pose = self.server.level.as_ref()
                .and_then(|l| l.dodges.get(self.cl.viewentity.saturating_sub(1)).map(|d| d.pose(l.vm.time)))
                .unwrap_or_default();
            self.view.third_person = self.camera().third_person();
            self.view.entities(&mut self.cl, &cv, &mut self.level, fs, &mut self.r, &mut ents);
            // player 1's own body belongs to player 1's portal views only
            for e in ents.iter_mut().filter(|e| e.portal_only) {
                e.views = 1;
            }
            let debris_on = self.cv("r_debris") != 0.0;
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
            if crate::blood::Settings::read(&self.server.core.cvars).enabled || (crate::impacts::impacts_on(&self.server.core.cvars) && (self.cv("r_impact_nails") != 0.0 || self.cv("r_impact_bullets") != 0.0)) {
                self.debris.track_shooters(&self.cl, &self.level, self.cl.time);
            }
            let fte_frustum = self.cull_fte_for_view().then(|| {
                qrender::ViewFrustum::from_view(v, (w, h), self.cv("r_renderscale"))
            });
            self.debris.entities(&mut ents, &mut dyn_verts, fte_frustum.as_ref());
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
            let mut brushes = None;
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
            self.impacts.frame(
                &self.server.core.cvars, &self.cl, &self.level, &mut brushes, &self.server.core.fs,
                &mut self.r, &mut ents, &dyn_verts, v.origin.into(), &self.debris, fte_frustum.as_ref(), &mut fte,
            );
            crate::view::dlights(&self.cl, &mut dls);
            let contents = self.contents(v.origin);
            blend = self.view.blend(&self.cl, contents, ft, cv.flash);
            if !self.guests.is_empty() {
                splits = self.split_views(*v, blend, ft, &mut ents);
            }
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
        let showfps = self.cv("scr_showfps") != 0.0;
        let speed = self.speedometer();
        let classic = self.classic_hud();
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
        let hud_cfg = crate::sbar::HudConfig::from_cvars(&self.server.core.cvars);
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
                        let pack = crate::sbar::Pack::from_game_dirs(self.server.core.fs.game_dirs().iter().map(String::as_str));
                        let teamplay = self.server.core.cvars.value_of("teamplay");
                        let live: Vec<usize> = (0..self.guests.len()).filter(|&k| self.guests[k].cl.signon == 4).collect();
                        for (i, sv) in splits.iter().enumerate() {
                            let [x, y, w, h] = sv.rect;
                            let mut cc = Canvas::new(w, h);
                            cc.ui = c.ui.clone();
                            let damage_view = if i == 0 { &self.view } else { &self.guests[live[i - 1]].view };
                            let damage_height = cc.vh() - if classic { CLASSIC_SBAR_LINES } else { 0.0 };
                            damage_view.draw_damage(&mut cc, damage_height, self.server.core.cvars.value_of("hud_damageindicator") != 0.0, self.server.core.cvars.value_of("v_flashscale"));
                            let feedback = if i == 0 { &self.feedback } else { &self.guests[live[i - 1]].feedback };
                            let marker = (feedback.remaining(self.realtime), feedback.kill);
                            let (cl, fade, face, locked, active, held_addon) = if i == 0 {
                                (&self.cl, &mut self.hud_fade, self.view.face_anim_time > self.cl.time, self.input.lockon.target.is_some(), self.input.lockon.active, self.toy.held)
                            } else {
                                let Some(g) = live.get(i - 1).and_then(|&k| self.guests.get_mut(k)) else { continue };
                                let face = g.view.face_anim_time > g.cl.time;
                                (&g.cl, &mut g.hud_fade, face, g.input.lockon.target.is_some(), g.input.lockon.active, g.toy.held)
                            };
                            if cl.intermission == 0 {
                                let vh = cc.vh() - if classic { CLASSIC_SBAR_LINES } else { 0.0 };
                                let color = reticle::color(&self.server.core.cvars, target_colors[i], marker.1 && marker.0 > 0.0);
                                draw_crosshair(&mut cc, vh, &self.server.core.cvars, locked, active, color);
                                hitmarker::draw_hitmarker(&mut cc, vh, &self.server.core.cvars, marker.0, color);
                            }
                            let mut sb = crate::sbar::Sbar::new(&mut cc, &mut pf, pack);
                            sb.teamplay = teamplay;
                            sb.addon_weapons = addon_entries.clone();
                            sb.held_addon = held_addon;
                            if classic {
                                sb.draw(cl, self.show_scores, face);
                            } else {
                                sb.draw_modern(cl, self.show_scores, face, &hud_cfg, fade, self.realtime);
                            }
                            if self.show_scores && cl.gametype == 1 {
                                crate::sbar::deathmatch_overlay(&mut cc, &mut pf, cl);
                            }
                            let con = if i == 0 { &self.con } else { &self.guests[live[i - 1]].center };
                            if con.realtime < con.center_time && !self.con.open {
                                let mut cp = crate::console::Console::new();
                                cp.centerprint.clone_from(&con.centerprint);
                                (cp.text_bg, cp.text_scale, cp.center_pos) = (self.con.text_bg, self.con.text_scale, self.con.center_pos);
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
                        let scene_h = c.vh() - if classic { CLASSIC_SBAR_LINES } else { 0.0 };
                        self.view.draw_damage(&mut c, scene_h, self.server.core.cvars.value_of("hud_damageindicator") != 0.0, self.server.core.cvars.value_of("v_flashscale"));
                        {
                            let color = reticle::color(&self.server.core.cvars, target_colors[0], self.feedback.kill && self.feedback.remaining(self.realtime) > 0.0);
                            draw_crosshair(&mut c, scene_h, &self.server.core.cvars, self.input.lockon.target.is_some(), self.input.lockon.active, color);
                            hitmarker::draw_hitmarker(&mut c, scene_h, &self.server.core.cvars, self.feedback.remaining(self.realtime), color);
                        }
                        let face = self.view.face_anim_time > self.cl.time;
                        let pack = crate::sbar::Pack::from_game_dirs(self.server.core.fs.game_dirs().iter().map(String::as_str));
                        let mut sb = crate::sbar::Sbar::new(&mut c, &mut pf, pack);
                        sb.teamplay = self.server.core.cvars.value_of("teamplay");
                        sb.addon_weapons = addon_entries.clone();
                        sb.held_addon = self.toy.held;
                        if classic {
                            sb.backtile(CLASSIC_SBAR_LINES);
                            sb.draw(&self.cl, self.show_scores, face);
                        } else {
                            sb.draw_modern(&self.cl, self.show_scores, face, &hud_cfg, &mut self.hud_fade, self.realtime);
                        }
                        if self.show_scores && self.cl.gametype == 1 {
                            crate::sbar::deathmatch_overlay(&mut c, &mut pf, &self.cl);
                        }
                        self.bullet_time.draw(&mut c, bullet_meter);
                        self.wheel.draw(&mut c, &mut pf, &glyphs, &self.cl, pad_ui, wheel_key);
                        self.goggles.wheel.draw(&mut c, &glyphs, &self.goggles.equip, pad_ui, equip_key);
                        if self.server.core.cvars.value_of("cl_captions") != 0.0 {
                            let bottom = if classic { scene_h - 4.0 } else { c.vh() - 44.0 };
                            let ts = self.server.core.cvars.value_of("scr_textscale");
                            self.captions.draw(&mut c, self.realtime, bottom, ts);
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
            // Split screen draws center prints in each player's cell.
            let center = ingame && !self.menu.active() && splits.is_empty();
            if !(self.clean_shot && self.screenshot.is_some()) && self.thumb.is_none() {
                self.con.draw(&mut c, conback, ingame && !self.menu.active(), center);
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
            self.menu.mods.online.poll();
            self.menu.draw(&mut c, &mut pf, &self.server.core.cvars, self.realtime);
            if showfps {
                self.fps.0 += ft as f64;
                self.fps.1 += 1;
                if self.fps.0 >= 0.5 {
                    self.fps.2 = (self.fps.1 as f64 / self.fps.0) as f32;
                    self.fps = (0.0, 0, self.fps.2);
                }
                let s = format!("{:4.0} fps", self.fps.2);
                c.reset_origin();
                let x = c.vw() - c.ptext_w(&s) - 8.0;
                c.ptext(x, 8.0, &s, crate::draw::WHITE);
            }
            if let Some(s) = speed.filter(|_| ingame && !self.menu.active()) {
                // Just below the crosshair, centred (ironwail's CANVAS_CROSSHAIR).
                c.reset_origin();
                let ofs = self.cv("scr_showspeed_ofs");
                let y = (c.vh() / 2.0 + 4.0 + ofs).clamp(0.0, c.vh() - 8.0);
                c.ptext((c.vw() / 2.0 - c.ptext_w(&s) / 2.0).floor(), y.floor(), &s, crate::draw::WHITE);
            }
        }
        let styles: Vec<i32> = (0..qnet::protocol::MAX_LIGHTSTYLES).map(|i| self.cl.lightstyle_value(i)).collect();
        // r_fog 0 hides fog without touching the map/mod values, so it comes back
        // as soon as the option is turned on again.
        let fog_on = self.cv("r_fog") != 0.0;
        let fog = qrender::Fog { density: if fog_on { self.cl.fog.density } else { 0.0 }, color: self.cl.fog.color };
        let v = |n: &str| self.cv(n);
        let settings = Settings {
            gamma: v("gamma").clamp(0.3, 3.0),
            contrast: v("contrast").clamp(0.5, 3.0),
            // Enhanced water explicitly controls opacity; other liquid types
            // retain map overrides and zero-to-water fallback.
            wateralpha: if v("r_enhancedwater") != 0.0 && v("r_watertranslucency") != 0.0 {
                v("r_wateralpha").clamp(0.0, 1.0)
            } else { 1.0 },
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
            water_ripples: if v("r_enhancedwater") != 0.0 && v("r_waterripples") != 0.0 { v("r_waterripples_amount").clamp(0.0, 4.0) } else { 0.0 },
            water_splashes: v("r_enhancedwater") != 0.0 && v("r_watersplash") != 0.0,
            water_caustics: if v("r_enhancedwater") != 0.0 && v("r_watercaustics") != 0.0 { v("r_watercaustics_amount").clamp(0.0, 4.0) } else { 0.0 },
            water_quality: (v("r_water_quality").round() as i32).clamp(0, 1) as u32,
            gel_quality: (v("addon_gel_quality").round() as i32).clamp(0, 1) as u32,
            shader_scope: v("r_shader_scope").clamp(0.0, 1.0) as u32,
            shader_scale: v("r_shader_scale").clamp(0.0, 1.0) as u32,
            goggles,
            ..Settings::default()
        };
        {
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
        let impacts = std::mem::take(&mut self.cl.shot_impacts);
        self.cl.blood_hits.clear();
        self.cl.weapon_impacts.clear();
        let gun_portals = crate::addons::portal_draw::gun_portals(&self.cl.portals, self.cv("addon_portal_view") != 0.0);
        let frame = qrender::RenderFrame {
            gun_portals: &gun_portals,
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

/// Scripted particle count scale for `r_particle_quality` (low halves it).
pub(super) fn particle_density(quality: f32) -> f32 {
    if quality.round() <= 0.0 { 0.5 } else { 1.0 }
}

/// Soft particle level (0 off, 1 low, 2 high) for `r_particle_quality`.
pub(super) fn soft_particles(quality: f32) -> i32 {
    quality.round().clamp(0.0, 2.0) as i32
}
