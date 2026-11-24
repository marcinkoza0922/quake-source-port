//! Demo playback controls: speed, fast-forward and rewind (ironwail's
//! keys), seeking through snapshots (`demo::keyframes`), jumps, clip marks
//! and pausing at the end. See docs/DESIGN.md "Demos".

use super::*;
use crate::demo::playback::{Playback, parse_time};
use qinput::keys::*;

/// How long the playback bar stays up after a control (s).
const BAR_SECS: f64 = 3.0;
/// PgUp/PgDn and shoulder jumps (s).
const JUMP: f64 = 10.0;

/// One block read by [`Host::demo_step`].
enum Step {
    Read,
    End,
    Error,
}

impl Host {
    /// The speed this frame: 0 paused, negative rewinding; 1 for timedemo
    /// and while exporting a clip.
    pub(super) fn demo_speed(&self) -> f32 {
        match &self.demo {
            Some(_) if self.timedemo.is_some() || self.exporting() => 1.0,
            Some(d) => d.play.speed(d.paused),
            None => 1.0,
        }
    }

    /// Running demo time of the frame being shown, once the index is ready.
    pub(super) fn demo_clock(&self) -> Option<f64> {
        let d = self.demo.as_ref()?;
        d.play.clock(self.cl.time.clamp(self.cl.mtime[1].min(self.cl.mtime[0]), self.cl.mtime[0]))
    }

    /// CL_GetMessage for demos: feed recorded messages until the client
    /// clock catches up with the newest one (everything before signon).
    pub(super) fn read_demo(&mut self, dt: f64) {
        let Some(d) = &mut self.demo else { return };
        d.play.poll_index();
        if d.play.index.is_some()
            && let Some(t) = d.play.pending_seek.take()
        {
            self.demo_seek(t);
            return;
        }
        if d.paused && self.timedemo.is_none() && d.play.scrub == 0 {
            return;
        }
        let speed = f64::from(self.demo_speed());
        if speed < 0.0 {
            if let Some(now) = self.demo_clock() {
                self.demo_seek((now + speed * dt).max(0.0));
            }
            return;
        }
        let dt = dt * speed;
        let mut n = 0;
        while self.demo.is_some() {
            if self.cl.signon == 4 {
                if let Some(td) = &mut self.timedemo {
                    // One message per frame; frame 1 (loading) doesn't count.
                    if n > 0 {
                        break;
                    }
                    td.0 += 1;
                    if td.0 == 2 {
                        td.1 = qcore::time::Instant::now();
                    }
                } else if self.cl.time + dt <= self.cl.mtime[0] {
                    break;
                }
            }
            n += 1;
            match self.demo_step() {
                Step::Read => self.demo_keyframe(),
                Step::End => return self.demo_reached_end(),
                Step::Error => return self.end_demo(),
            }
        }
    }

    /// Reads, parses and handles one block.
    fn demo_step(&mut self) -> Step {
        let mut buf = std::mem::take(&mut self.demo_msg_buf);
        let angles = {
            let Some(d) = &mut self.demo else {
                self.demo_msg_buf = buf;
                return Step::End;
            };
            let Some(m) = d.next_message() else {
                self.demo_msg_buf = buf;
                return Step::End;
            };
            buf.clear();
            buf.extend_from_slice(m.data);
            m.angles
        };
        self.cl.mviewangles[1] = self.cl.mviewangles[0];
        self.cl.mviewangles[0] = angles;
        self.cl.defer_impact_effects();
        let r = self.cl.parse_server_message(&buf);
        self.demo_msg_buf = buf;
        self.cl.take_reliable_out();
        if let Err(e) = r {
            self.con.print(&format!("demo: bad message: {e:?}\n"));
            return Step::Error;
        }
        // NewMap must load models before entities parsed later use them.
        self.process_client_events();
        if self.demo.is_none() { Step::End } else { Step::Read }
    }

