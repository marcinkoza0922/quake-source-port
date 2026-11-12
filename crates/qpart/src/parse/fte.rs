use super::*;

impl Registry {
    pub(crate) fn parse_fte(&mut self, config: &str, mut weak: bool, text: &str) {
        let mut ns = config.to_string();
        let mut lines = Lines { rest: text };
        let mut pending: Option<&str> = None;
        loop {
            let line = match pending.take().or_else(|| lines.next()) {
                Some(l) => l,
                None => return,
            };
            let args = Args::tokenize(line);
            let var = args.argv(0);
            match var {
                "r_effect" | "r_trail" => {
                    self.associate_effect(&args);
                    continue;
                }
                "r_partredirect" => {
                    self.redirect(&args);
                    continue;
                }
                "r_part" => {}
                _ => {
                    if !var.is_empty() {
                        self.msg(format!("Unknown particle command \"{var}\""));
                    }
                    continue;
                }
            }
            if args.argc() != 2 {
                if args.argv(1) == "namespace" {
                    ns = args.argv(2).to_string();
                    if args.argc() >= 4 {
                        weak = atoi(args.argv(3)) != 0;
                    }
                } else {
                    self.msg("No name for particle effect".into());
                }
                continue;
            }
            let Some(brace) = lines.next() else { return };
            if !trim_ws(brace).starts_with('{') {
                self.msg("This is a multiline command and should be used within config files".into());
                pending = Some(brace);
                continue;
            }
            let raw = args.argv(1);
            let plus = raw.starts_with('+');
            let mut id = self.allocate(&ns, raw.strip_prefix('+').unwrap_or(raw));

            if weak && self.types[id].loaded == Loaded::Strong {
                let mut depth = 1;
                loop {
                    let Some(l) = lines.next() else { return };
                    let l = trim_ws(l);
                    if l.starts_with('{') {
                        depth += 1;
                    } else if l.starts_with('}') {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                }
                continue;
            }

            if plus {
                if self.types[id].is_loaded() {
                    let mut ok = false;
                    for i in 0..64 {
                        let parent = id;
                        let newname = format!("+{i}{}", self.types[id].name);
                        id = self.allocate(&ns, &newname);
                        if !self.types[id].is_loaded() {
                            if self.types[parent].assoc.is_some() {
                                self.msg(format!("warning: assoc on particle chain {} overridden", &raw[1..]));
                            }
                            self.types[parent].assoc = Some(id);
                            ok = true;
                            break;
                        }
                    }
                    if !ok {
                        self.msg("Too many duplicate names, gave up".into());
                        return;
                    }
                }
            } else if self.types[id].is_loaded() {
                let mut assoc = self.types[id].assoc;
                let mut guard = 0;
                while let Some(a) = assoc {
                    if !self.types[a].name.starts_with('+') || guard > self.types.len() {
                        break;
                    }
                    self.types[a].loaded = Loaded::No;
                    assoc = self.types[a].assoc;
                    guard += 1;
                }
            }

            self.types[id].reset_to_defaults();
            let mut st = BodyState::default();
            loop {
                let Some(l) = lines.next() else {
                    let n = self.types[id].name.clone();
                    self.msg(format!("Unexpected end of buffer with effect {n}"));
                    return;
                };
                let mut buf = trim_ws(l);
                // `shader` may swallow the next line; loop to reparse it.
                loop {
                    if buf.starts_with('}') {
                        break;
                    }
                    match self.parse_field(id, &ns, buf, &mut lines, &mut st) {
                        Some(next) => buf = trim_ws(next),
                        None => break,
                    }
                }
                if buf.starts_with('}') {
                    break;
                }
            }
            self.finish_fte(id, weak, &st);
        }
    }

    pub(super) fn finish_fte(&mut self, id: TypeId, weak: bool, st: &BodyState) {
        let opts = self.opts;
        let mut warn = Vec::new();
        let t = &mut self.types[id];
        t.loaded = if weak { Loaded::Weak } else { Loaded::Strong };
        if t.clipcount < 1.0 {
            t.clipcount = 1.0;
        }
        if !st.settype {
            if t.looks.kind == RenderType::Normal && t.texname.is_empty() {
                t.looks.kind = if t.scale != 0.0 { RenderType::SparkFan } else { RenderType::Spark };
            } else if t.looks.kind == RenderType::Spark {
                if !t.texname.is_empty() {
                    t.looks.kind = RenderType::TexturedSpark;
                } else if t.scale != 0.0 {
                    t.looks.kind = RenderType::SparkFan;
                }
            }
        }
        if !st.setalphadelta {
            t.alphachange = (-t.alphachange / t.die) * t.alpha;
        }
        t.finish(&opts, &mut warn);
        if t.looks.kind == RenderType::Beam && !st.setbeamlen {
            t.rotationstartmin = 1.0 / 128.0;
        }
        for s in &t.sounds {
            if !s.name.is_empty() && !self.sounds_to_precache.contains(&s.name) {
                self.sounds_to_precache.push(s.name.clone());
            }
        }
        self.messages.extend(warn);
        self.dirty = true;
    }
}
