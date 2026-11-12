use super::*;

impl Host {
    /// CL_GetMessage for demos: feed recorded messages until the client
    /// clock catches up with the newest one (everything before signon).
    pub(super) fn read_demo(&mut self, dt: f64) {
        let mut ended = false;
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
                        td.1 = std::time::Instant::now();
                    }
                } else if self.cl.time + dt <= self.cl.mtime[0] {
                    break;
                }
            }
            n += 1;
            let Some((angles, data)) = self.demo.as_mut().and_then(|d| d.next_message()).map(|m| (m.angles, m.data.to_vec())) else {
                ended = true;
                break;
            };
            self.cl.mviewangles[1] = self.cl.mviewangles[0];
            self.cl.mviewangles[0] = angles;
            let r = self.cl.parse_server_message(&data);
            self.cl.take_reliable_out();
            if let Err(e) = r {
                self.con.print(&format!("demo: bad message: {e:?}\n"));
                ended = true;
                break;
            }
            // NewMap must load models before entities parsed later use them.
            self.process_client_events();
            if self.demo.is_none() {
                return;
            }
        }
        if ended {
            self.end_demo();
        }
    }

    /// CL_StopPlayback + CL_NextDemo.
    pub(super) fn end_demo(&mut self) {
        self.demo = None;
        self.finish_timedemo();
        self.cl.demoplayback = false;
        if !self.next_demo() {
            self.disconnect_client();
        }
    }

    /// CL_NextDemo: queue the next demo of the loop; false when there is none.
    pub(super) fn next_demo(&mut self) -> bool {
        if self.demonum < 0 {
            return false;
        }
        if self.demonum as usize >= self.demos.len() {
            self.demonum = 0;
            if self.demos.is_empty() {
                self.con.print("No demos listed with startdemos\n");
                self.demonum = -1;
                return false;
            }
        }
        let name = self.demos[self.demonum as usize].clone();
        self.cbuf.insert_text(&format!("playdemo {name}\n"));
        self.demonum += 1;
        true
    }

    /// CL_PlayDemo_f: drop any game and start reading `name`(.dem).
    pub(super) fn play_demo(&mut self, name: &str) {
        self.stop_game();
        let file = if name.contains('.') { name.to_string() } else { format!("{name}.dem") };
        // Recorded demos live in the user dir, which isn't a search path.
        let data = self.server.core.fs.open(&file).or_else(|| std::fs::read(self.user_dir().join(&file)).ok());
        let Some(data) = data else {
            self.con.print(&format!("ERROR: couldn't open {file}.\n"));
            self.demonum = -1;
            return;
        };
        self.con.print(&format!("Playing demo from {file}.\n"));
        self.cl = ClientState::new();
        self.cl.demoplayback = true;
        self.demo = Some(crate::demo::Demo::new(data));
        self.con.forced = false;
    }

    /// CL_FinishTimeDemo.
    pub(super) fn finish_timedemo(&mut self) {
        let Some((frames, start)) = self.timedemo.take() else { return };
        let frames = frames.saturating_sub(1);
        let t = start.elapsed().as_secs_f64().max(1e-6);
        self.con.print(&format!("{frames} frames {t:5.1} seconds {:5.1} fps\n", frames as f64 / t));
    }

    /// True while a timedemo runs: the main loop drops its frame cap.
    pub fn timedemo_active(&self) -> bool {
        self.timedemo.is_some()
    }

    pub(super) fn start_recording(&mut self, path: &std::path::Path, track: i32) {
        match crate::demo::DemoWriter::create(path, track) {
            Ok(w) => {
                self.con.print(&format!("Recording to {}.\n", path.display()));
                self.recording = Some(w);
            }
            Err(e) => self.con.print(&format!("ERROR: couldn't create {}: {e}\n", path.display())),
        }
    }

    /// CL_Stop_f: end with svc_disconnect.
    pub(super) fn stop_recording(&mut self) {
        let Some(mut w) = self.recording.take() else { return };
        let r = w.write(self.cl.viewangles, &[qnet::protocol::SVC_DISCONNECT]).and_then(|_| w.finish());
        self.con.print(&match r {
            Ok(()) => "Completed demo\n".to_string(),
            Err(e) => format!("demo write failed: {e}\n"),
        });
    }

    /// CL_Record_f (with ProQuake's mid-game start).
    pub(super) fn record(&mut self, a: &Args) {
        use qnet::protocol::*;
        if self.demo.is_some() {
            self.print("Can't record during demo playback\n");
            return;
        }
        if self.recording.is_some() {
            self.stop_recording();
        }
        let c = a.argc();
        if !(2..=4).contains(&c) {
            self.print("record <demoname> [<map> [cd track]]\n");
            return;
        }
        let name = a.argv(1);
        if name.contains("..") {
            self.print("Relative pathnames are not allowed.\n");
            return;
        }
        let connected = self.local.is_some() && self.server.active();
        if c == 2 && connected && self.cl.signon < 2 {
            self.print("Can't record - try again when connected\n");
            return;
        }
        let track = if c == 4 { a.argv(3).parse().unwrap_or(-1) } else { -1 };
        let file = if name.ends_with(".dem") { name.to_string() } else { format!("{name}.dem") };
        let path = self.user_dir().join(file);
        if c > 2 {
            self.pending_record = Some((path, track));
            self.cbuf.insert_text(&format!("map {}\n", a.argv(2)));
            return;
        }
        self.start_recording(&path, track);
        if !connected {
            return;
        }
        // Replacement deltas only send changes: ask for every entity again
        // so the demo starts from a complete state.
        if self.cl.pext2 & qclient::delta::PEXT2_REPLACEMENTDELTAS != 0 {
            self.cl.ack_frames.push(-1);
        }
        let Some(mut w) = self.recording.take() else { return };
        let ang = self.cl.viewangles;
        let mut res = self.demo_head.iter().try_for_each(|m| w.write(ang, m));
        let mut b = qnet::msg::SizeBuf::new(1 << 20);
        for (i, sc) in self.cl.scores.iter().enumerate() {
            b.byte(SVC_UPDATENAME as i32);
            b.byte(i as i32);
            b.string(sc.name.as_bytes());
            b.byte(SVC_UPDATEFRAGS as i32);
            b.byte(i as i32);
            b.short(sc.frags);
            b.byte(SVC_UPDATECOLORS as i32);
            b.byte(i as i32);
            b.byte(sc.colors);
        }
        for (i, ls) in self.cl.lightstyles.iter().enumerate() {
            b.byte(SVC_LIGHTSTYLE as i32);
            b.byte(i as i32);
            b.string(ls.as_bytes());
        }
        for (i, &v) in self.cl.stats.iter().enumerate() {
            if v == 0 {
                continue;
            }
            if b.len() > 4096 {
                res = res.and_then(|_| w.write(ang, &b.data));
                b.clear();
            }
            if i < 32 {
                b.byte(SVC_UPDATESTAT as i32);
                b.byte(i as i32);
                b.long(v);
            } else {
                b.byte(SVC_STUFFTEXT as i32);
                b.string(format!("//st {i} {v}\n").as_bytes());
            }
        }
        b.byte(SVC_SETVIEW as i32);
        b.short(self.cl.viewentity as i32);
        if self.demo_head_signon < 3 {
            b.byte(SVC_SIGNONNUM as i32);
            b.byte(3);
        }
        res = res.and_then(|_| w.write(ang, &b.data));
        match res {
            Ok(()) => self.recording = Some(w),
            Err(e) => self.con.print(&format!("demo write failed: {e}\n")),
        }
    }
}