    /// The last block was read (or svc_disconnect): user-started demos stay
    /// on the last frame, paused (`cl_demo_endpause`); the attract loop,
    /// timedemo and exports move on as before.
    pub(super) fn demo_reached_end(&mut self) {
        let hold = self.cv("cl_demo_endpause") != 0.0 && self.demonum < 0 && self.timedemo.is_none() && !self.exporting();
        match &mut self.demo {
            Some(d) if hold => {
                d.paused = true;
                d.play.scrub = 0;
                d.play.shown_until = f64::INFINITY;
            }
            Some(_) => self.end_demo(),
            None => {}
        }
        if self.exporting() {
            self.finish_export();
        }
    }

    /// Counts serverinfos so the clock knows which map is playing.
    pub(super) fn demo_new_map(&mut self) {
        if let Some(d) = &mut self.demo {
            d.play.ord = Some(d.play.ord.map_or(0, |o| o + 1));
        }
    }

    /// Takes a snapshot after a block when none is near this time.
    fn demo_keyframe(&mut self) {
        let Some(d) = &mut self.demo else { return };
        let Some(ord) = d.play.ord else { return };
        let t = self.cl.mtime[0];
        if self.cl.signon != 4 || self.timedemo.is_some() || d.play.keyframes.covered(ord, t) {
            return;
        }
        let state = Box::new(self.cl.demo_snapshot());
        let bytes = state.snapshot_bytes();
        d.play.keyframes.insert(crate::demo::keyframes::Keyframe { ord, time: t, pos: d.pos(), state, bytes });
    }

    /// Jumps to running demo time `target`: from the current frame when it
    /// is just ahead, else from the latest snapshot before it (or the start),
    /// then replays blocks without sound or one-shot effects.
    pub(super) fn demo_seek(&mut self, target: f64) {
        let Some(d) = &mut self.demo else { return };
        d.play.poll_index();
        let Some(idx) = d.play.index.clone() else {
            d.play.pending_seek = Some(target);
            return;
        };
        let Some((ord, mut t)) = idx.locate(target) else { return };
        let here = d.play.ord == Some(ord) && self.cl.signon == 4 && self.cl.mtime[0] <= t;
        let kf = d.play.keyframes.best(ord, t).or_else(|| d.play.keyframes.first(ord));
        let mtime = self.cl.mtime[0];
        let have_kf = kf.is_some();
        let restore = kf.filter(|k| !(here && k.time <= mtime)).map(|k| (k.time, k.pos, (*k.state).clone()));
        let reload = match restore {
            Some((kt, pos, state)) => {
                // Nothing earlier on this map: stop at its first snapshot.
                t = t.max(kt);
                d.seek(pos);
                let reload = d.play.ord != Some(ord);
                d.play.ord = Some(ord);
                self.cl.demo_restore(state);
                reload
            }
            None if have_kf || here => false,
            None => {
                d.rewind();
                d.play.ord = None;
                let (name, color) = (self.cl.name.clone(), self.cl.color);
                self.cl = ClientState::new();
                (self.cl.name, self.cl.color, self.cl.demoplayback) = (name, color, true);
                false
            }
        };
        if reload {
            self.reload_seek_level();
        }
        self.demo_seeking = true;
        let mut ended = false;
        while let Some(d) = &self.demo {
            if d.play.ord == Some(ord) && self.cl.signon == 4 && self.cl.mtime[0] >= t {
                break;
            }
            if d.at_end() {
                ended = true;
                break;
            }
            match self.demo_step() {
                Step::Read => self.demo_keyframe(),
                Step::End => {
                    ended = true;
                    break;
                }
                Step::Error => {
                    self.demo_seeking = false;
                    return self.end_demo();
                }
            }
        }
        self.demo_seeking = false;
        self.cl.time = t.min(self.cl.mtime[0]);
        self.cl.oldtime = self.cl.time;
        self.clear_seek_fx();
        if ended {
            self.demo_reached_end();
        }
    }

