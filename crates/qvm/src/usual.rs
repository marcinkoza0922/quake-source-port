//! What a progs usually passes for a function's later arguments.

use crate::exec::Vm;

const OP_STORE: std::ops::RangeInclusive<u16> = 31..=36;
const OP_CALL: std::ops::RangeInclusive<u16> = 51..=59;

impl Vm {
    /// For each argument of `fnum` from the fifth on: the constant more than
    /// half of the progs' own calls pass, else 0. Mods gave `T_Damage` and
    /// `T_RadiusDamage` such arguments (Arcane Dimensions: check armour, for
    /// instance), each with its own meaning.
    pub fn usual_late_args(&mut self, fnum: usize) -> [u32; 4] {
        if let Some((_, args)) = self.usual.iter().find(|u| u.0 == fnum) {
            return *args;
        }
        let args = self.find_usual_args(fnum);
        self.usual.push((fnum, args));
        args
    }

    fn find_usual_args(&self, fnum: usize) -> [u32; 4] {
        let p = &self.progs;
        let count = p.functions.get(fnum).map_or(0, |f| f.numparms.clamp(0, 8) as usize);
        let is_fn = |g: u16| p.globals.get(g as usize) == Some(&(fnum as u32));
        let mut seen: [Vec<(Option<u32>, u32)>; 4] = Default::default();
        for (i, call) in p.statements.iter().enumerate().filter(|(_, s)| OP_CALL.contains(&s.op) && is_fn(s.a)) {
            let passed = usize::from(call.op - OP_CALL.start());
            for k in 4..count.min(passed) {
                let parm = crate::ofs_parm(k) as u16;
                let before = p.statements[..i].iter().rev().take_while(|s| !OP_CALL.contains(&s.op)).take(40);
                let value = before.filter(|s| OP_STORE.contains(&s.op) && s.b == parm).map(|s| p.globals.get(s.a as usize).copied()).next().flatten();
                let seen = &mut seen[k - 4];
                match seen.iter_mut().find(|v| v.0 == value) {
                    Some(v) => v.1 += 1,
                    None => seen.push((value, 1)),
                }
            }
        }
        seen.map(|values| {
            let total: u32 = values.iter().map(|v| v.1).sum();
            values.iter().find(|v| v.1 * 2 > total).and_then(|v| v.0).unwrap_or(0)
        })
    }
}
