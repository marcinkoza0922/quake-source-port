use super::*;

impl Host {
    pub(super) fn render(&mut self, view: Option<qrender::View>, gun: Option<qrender::RenderEntity>, ft: f32) {
        let (w, h) = self.r.size();
        let mut c = Canvas::new(w, h);
        self.con.notify_time = self.cv("con_notifytime").max(0.0) as f64;
        self.con.text_bg = self.cv("scr_textbg") != 0.0;
        self.con.text_scale = self.cv("scr_textscale");
        self.con.center_pos = self.cv("scr_centerpos").clamp(0.0, 2.0) as u8;
        self.con.notify_pos = self.cv("con_notifypos").clamp(0.0, 2.0) as u8;
        self.sync_ui_text();
        c.ui = Some(self.ui_text.clone());
        let mut ents = Vec::new();
        let mut dyn_verts = Vec::new();
        let mut parts = Vec::new();
        let mut dls = Vec::new();
        let mut fte = (Vec::new(), Vec::new(), Vec::new());
        let mut blend = [0.0; 4];
        if let Some(v) = &view {
            let fs = &self.server.core.fs;
            let cv = self.view_cvars();
            self.view.replaced_models = self.fte.replaced_models(&self.cl, fs);
            self.view.entities(&mut self.cl, &cv, &mut self.level, fs, &mut self.r, &mut ents);
            let debris_on = self.cv("r_debris") != 0.0;
            let gravity = self.cv("sv_gravity");
            let set = crate::debris::Settings {
                ragdolls: self.cv("r_debris_ragdolls") != 0.0,
                at_death: self.cv("r_debris_ragdollstart") != 0.0,
                max_ragdolls: self.cv("r_debris_maxragdolls").clamp(1.0, 128.0) as usize,
                gibs: self.cv("r_debris_gibs") != 0.0,
                shrapnel: self.cv("r_debris_shrapnel") != 0.0,
                movers: self.cv("r_debris_movers") != 0.0,
                force: self.cv("r_debris_force").clamp(0.0, 6.0),
                kill_lift: self.cv("r_debris_killlift") != 0.0,
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
            self.debris.entities(&mut ents, &mut dyn_verts);
            if self.cv("r_drawentities") == 0.0 {
                ents.clear();
            }
            if let Some(g) = gun {
                ents.push(g);
            }
            crate::view::particles(&self.cl, &mut parts);
            fte = self.fte.frame(
                &mut self.cl,
                self.level.bsp.as_ref(),
                &self.server.core.fs,
                &mut self.r,
                self.snd.as_mut(),
                v,
                &self.level.mapname,
            );
            crate::view::dlights(&self.cl, &mut dls);
            let contents = self.contents(v.origin);
            blend = self.view.blend(&self.cl, contents, ft, cv.flash);
        }
        // 2D
        let glyphs = self.glyphs();
        let pad_ui = self.pad_ui();
        let wheel_key = crate::menu::keys_for(&self.input.bindings, "+weaponwheel")
            .into_iter()
            .find(|&k| crate::glyphs::is_pad_key(k))
            .unwrap_or(qinput::keys::K_XBUTTON);
        let crosshair = self.cv("crosshair") != 0.0;
        let showfps = self.cv("scr_showfps") != 0.0;
        let classic = self.classic_hud();
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
                    _ => {
                        let scene_h = c.vh() - if classic { CLASSIC_SBAR_LINES } else { 0.0 };
                        if crosshair {
                            draw_crosshair(&mut c, scene_h, &self.server.core.cvars);
                        }
                        let face = self.view.face_anim_time > self.cl.time;
                        let pack = crate::sbar::Pack::from_game_dirs(self.server.core.fs.game_dirs().iter().map(String::as_str));
                        let mut sb = crate::sbar::Sbar::new(&mut c, &mut pf, pack);
                        sb.teamplay = self.server.core.cvars.value_of("teamplay");
                        if classic {
                            sb.backtile(CLASSIC_SBAR_LINES);
                            sb.draw(&self.cl, self.show_scores, face);
                        } else {
                            sb.draw_modern(&self.cl, self.show_scores, face, &hud_cfg, &mut self.hud_fade, self.realtime);
                        }
                        if self.show_scores && self.cl.gametype == 1 {
                            crate::sbar::deathmatch_overlay(&mut c, &mut pf, &self.cl);
                        }
                        self.wheel.draw(&mut c, &mut pf, &glyphs, &self.cl, pad_ui, wheel_key);
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
                    let s = if pad_ui { "Press any button to continue..." } else { "Press any key to continue..." };
                    c.center_320x200();
                    let x = ((320.0 - s.len() as f32 * 8.0) / 2.0).floor();
                    c.fill(x - 6.0, 185.0, s.len() as f32 * 8.0 + 12.0, 14.0, [0.0, 0.0, 0.0, 0.6]);
                    crate::menu::gold(&mut c, x, 188.0, s);
                    c.reset_origin();
                }
            }
            let conback = pf("gfx/conback.lmp");
            let ingame = view.is_some() && self.cl.intermission == 0;
            self.con.draw(&mut c, conback, ingame && !self.menu.active(), ingame && !self.menu.active());
            if self.server.core.sv.paused && view.is_some() {
                let p = pf("gfx/pause.lmp");
                if let Some((_, pw, _)) = p {
                    c.pic(((c.vw() - pw as f32) / 2.0).floor(), 48.0, p);
                }
            }
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
                let x = c.vw() - s.len() as f32 * 8.0 - 8.0;
                c.string(x, 8.0, &s);
            }
        }
        let styles: Vec<i32> = (0..qnet::protocol::MAX_LIGHTSTYLES).map(|i| self.cl.lightstyle_value(i)).collect();
        let fog = qrender::Fog { density: self.cl.fog.density, color: self.cl.fog.color };
        let v = |n: &str| self.cv(n);
        let settings = Settings {
            gamma: v("gamma").clamp(0.3, 3.0),
            contrast: v("contrast").clamp(0.5, 3.0),
            // GL_WaterAlphaForTextureType: map keys win; per-type values of
            // 0 fall back to r_wateralpha.
            wateralpha: self.level.liquid_alpha[0].unwrap_or(v("r_wateralpha")).clamp(0.0, 1.0),
            lavaalpha: self.level.liquid_alpha[1].or(Some(v("r_lavaalpha"))).filter(|&a| a > 0.0).map(|a| a.min(1.0)),
            slimealpha: self.level.liquid_alpha[2].or(Some(v("r_slimealpha"))).filter(|&a| a > 0.0).map(|a| a.min(1.0)),
            telealpha: self.level.liquid_alpha[3].or(Some(v("r_telealpha"))).filter(|&a| a > 0.0).map(|a| a.min(1.0)),
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
            soft_particle_distance: if v("r_softparticles") != 0.0 { v("r_softparticledistance").max(0.01) } else { 0.0 },
            ssao: v("r_ssao").max(0.0),
            ssao_radius: v("r_ssao_radius"),
            // Reduce Motion turns motion blur off
            motion_blur: if v("a11y_reducemotion") != 0.0 || v("r_motionblur") == 0.0 {
                0.0
            } else {
                v("r_motionblur_amount").clamp(0.0, 4.0)
            },
            water_ripples: if v("r_enhancedwater") != 0.0 && v("r_waterripples") != 0.0 { v("r_waterripples_amount").clamp(0.0, 4.0) } else { 0.0 },
            water_splashes: v("r_enhancedwater") != 0.0 && v("r_watersplash") != 0.0,
            water_caustics: if v("r_enhancedwater") != 0.0 && v("r_watercaustics") != 0.0 { v("r_watercaustics_amount").clamp(0.0, 4.0) } else { 0.0 },
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
        let frame = qrender::RenderFrame {
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
            player: self.cl.entities.get(self.cl.viewentity).filter(|_| self.cl.viewentity > 0).map(|e| e.origin),
        };
        if let Err(e) = self.r.render(&frame) {
            self.con.print(&format!("render error: {e:?}\n"));
        }
    }
}
