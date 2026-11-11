//! Edict storage and ED_Alloc/ED_Free. Port of pr_edict.c.
//!
//! Each edict is `entityfields` contiguous `u32` slots in one dense array;
//! QC entity values are edict numbers. Liveness/free times come from
//! [`qecs::EntityAllocator`].

use crate::{EType, Result, Vm, VmError};

impl Vm {
    /// Slots per edict.
    pub fn entityfields(&self) -> usize {
        self.progs.entityfields
    }
    /// sv.num_edicts (high-water mark).
    pub fn num_edicts(&self) -> usize {
        self.alloc.len()
    }
    pub fn max_edicts(&self) -> usize {
        self.max_edicts
    }
    /// True if `e` was freed (or never allocated).
    pub fn is_free(&self, e: u32) -> bool {
        self.alloc.get(e).is_none()
    }
    /// Time `e` was freed.
    pub fn free_time(&self, e: u32) -> Option<f32> {
        self.alloc.free_time(e)
    }
    /// Generational handle for a live edict.
    pub fn entity(&self, e: u32) -> Option<qecs::Entity> {
        self.alloc.get(e)
    }

    /// Reset all edicts: world and client slots (`1..=maxclients`) are allocated and zeroed.
    pub fn clear_edicts(&mut self) {
        self.alloc.clear();
        self.fields.clear();
        for i in 0..self.reserved as u32 {
            self.alloc.alloc_at(i);
        }
        self.fields.resize(self.reserved * self.progs.entityfields, 0);
    }

    fn sync_fields(&mut self) {
        let n = self.alloc.len() * self.progs.entityfields;
        if self.fields.len() < n {
            self.fields.resize(n, 0);
        }
    }

    /// ED_Alloc: reuse a free index `>= maxclients+1` freed over 0.5s ago (or
    /// within the first 2s of the level), otherwise grow.
    pub fn ed_alloc(&mut self) -> Result<u32> {
        let (now, min, max) = (self.time as f32, self.reserved as u32, self.max_edicts);
        let ok = move |i: u32, t: f32| i >= min && (t < 2.0 || now - t > 0.5);
        let len = self.alloc.len();
        if len >= max && !(min..len as u32).any(|i| self.alloc.free_time(i).is_some_and(|t| ok(i, t))) {
            return Err(VmError::new(format!("ED_Alloc: no free edicts (max_edicts is {max})")));
        }
        let e = self.alloc.alloc_with(ok);
        self.sync_fields();
        self.edict_mut(e.index()).fill(0);
        Ok(e.index())
    }

    /// Allocate edict `e` exactly (savegame loading); zeroes its fields.
    pub fn ed_alloc_at(&mut self, e: u32) -> Result<()> {
        if e as usize >= self.max_edicts {
            return Err(VmError::new(format!("ED_Alloc: edict {e} beyond max_edicts")));
        }
        self.alloc.alloc_at(e);
        self.sync_fields();
        self.edict_mut(e).fill(0);
        Ok(())
    }

    /// ED_Free: marks free and resets the fields the references reset. The
    /// host must unlink the edict from the world first.
    pub fn ed_free(&mut self, e: u32) {
        let Some(h) = self.alloc.get(e) else { return };
        for n in ["model", "takedamage", "modelindex", "colormap", "skin", "frame", "solid"] {
            if let Some(o) = self.field_ofs(n) {
                self.set_ent_i(e, o, 0);
            }
        }
        for n in ["origin", "angles"] {
            if let Some(o) = self.field_ofs(n) {
                self.set_ent_v(e, o, [0.0; 3]);
            }
        }
        let o = self.sys.f_nextthink;
        self.set_ent_f(e, o, -1.0);
        self.alloc.free(h, self.time as f32);
    }