    /// A snapshot from another map was restored: load that map's models
    /// and sounds again and restart its static sounds.
    fn reload_seek_level(&mut self) {
        self.new_map();
        if let Some(snd) = &self.snd {
            for s in &self.cl.static_sounds {
                if let Some(Some(sfx)) = self.level.sounds.get(s.sound) {
                    snd.event(SoundEvent::Static {
                        sfx: sfx.clone(),
                        origin: s.origin,
                        volume: s.volume as f32 / 255.0,
                        attenuation: s.attenuation as f32 / 64.0,
                    });
                }
            }
        }
        if !self.cl.skybox.is_empty() {
            let sky = self.cl.skybox.clone();
            self.level.set_sky(&self.server.core.fs, &mut self.r, &sky);
        }
    }

    /// Effects that would be left half-played after a jump.
    fn clear_seek_fx(&mut self) {
        // ragdolls and add-on effects hold entity numbers of the old state
        self.debris.clear(&mut self.cl);
        self.clear_toy_fx();
        self.cl.clear_transient();
        self.fte.clear_effects(self.cl.time);
        self.blood.clear();
        self.impacts.clear();
        self.hitfx.clear();
        self.tracers.clear();
        self.view.reset_effects();
        if let Some(s) = &self.snd {
            s.event(SoundEvent::StopDynamic);
        }
    }

    /// Events skipped while a seek replays blocks.
    pub(super) fn seek_skips(e: &ClientEvent) -> bool {
        !matches!(
            e,
            ClientEvent::StuffText(_)
                | ClientEvent::ModelPrecache(_)
                | ClientEvent::SoundPrecache(_)
                | ClientEvent::StaticSound(_)
                | ClientEvent::CdTrack { .. }
                | ClientEvent::Skybox(_)
                | ClientEvent::LightStyle(_)
                | ClientEvent::Signon(_)
                | ClientEvent::NewMap
                | ClientEvent::Disconnect
                | ClientEvent::Pause(_)
        )
    }

    /// Relative or absolute jump, clamped to the demo.
    pub(super) fn demo_jump(&mut self, delta: f64) {
        if let Some(now) = self.demo_clock() {
            self.demo_seek((now + delta).max(0.0));
        }
    }

    /// Start of the next map (End key), or the last frame.
    pub(super) fn demo_next_map(&mut self) {
        let Some(d) = &self.demo else { return };
        let (Some(idx), Some(ord)) = (&d.play.index, d.play.ord) else { return };
        let t = idx.maps.get(ord + 1).map_or(idx.duration, |m| m.base);
        self.demo_seek(t);
    }

    pub(super) fn show_demo_bar(&mut self) {
        if let Some(d) = &mut self.demo
            && d.play.shown_until.is_finite()
        {
            d.play.shown_until = self.realtime + BAR_SECS;
        }
    }

