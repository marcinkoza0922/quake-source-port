//! CSQC 2D drawing, pic cache, stat and `read*` packet-reader extension builtins.

use qvm::{Result, Vm};

use super::super::draw;
use super::super::markup;
use super::{CsqcHost, b_arg, p, s_arg};

impl CsqcHost<'_, '_> {
    /// Handles the 2D/pic/stat/packet builtins; returns `false` if `name` isn't one of them.
    pub(super) fn ext_draw_stat_net(&mut self, vm: &mut Vm, name: &str) -> Result<bool> {
        let sp = self.draw_space();
        match name {
            // ---- 2D Pictures & Drawing ----
            "precache_pic" => {
                let path = s_arg(vm, 0)?;
                let h = vm.g_i(p(0));
                if self.state.pics.precache(self.env.fs, self.env.r, &path).is_some() {
                    vm.ret_i(h);
                } else {
                    vm.ret_i(0);
                }
            }
            "iscachedpic" => {
                let path = s_arg(vm, 0)?;
                vm.ret_f(if self.state.pics.is_cached(&path) { 1.0 } else { 0.0 });
            }
            "freepic" => {
                let path = s_arg(vm, 0)?;
                self.state.pics.free(&path);
            }
            "drawgetimagesize" => {
                let path = s_arg(vm, 0)?;
                if let Some((_, w, h)) = self.state.pics.precache(self.env.fs, self.env.r, &path) {
                    vm.ret_v([w as f32, h as f32, 0.0]);
                } else {
                    vm.ret_v([0.0; 3]);
                }
            }
            "drawfill" => {
                let (pos, sz, rgb, alpha) = (vm.g_v(p(0)), vm.g_v(p(1)), vm.g_v(p(2)), vm.g_f(p(3)));
                if let Some(c) = &mut self.env.canvas {
                    draw::draw_fill(&mut c.list, sp, [pos[0], pos[1]], [sz[0], sz[1]], rgb, alpha);
                }
                vm.ret_f(1.0);
            }
            "drawpic" => {
                let (pos, path, sz, rgb, alpha) = (vm.g_v(p(0)), s_arg(vm, 1)?, vm.g_v(p(2)), vm.g_v(p(3)), vm.g_f(p(4)));
                if let Some((pic, pw, ph)) = self.state.pics.precache(self.env.fs, self.env.r, &path) {
                    let sz = [
                        if sz[0] == 0.0 { pw as f32 } else { sz[0] },
                        if sz[1] == 0.0 { ph as f32 } else { sz[1] },
                    ];
                    if let Some(c) = &mut self.env.canvas {
                        draw::draw_pic(&mut c.list, sp, [pos[0], pos[1]], sz, pic, [0.0, 0.0, 1.0, 1.0], [rgb[0], rgb[1], rgb[2], alpha]);
                    }
                    vm.ret_f(1.0);
                } else {
                    vm.ret_f(0.0);
                }
            }
            "drawsubpic" => {
                let (pos, sz, path, srcpos, srcsz, rgb, alpha) = (
                    vm.g_v(p(0)), vm.g_v(p(1)), s_arg(vm, 2)?, vm.g_v(p(3)), vm.g_v(p(4)), vm.g_v(p(5)), vm.g_f(p(6)),
                );
                if let Some((pic, _, _)) = self.state.pics.precache(self.env.fs, self.env.r, &path) {
                    let uv = [srcpos[0], srcpos[1], srcpos[0] + srcsz[0], srcpos[1] + srcsz[1]];
                    if let Some(c) = &mut self.env.canvas {
                        draw::draw_pic(&mut c.list, sp, [pos[0], pos[1]], [sz[0], sz[1]], pic, uv, [rgb[0], rgb[1], rgb[2], alpha]);
                    }
                    vm.ret_f(1.0);
                } else {
                    vm.ret_f(0.0);
                }
            }
            "drawline" => {
                let (w, p0, p1, rgb, alpha) = (vm.g_f(p(0)), vm.g_v(p(1)), vm.g_v(p(2)), vm.g_v(p(3)), vm.g_f(p(4)));
                if let Some(c) = &mut self.env.canvas {
                    draw::draw_line(&mut c.list, sp, w, [p0[0], p0[1]], [p1[0], p1[1]], rgb, alpha);
                }
            }
            "drawcharacter" => {
                let (pos, ch, scale, rgb, alpha) = (vm.g_v(p(0)), vm.g_f(p(1)) as u32 as u8, vm.g_v(p(2)), vm.g_v(p(3)), vm.g_f(p(4)));
                if let Some(c) = &mut self.env.canvas {
                    draw::draw_char(&mut c.list, sp, [pos[0], pos[1]], ch, [scale[0], scale[1]], rgb, alpha);
                }
                vm.ret_f(1.0);
            }
            "drawrawstring" => {
                let (pos, text, scale, rgb, alpha) = (vm.g_v(p(0)), b_arg(vm, 1)?, vm.g_v(p(2)), vm.g_v(p(3)), vm.g_f(p(4)));
                if let Some(c) = &mut self.env.canvas {
                    draw::draw_string(&mut c.list, sp, [pos[0], pos[1]], &text, [scale[0], scale[1]], [rgb[0], rgb[1], rgb[2], alpha], false);
                }
                vm.ret_f(1.0);
            }
            "drawstring" | "drawcolorcodedstring" => {
                let (pos, text, scale, rgb, alpha) = if vm.argc >= 5 {
                    (vm.g_v(p(0)), b_arg(vm, 1)?, vm.g_v(p(2)), vm.g_v(p(3)), vm.g_f(p(4)))
                } else {
                    // 4-arg drawcolorcodedstring(pos, text, scale, alpha, flag)
                    (vm.g_v(p(0)), b_arg(vm, 1)?, vm.g_v(p(2)), [1.0; 3], vm.g_f(p(3)))
                };
                if let Some(c) = &mut self.env.canvas {
                    draw::draw_string(&mut c.list, sp, [pos[0], pos[1]], &text, [scale[0], scale[1]], [rgb[0], rgb[1], rgb[2], alpha], true);
                }
                vm.ret_f(1.0);
            }
            "stringwidth" => {
                let text = b_arg(vm, 0)?;
                let handle_colors = vm.g_f(p(1)) != 0.0;
                let char_w = if vm.argc >= 3 { vm.g_v(p(2))[0] } else { 8.0 };
                let char_w = if char_w > 0.0 { char_w } else { 8.0 };
                vm.ret_f(markup::visible_len(&text, handle_colors) as f32 * char_w);
            }
            "drawsetcliparea" => {
                let (x, y, w, h) = (vm.g_f(p(0)), vm.g_f(p(1)), vm.g_f(p(2)), vm.g_f(p(3)));
                if let Some(c) = &mut self.env.canvas {
                    draw::set_clip(&mut c.list, sp, x, y, w, h);
                }
            }
            "drawresetcliparea" => {
                if let Some(c) = &mut self.env.canvas {
                    draw::reset_clip(&mut c.list);
                }
            }

            // ---- Stats ----
            "getstati" | "getstatbits" => {
                let idx = vm.g_f(p(0)) as usize;
                let mut v = self.env.cl.stats.get(idx).copied().unwrap_or(0) as u32;
                if vm.argc >= 2 {
                    let first = (vm.g_f(p(1)) as u32).min(31);
                    let count = if vm.argc >= 3 { (vm.g_f(p(2)) as u32).clamp(1, 32) } else { 1 };
                    let mask = if count >= 32 { u32::MAX } else { (1u32 << count) - 1 };
                    v = (v >> first) & mask;
                }
                vm.ret_f(v as i32 as f32);
            }
            "getstatf" => {
                let idx = vm.g_f(p(0)) as usize;
                let v = self.env.cl.statsf.get(idx).copied().unwrap_or(0.0);
                vm.ret_f(v);
            }
            "getstats" => {
                let idx = vm.g_f(p(0)) as usize;
                let s = self.env.cl.statss.get(idx).map_or("", String::as_str).as_bytes().to_vec();
                vm.ret_temp(&s);
            }

            // ---- Packet Reader (CSQC_Parse_Event) ----
            "readbyte" => vm.ret_f(self.state.packet.read_byte() as f32),
            "readchar" => vm.ret_f(self.state.packet.read_char() as f32),
            "readshort" => vm.ret_f(self.state.packet.read_short() as f32),
            "readlong" => vm.ret_f(self.state.packet.read_long() as f32),
            "readcoord" => vm.ret_f(self.state.packet.read_coord()),
            "readangle" => vm.ret_f(self.state.packet.read_angle()),
            "readfloat" => vm.ret_f(self.state.packet.read_float()),
            "readentitynum" => vm.ret_f(self.state.packet.read_entity_num() as f32),
            "readstring" => {
                let s = self.state.packet.read_string();
                vm.ret_temp(&s);
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