    /// All field slots of an edict.
    pub fn edict(&self, e: u32) -> &[u32] {
        let s = self.progs.entityfields;
        &self.fields[e as usize * s..(e as usize + 1) * s]
    }
    pub fn edict_mut(&mut self, e: u32) -> &mut [u32] {
        let s = self.progs.entityfields;
        &mut self.fields[e as usize * s..(e as usize + 1) * s]
    }
    #[inline]
    pub fn ent_i(&self, e: u32, ofs: usize) -> i32 {
        self.fields[e as usize * self.progs.entityfields + ofs] as i32
    }
    #[inline]
    pub fn ent_f(&self, e: u32, ofs: usize) -> f32 {
        f32::from_bits(self.fields[e as usize * self.progs.entityfields + ofs])
    }
    #[inline]
    pub fn ent_v(&self, e: u32, ofs: usize) -> [f32; 3] {
        let p = e as usize * self.progs.entityfields + ofs;
        [0, 1, 2].map(|i| f32::from_bits(self.fields[p + i]))
    }
    #[inline]
    pub fn set_ent_i(&mut self, e: u32, ofs: usize, v: i32) {
        self.fields[e as usize * self.progs.entityfields + ofs] = v as u32;
    }
    #[inline]
    pub fn set_ent_f(&mut self, e: u32, ofs: usize, v: f32) {
        self.fields[e as usize * self.progs.entityfields + ofs] = v.to_bits();
    }
    #[inline]
    pub fn set_ent_v(&mut self, e: u32, ofs: usize, v: [f32; 3]) {
        let p = e as usize * self.progs.entityfields + ofs;
        for (i, x) in v.into_iter().enumerate() {
            self.fields[p + i] = x.to_bits();
        }
    }
    /// String field as bytes.
    pub fn ent_str(&self, e: u32, ofs: usize) -> Result<&[u8]> {
        self.strings.get(self.ent_i(e, ofs))
    }
    pub fn classname(&self, e: u32) -> String {
        String::from_utf8_lossy(self.ent_str(e, self.sys.f_classname).unwrap_or(b"")).into_owned()
    }

    /// PR_UglyValueString-style formatting of a typed value.
    pub fn value_string(&self, ty: EType, v: &[u32]) -> String {
        let f = |i: usize| f32::from_bits(v[i]);
        match ty {
            EType::String => String::from_utf8_lossy(self.strings.get(v[0] as i32).unwrap_or(b"")).into_owned(),
            EType::Entity => format!("entity {}", v[0] as i32),
            EType::Function => format!("{}()", self.progs.function_name(v[0] as usize)),
            EType::Field => {
                let n = self.progs.field_at_ofs(v[0] as usize).map(|d| d.name).unwrap_or(0);
                format!(".{}", String::from_utf8_lossy(self.progs.string(n)))
            }
            EType::Void => "void".into(),
            EType::Float => fmt_g(f(0)),
            EType::Vector => format!("'{:5.1} {:5.1} {:5.1}'", f(0), f(1), f(2)),
            EType::Pointer => format!("pointer {}", v[0] as i32),
            EType::Other(t) => format!("bad type {t}"),
        }
    }

    /// ED_Print: non-zero fields, one `name value` per line.
    pub fn ed_print(&self, e: u32) -> String {
        if self.is_free(e) {
            return "FREE\n".into();
        }
        let mut out = format!("\nEDICT {e}:\n");
        let ed = self.edict(e);
        for d in &self.progs.fielddefs {
            let name = self.progs.string(d.name);
            if name.len() > 1 && name[name.len() - 2] == b'_' {
                continue; // skip _x, _y, _z vars
            }
            let ty = d.etype();
            let o = d.ofs as usize;
            let v = &ed[o..o + ty.size()];
            if v.iter().all(|&x| x == 0) {
                continue;
            }
            out += &format!("{:15} {}\n", String::from_utf8_lossy(name), self.value_string(ty, v));
        }
        out
    }
}

/// `%5.1f` for non-integers, `%d`-like for integral values (close to `%g` usage in ED_Print).
pub(crate) fn fmt_g(v: f32) -> String {
    if v == (v as i32) as f32 { format!("{}", v as i32) } else { format!("{v}") }
}