    /// Demo control keys in game mode; true when the key was used. Held
    /// keys (scrub, slow) also track their release in every mode. Every
    /// control has a pad button: A pause, d-pad scrub and speed, LB/RB 10 s,
    /// LT/RT mark in/out, B clear marks, X export, Y keep, LS start, RS next
    /// level, View hide the bar.
    pub(super) fn demo_key(&mut self, key: i32, down: bool) -> bool {
        let realtime = self.realtime;
        let game = self.key_dest() == KeyDest::Game;
        // the attract loop keeps vanilla keys: any key opens the menu
        if self.exporting() || self.demonum >= 0 {
            return false;
        }
        let Some(d) = &mut self.demo else { return false };
        let is_mvd = d.is_mvd();
        let p: &mut Playback = &mut d.play;
        let held = |p: &mut Playback, key: i32, down: bool| match key {
            K_LEFTARROW | K_DPAD_LEFT => p.scrub = if down { -1 } else if p.scrub < 0 { 0 } else { p.scrub },
            K_RIGHTARROW | K_DPAD_RIGHT => p.scrub = if down { 1 } else if p.scrub > 0 { 0 } else { p.scrub },
            K_SHIFT | K_CTRL => p.slow = down,
            _ => {}
        };
        if !down {
            held(p, key, false);
            return game && matches!(key, K_LEFTARROW | K_RIGHTARROW | K_DPAD_LEFT | K_DPAD_RIGHT | K_SHIFT | K_CTRL);
        }
        if !game {
            return false;
        }
        if !p.shown_until.is_finite() {
            p.shown_until = realtime + BAR_SECS;
        }
        let k = if (b'A' as i32..=b'Z' as i32).contains(&key) { key + 32 } else { key };
        let in_freecam = self.freecam.cam.is_some();
        match k {
            K_SPACE if in_freecam => return false,
            K_SPACE | K_ABUTTON | K_PAUSE => {
                d.paused = !d.paused;
                d.play.scrub = 0;
            }
            k if k == b'p' as i32 => {
                d.paused = !d.paused;
                d.play.scrub = 0;
            }
            k if k == b'f' as i32 => {
                self.set_freecam(!in_freecam);
            }
            k if k == b']' as i32 && is_mvd => {
                self.demo_switch_pov("next");
            }
            k if k == b'[' as i32 && is_mvd => {
                self.demo_switch_pov("prev");
            }
            k if (b'1' as i32..=b'9' as i32).contains(&k) && is_mvd => {
                let s = ((k - b'0' as i32) as u8).to_string();
                self.demo_switch_pov(&s);
            }
            K_UPARROW | K_DPAD_UP => d.play.faster(&mut d.paused),
            K_DOWNARROW | K_DPAD_DOWN => d.play.slower(&mut d.paused),
            K_LEFTARROW | K_RIGHTARROW | K_DPAD_LEFT | K_DPAD_RIGHT | K_SHIFT | K_CTRL => held(p, key, true),
            K_PGUP | K_LSHOULDER => self.demo_jump(-JUMP),
            K_PGDN | K_RSHOULDER => self.demo_jump(JUMP),
            K_HOME | K_LTHUMB => self.demo_seek(0.0),
            K_END | K_RTHUMB => self.demo_next_map(),
            K_LTRIGGER => self.demo_mark(0),
            K_RTRIGGER => self.demo_mark(1),
            k if k == b'i' as i32 => self.demo_mark(0),
            k if k == b'o' as i32 => self.demo_mark(1),
            K_BACKSPACE | K_DEL | K_BBUTTON => d.play.marks = [None; 2],
            K_XBUTTON => self.cbuf.add_text("menu_export\n"),
            k if k == b'e' as i32 => self.cbuf.add_text("menu_export\n"),
            K_YBUTTON => self.cbuf.add_text("demo_keep\n"),
            k if k == b'k' as i32 => self.cbuf.add_text("demo_keep\n"),
            // no `show_demo_bar` after it: hiding must hide
            k if k == b'h' as i32 || k == K_BACK => return {
                self.toggle_demo_bar();
                true
            },
            // the wheel would open the menu (any other key does)
            K_MWHEELUP | K_MWHEELDOWN => {}
            _ => return false,
        }
        self.show_demo_bar();
        true
    }

    /// Sets clip mark `i` (0 in, 1 out) at the current time.
    pub(super) fn demo_mark(&mut self, i: usize) {
        let now = self.demo_clock();
        if let (Some(d), Some(now)) = (&mut self.demo, now) {
            d.play.marks[i] = Some(now);
        }
    }

    /// True while a clip is being rendered to video.
    pub(super) fn exporting(&self) -> bool {
        self.capture.as_ref().is_some_and(|c| c.export.is_some())
    }

