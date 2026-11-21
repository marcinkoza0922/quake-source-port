use super::*;

impl Host {
    /// Apply the display after config.cfg, including desktop command-line overrides.
    pub(super) fn startup_video(&mut self) {
        #[cfg(not(target_os = "android"))]
        self.apply_video_args();
        self.vid_apply();
    }

    #[cfg(not(target_os = "android"))]
    fn apply_video_args(&mut self) {
        let arg = |n: &str| self.cmdline.iter().position(|a| a == n).and_then(|i| self.cmdline.get(i + 1)).cloned();
        let has = |n: &str| self.cmdline.iter().any(|a| a == n);
        let cv = &mut self.server.core.cvars;
        if let Some(w) = arg("-width") {
            cv.set("vid_width", &w);
        }
        if let Some(h) = arg("-height") {
            cv.set("vid_height", &h);
        }
        if has("-window") {
            cv.set("vid_fullscreen", "0");
        } else if has("-fullscreen") {
            cv.set("vid_fullscreen", "1");
        }
    }

    /// Applies vid_fullscreen / vid_width / vid_height to the window.
    pub fn vid_apply(&mut self) {
        let Some(win) = &self.window else { return };
        #[cfg(target_os = "android")]
        let result = {
            self.server.core.cvars.set("vid_fullscreen", "2");
            win.set_fullscreen(true)
        };
        #[cfg(not(target_os = "android"))]
        let result = {
            let mode = match self.cv("vid_fullscreen") as i32 {
                1 => qplatform::DisplayMode::Fullscreen,
                2 => qplatform::DisplayMode::Borderless,
                _ => qplatform::DisplayMode::Windowed,
            };
            let size = (self.cv("vid_width").max(320.0) as u32, self.cv("vid_height").max(200.0) as u32);
            win.set_display(mode, size)
        };
        if let Err(e) = result {
            self.con.print(&format!("vid_apply: {e:?}\n"));
            return;
        }
        let (w, h) = win.drawable_size();
        self.r.resize(w, h);
    }

    /// Fills the Graphics page's pending display state from the cvars.
    pub(super) fn vid_menu(&mut self) {
        #[cfg(not(target_os = "android"))]
        self.sync_display_choices();
        self.menu.max_msaa = self.r.max_msaa();
    }

    #[cfg(not(target_os = "android"))]
    fn sync_display_choices(&mut self) {
        let mut res = self.window.as_ref().map(|w| w.display_modes()).unwrap_or_default();
        let cur = (self.cv("vid_width") as u32, self.cv("vid_height") as u32);
        if !res.contains(&cur) {
            res.push(cur);
            res.sort_by_key(|r| std::cmp::Reverse(r.0 * r.1));
        }
        let m = &mut self.menu;
        m.resolutions = res;
        m.pending_res = cur;
        m.pending_mode = (self.server.core.cvars.value_of("vid_fullscreen") as usize).min(2);
    }
}
