use super::*;

impl Host {
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
                self.con.print(&qcore::ctr!("No demos listed with startdemos\n"));
                self.demonum = -1;
                return false;
            }
        }
        let name = self.demos[self.demonum as usize].clone();
        self.cbuf.insert_text(&format!("_playdemo_loop {name}\n"));
        self.demonum += 1;
        true
    }

    /// CL_PlayDemo_f: drop any game and start reading `name`(.dem/.qwd/.mvd).
    pub(super) fn play_demo(&mut self, name: &str) {
        self.stop_game();
        let candidates: Vec<String> = if name.contains('.') {
            vec![name.to_string()]
        } else {
            vec![format!("{name}.dem"), format!("{name}.qwd"), format!("{name}.mvd")]
        };
        let mut found = None;
        for c in &candidates {
            if let Some(d) = self.server.core.fs.open(c).or_else(|| qcore::vfs::read(&self.user_dir().join(c)).ok()) {
                found = Some((c.clone(), d));
                break;
            }
        }
        let Some((file, data)) = found else {
            self.con.print(&qcore::ctr!("ERROR: couldn't open {}.\n", candidates[0]));
            self.demonum = -1;
            return;
        };
        self.con.print(&qcore::ctr!("Playing demo from {}.\n", file));
        self.cl = ClientState::new();
        self.cl.demoplayback = true;
        let mut demo = crate::demo::Demo::new(data);
        demo.play.name = file.clone();
        demo.play.auto = file.starts_with(&format!("{}/", crate::demo::library::AUTO_DIR));
        // The timeline scan replays the file once more without effects.
        let data = demo.data();
        if cfg!(target_arch = "wasm32") {
            demo.play.index = Some(std::sync::Arc::new(crate::demo::index::scan(&data)));
        } else {
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(crate::demo::index::scan(&data));
            });
            demo.play.index_rx = Some(rx);
        }
        self.demo = Some(demo);
        self.con.forced = false;
    }

    /// CL_FinishTimeDemo.
    pub(super) fn finish_timedemo(&mut self) {
        let Some((frames, start)) = self.timedemo.take() else { return };
        let frames = frames.saturating_sub(1);
        let t = start.elapsed().as_secs_f64().max(1e-6);
        self.con.print(&qcore::ctr!("{} frames {} seconds {} fps\n", frames, format!("{t:5.1}"), format!("{:5.1}", frames as f64 / t)));
    }

    /// True while a timedemo runs: the main loop drops its frame cap.
    pub fn timedemo_active(&self) -> bool {
        self.timedemo.is_some()
    }

    pub(super) fn start_recording(&mut self, path: &std::path::Path, track: i32) {
        match crate::demo::DemoWriter::create(path, track) {
            Ok(w) => {
                self.con.print(&qcore::ctr!("Recording to {}.\n", path.display()));
                self.recording = Some(w);
            }
            Err(e) => self.con.print(&qcore::ctr!("ERROR: couldn't create {}: {}\n", path.display(), e)),
        }
    }

    /// CL_Stop_f: end with svc_disconnect.
    pub(super) fn stop_recording(&mut self) {
        let Some(mut w) = self.recording.take() else { return };
        if let Some(q) = &mut self.qw_client {
            q.recording_qwd = false;
        }
        let path = w.path().to_path_buf();
        let is_qwd = w.is_qwd();
        let r = if is_qwd {
            let end = qclient::qw::demo::encode_qwd_end(self.realtime as f32);
            w.write_raw(&end).and_then(|_| w.finish())
        } else {
            w.write(self.cl.viewangles, &[qnet::protocol::SVC_DISCONNECT]).and_then(|_| w.finish())
        };
        match r {
            Ok(()) => {
                self.con.print(&qcore::ctr!("Completed demo\n"));
                if !is_qwd {
                    self.convert_demo(path);
                }
            }
            Err(e) => self.con.print(&format!("demo write failed: {e}\n")),
        }
    }

    /// Rewrites a finished demo for other ports in the background
    /// (`demo::convert`); [`Self::poll_demo_jobs`] prints the outcome.
    pub(super) fn convert_demo(&mut self, path: std::path::PathBuf) {
        if cfg!(target_arch = "wasm32") {
            let msg = crate::demo::convert::convert_file(&path);
            self.con.print(&msg);
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(crate::demo::convert::convert_file(&path));
        });
        self.demo_jobs.push(rx);
    }

    pub(super) fn poll_demo_jobs(&mut self) {
        let mut done = Vec::new();
        self.demo_jobs.retain(|rx| match rx.try_recv() {
            Ok(msg) => {
                done.push(msg);
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => false,
        });
        for msg in done {
            self.con.print(&msg);
        }
    }

    /// CL_Record_f (with ProQuake's mid-game start).
    pub(super) fn record(&mut self, a: &Args) {
        if self.demo.is_some() {
            self.print(&qcore::ctr!("Can't record during demo playback\n"));
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
            self.print(&qcore::ctr!("Relative pathnames are not allowed.\n"));
            return;
        }
        let connected = (self.local.is_some() && self.server.active()) || self.remote();
        if c == 2 && connected && self.cl.signon < 2 {
            self.print(&qcore::ctr!("Can't record - try again when connected\n"));
            return;
        }
        let track = if c == 4 { a.argv(3).parse().unwrap_or(-1) } else { -1 };
        let is_qw = self.qw_client.is_some() && c == 2;
        let default_ext = if is_qw { "qwd" } else { "dem" };
        let file = if name.ends_with(".dem") || name.ends_with(".qwd") {
            name.to_string()
        } else {
            format!("{name}.{default_ext}")
        };
        let path = self.user_dir().join(file);
        if c > 2 {
            self.pending_record = Some((path, track));
            self.cbuf.insert_text(&format!("map {}\n", a.argv(2)));
            return;
        }
        self.record_at(&path, track, connected);
    }

    /// Toggle manual demo recording: stops if active, otherwise starts a new
    /// timestamped demo for the current map.
    pub(super) fn record_toggle(&mut self) {
        if self.recording.is_some() {
            self.stop_recording();
            return;
        }
        if self.demo.is_some() {
            self.print(&qcore::ctr!("Can't record during demo playback\n"));
            return;
        }
        let connected = (self.local.is_some() && self.server.active()) || self.remote();
        if !connected || self.cl.signon < 2 {
            self.print(&qcore::ctr!("Can't record - try again when connected\n"));
            return;
        }
        let map: String = self.cl.mapname.chars().map(|c| if c.is_alphanumeric() || "-_".contains(c) { c } else { '_' }).collect();
        let map_part = if map.is_empty() { "demo" } else { &map };
        let stamp = crate::demo::stamp_now();
        let dir = self.user_dir();
        let ext = if self.qw_client.is_some() { "qwd" } else { "dem" };
        let mut path = dir.join(format!("{map_part}_{stamp}.{ext}"));
        let mut n = 2u32;
        while qcore::vfs::exists(&path) {
            path = dir.join(format!("{map_part}_{stamp}_{n}.{ext}"));
            n += 1;
        }
        self.record_at(&path, -1, true);
    }

    fn record_at(&mut self, path: &std::path::Path, track: i32, connected: bool) {
        use qnet::protocol::*;
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("qwd"))
            && let Some(q) = &mut self.qw_client
        {
            let inseq = q.incoming_sequence();
            let header = q.qw.encode_qwd_header(self.realtime as f32, inseq);
            match crate::demo::DemoWriter::create_qwd(path, &header) {
                Ok(w) => {
                    q.recording_qwd = true;
                    let _ = q.take_qwd_tape();
                    self.con.print(&qcore::ctr!("Recording to {}.\n", path.display()));
                    self.recording = Some(w);
                }
                Err(e) => self.con.print(&qcore::ctr!("ERROR: couldn't create {}: {}\n", path.display(), e)),
            }
            return;
        }
        self.start_recording(path, track);
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

pub(super) fn register(c: &mut Commands<Host>) {
    c.add("startdemos", |h, a| {
        h.demos = (1..a.argc()).map(|i| a.argv(i).to_string()).collect();
        h.print(&qcore::ctr!("{} demo(s) in loop\n", h.demos.len()));
        if !h.server.active() && !h.dedicated && !h.remote() && h.demonum != -1 && h.demo.is_none() {
            h.demonum = 0;
            if h.cv("cl_startdemos") == 0.0 {
                h.demonum = -1;
                return;
            }
            h.next_demo();
        } else {
            h.demonum = -1;
        }
    });
    c.add("demos", |h, _| {
        if h.demonum == -1 {
            h.demonum = 1;
        }
        h.stop_game();
        h.next_demo();
    });
    c.add("record", |h, a| h.record(a));
    c.add("record_toggle", |h, _| h.record_toggle());
    c.add("demo_convert", |h, a| {
        if a.argc() != 2 || a.argv(1).contains("..") {
            h.print("demo_convert <demoname> : rewrite a demo so other source ports can play it\n");
            return;
        }
        let n = a.argv(1);
        let file = if n.ends_with(".dem") { n.to_string() } else { format!("{n}.dem") };
        h.convert_demo(h.user_dir().join(file));
    });
    c.add("stop", |h, _| {
        if h.recording.is_none() {
            h.print(&qcore::ctr!("Not recording a demo.\n"));
        }
        h.stop_recording();
    });
    c.add("timedemo", |h, a| {
        if a.argc() != 2 {
            h.print("timedemo <demoname> : gets demo speeds\n");
            return;
        }
        h.play_demo(a.argv(1));
        if h.demo.is_some() {
            h.timedemo = Some((0, qcore::time::Instant::now()));
        }
    });
    c.add("stopdemo", |h, _| {
        if h.demo.is_some() {
            h.disconnect_client();
        }
    });
    c.add("playdemo", |h, a| {
        if a.argc() < 2 {
            h.print("playdemo <demoname> : plays a demo\n");
            return;
        }
        // a demo the player picked ends the attract loop
        h.demonum = -1;
        h.play_demo(a.argv(1));
    });
    c.add("_playdemo_loop", |h, a| h.play_demo(a.argv(1)));
}