    /// Switches the active player POV in a Multi-View Demo (`.mvd`) or toggles
    /// the free-roam camera (`demo_pov free`).
    pub(super) fn demo_switch_pov(&mut self, target: &str) {
        let arg = target.trim();
        if arg.eq_ignore_ascii_case("free") || arg.eq_ignore_ascii_case("roam") {
            self.set_freecam(self.freecam.cam.is_none());
            self.show_demo_bar();
            return;
        }
        let Some(d) = &self.demo else {
            self.print(&qcore::ctr!("Not playing a demo.\n"));
            return;
        };
        if !d.is_mvd() {
            self.print("demo_pov: POV switching requires an .mvd demo (use 'demo_pov free' or 'freecam' for free-roam camera)\n");
            return;
        }
        let players = d.mvd_players().to_vec();
        if players.is_empty() {
            self.print("demo_pov: no active players in demo\n");
            return;
        }
        let cur = d.mvd_pov().unwrap_or(players[0].0);
        let cur_idx = players.iter().position(|(s, _)| *s == cur).unwrap_or(0);
        let chosen = match arg.to_ascii_lowercase().as_str() {
            "" | "next" | "+" => {
                if self.freecam.cam.is_some() {
                    Some(players[cur_idx].clone())
                } else {
                    Some(players[(cur_idx + 1) % players.len()].clone())
                }
            }
            "prev" | "-" => Some(players[(cur_idx + players.len() - 1) % players.len()].clone()),
            s => {
                if let Ok(n) = s.parse::<usize>()
                    && n >= 1
                {
                    players
                        .get(n - 1)
                        .cloned()
                        .or_else(|| players.iter().find(|(slot, _)| *slot + 1 == n).cloned())
                } else {
                    players.iter().find(|(_, name)| name.to_ascii_lowercase().contains(s)).cloned()
                }
            }
        };
        let Some((slot, name)) = chosen else {
            self.print(&format!("demo_pov: player '{arg}' not found\n"));
            return;
        };
        self.set_freecam(false);
        let now = self.demo_clock().unwrap_or(0.0);
        let changed = self.demo.as_ref().is_some_and(|d| d.mvd_pov() != Some(slot));
        if changed
            && let Some(d) = &mut self.demo
            && d.set_mvd_pov(slot)
        {
            self.demo_seek(now);
        }
        self.print(&format!("POV: {name} (#{})\n", slot + 1));
        self.show_demo_bar();
    }
}

pub(super) fn register(c: &mut Commands<Host>) {
    c.add("demo_seek", |h, a| {
        // the tokenizer splits `1:30` at the colon: take the whole line
        let line: String = a.args().chars().filter(|c| !c.is_whitespace() && *c != '"').collect();
        let Some((t, sign)) = (a.argc() >= 2).then(|| parse_time(&line)).flatten() else {
            h.print("demo_seek <time> : jump to a time in the playing demo (90, 1:30, +10, -10)\n");
            return;
        };
        if h.demo.is_none() {
            return h.print(&qcore::ctr!("Not playing a demo.\n"));
        }
        if sign == 0 { h.demo_seek(t) } else { h.demo_jump(t * f64::from(sign)) }
        h.show_demo_bar();
    });
    c.add("demo_speed", |h, a| {
        let Some(d) = &mut h.demo else { return h.print(&qcore::ctr!("Not playing a demo.\n")) };
        match a.argv(1).parse::<f32>() {
            Ok(v) if a.argc() == 2 && v.is_finite() => {
                if v <= 0.0 {
                    d.paused = true;
                } else {
                    d.play.base_speed = v.clamp(crate::demo::playback::MIN_SPEED, crate::demo::playback::MAX_SPEED);
                    d.paused = false;
                }
                h.show_demo_bar();
            }
            _ => {
                let s = format!("demo_speed <speed> : playback speed, {} to {} (0 pauses); now {}\n", crate::demo::playback::MIN_SPEED, crate::demo::playback::MAX_SPEED, d.play.base_speed);
                h.print(&s);
            }
        }
    });
    c.add("demo_pause", |h, _| {
        if let Some(d) = &mut h.demo {
            d.paused = !d.paused;
            h.show_demo_bar();
        }
    });
    c.add("demo_mark_in", |h, _| {
        h.demo_mark(0);
        h.show_demo_bar();
    });
    c.add("demo_mark_out", |h, _| {
        h.demo_mark(1);
        h.show_demo_bar();
    });
    c.add("demo_mark_clear", |h, _| {
        if let Some(d) = &mut h.demo {
            d.play.marks = [None; 2];
        }
    });
    c.add("demo_pov", |h, a| {
        let arg = if a.argc() >= 2 { a.argv(1) } else { "next" };
        h.demo_switch_pov(arg);
    });
    c.add("track", |h, a| {
        let arg = if a.argc() >= 2 { a.argv(1) } else { "next" };
        h.demo_switch_pov(arg);
    });
}
