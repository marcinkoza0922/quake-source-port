//! DP_QC_* builtins the VM can answer alone: string buffers, entity chains and
//! field access, tokenizing, URI escapes, dates. Ported from vkQuake `pr_ext.c`.

use super::*;

mod text;
use text::{strftime, strip_markup};

const NUM_STRBUFS: usize = 64;
const MAX_TOKENS: usize = 64;
const TEMP_MAX: usize = crate::STRINGTEMP_LENGTH - 1;

/// One `buf_create` buffer; freed slots are holes.
pub(crate) type StrBuf = Vec<Option<Vec<u8>>>;

pub(super) const TABLE: &[(&str, i32, InternalFn)] = &[
    ("randomvec", 91, randomvec),
    ("findchain", 402, |vm, _| {
        let want = vm.g_str(ofs_parm(1))?.to_vec();
        chain(vm, |vm, e, f| vm.ent_str(e, f).is_ok_and(|s| s == want))
    }),
    ("findchainfloat", 403, |vm, _| {
        let want = vm.g_f(ofs_parm(1));
        chain(vm, |vm, e, f| vm.ent_f(e, f) == want)
    }),
    ("findchainflags", 450, |vm, _| {
        let want = vm.g_f(ofs_parm(1)) as i32;
        chain(vm, |vm, e, f| vm.ent_f(e, f) as i32 & want != 0)
    }),
    ("buf_create", 460, buf_create),
    ("buf_del", 461, |vm, _| {
        if let Some(i) = buf_index(vm, 0) {
            vm.strbufs[i] = None;
        }
        Ok(())
    }),
    ("buf_getsize", 462, |vm, _| {
        let n = buf(vm, 0).map_or(0, |b| b.len());
        vm.ret_f(n as f32);
        Ok(())
    }),
    ("buf_copy", 463, |vm, _| {
        if let (Some(from), Some(to)) = (buf_index(vm, 0), buf_index(vm, 1)) {
            vm.strbufs[to] = vm.strbufs[from].clone();
        }
        Ok(())
    }),
    ("buf_sort", 464, buf_sort),
    ("buf_implode", 465, buf_implode),
    ("bufstr_get", 466, |vm, _| {
        let i = vm.g_f(ofs_parm(1));
        match buf(vm, 0).and_then(|b| b.get(i as usize)?.clone()).filter(|_| i >= 0.0) {
            Some(mut s) => {
                s.truncate(TEMP_MAX);
                vm.ret_temp(&s);
            }
            None => vm.ret_i(0),
        }
        Ok(())
    }),
    ("bufstr_set", 467, |vm, _| {
        let i = vm.g_f(ofs_parm(1));
        let s = vm.g_str(ofs_parm(2))?.to_vec();
        // a script's huge index must not take the machine's memory
        if (0.0..65536.0).contains(&i)
            && let Some(b) = buf(vm, 0)
        {
            let i = i as usize;
            if b.len() <= i {
                b.resize(i + 1, None);
            }
            b[i] = Some(s);
        }
        Ok(())
    }),
    ("bufstr_add", 468, |vm, _| {
        let s = vm.g_str(ofs_parm(1))?.to_vec();
        let ordered = vm.g_f(ofs_parm(2)) != 0.0;
        let Some(b) = buf(vm, 0) else { return Ok(()) };
        let at = if ordered { None } else { b.iter().position(Option::is_none) };
        let i = at.unwrap_or(b.len());
        if i == b.len() {
            b.push(None);
        }
        b[i] = Some(s);
        vm.ret_f(i as f32);
        Ok(())
    }),
    ("bufstr_free", 469, |vm, _| {
        let i = vm.g_f(ofs_parm(1));
        if i >= 0.0
            && let Some(s) = buf(vm, 0).and_then(|b| b.get_mut(i as usize))
        {
            *s = None;
        }
        Ok(())
    }),
    ("strlennocol", 476, |vm, _| {
        let n = strip_markup(vm.g_str(ofs_parm(0))?).len();
        vm.ret_f(n as f32);
        Ok(())
    }),
    ("strdecolorize", 477, |vm, _| {
        let mut s = strip_markup(vm.g_str(ofs_parm(0))?);
        s.truncate(TEMP_MAX);
        vm.ret_temp(&s);
        Ok(())
    }),
    ("strftime", 478, |vm, _| {
        // without a time zone database the local flag gives UTC too
        let now = qcore::time::SystemTime::now().duration_since(qcore::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let mut s = strftime(vm.g_str(ofs_parm(1))?, now as i64);
        s.truncate(TEMP_MAX);
        vm.ret_temp(&s);
        Ok(())
    }),
    ("tokenizebyseparator", 479, tokenize_by_separator),
    ("crc16", 494, |vm, _| {
        let mut s = vm.var_string(1)?;
        if vm.g_f(ofs_parm(0)) != 0.0 {
            s.make_ascii_lowercase();
        }
        vm.ret_f(f32::from(crate::crc_block(&s)));
        Ok(())
    }),
    ("numentityfields", 496, |vm, _| {
        let n = vm.progs.fielddefs.len();
        vm.ret_f(n as f32);
        Ok(())
    }),
    ("entityfieldname", 497, |vm, _| {
        let name = field_def(vm, 0).map_or(0, |d| d.name);
        vm.ret_i(name);
        Ok(())
    }),
    ("entityfieldtype", 498, |vm, _| {
        let ty = field_def(vm, 0).map_or(0, |d| d.ty & 0x7fff);
        vm.ret_f(f32::from(ty));
        Ok(())
    }),
    ("entityfieldref", 0, |vm, _| {
        // the field's value, as a field constant holds it
        let ofs = field_def(vm, 0).map_or(0, |d| i32::from(d.ofs));
        vm.ret_i(ofs);
        Ok(())
    }),
    ("findentityfield", 0, |vm, _| {
        let name = vm.g_str(ofs_parm(0))?;
        let i = vm.progs.fielddefs.iter().position(|d| vm.progs.string(d.name) == name);
        vm.ret_f(i.unwrap_or(0) as f32);
        Ok(())
    }),
    ("getentityfieldstring", 499, get_entity_field_string),
    ("putentityfieldstring", 500, |vm, _| {
        let def = field_def(vm, 0);
        let e = vm.g_i(ofs_parm(1));
        let v = qcore::text::decode(vm.g_str(ofs_parm(2))?);
        let ok = match def {
            Some(d) if e > 0 && (e as usize) < vm.num_edicts() => vm.set_field_string(e as u32, d, &v),
            _ => false,
        };
        vm.ret_f(f32::from(ok));
        Ok(())
    }),
    ("uri_escape", 510, |vm, _| {
        let mut out = Vec::new();
        for &b in vm.g_str(ofs_parm(0))? {
            if out.len() + 4 >= crate::STRINGTEMP_LENGTH {
                break;
            }
            if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_') {
                out.push(b);
            } else {
                out.extend_from_slice(format!("%{b:02X}").as_bytes());
            }
        }
        vm.ret_temp(&out);
        Ok(())
    }),
    ("uri_unescape", 511, |vm, _| {
        let s = vm.g_str(ofs_parm(0))?;
        let (mut out, mut i) = (Vec::new(), 0);
        while i < s.len() && out.len() < TEMP_MAX {
            let hex = |k: usize| s.get(i + k).and_then(|&c| (c as char).to_digit(16));
            match (s[i], hex(1), hex(2)) {
                (b'%', Some(h), Some(l)) => {
                    out.push((h * 16 + l) as u8);
                    i += 3;
                }
                (b, ..) => {
                    out.push(b);
                    i += 1;
                }
            }
        }
        vm.ret_temp(&out);
        Ok(())
    }),
    // vkQuake refuses the request too
    ("uri_get", 513, |vm, _| {
        vm.ret_f(0.0);
        Ok(())
    }),
    ("argv_start_index", 515, |vm, _| token_index(vm, |t| t.0)),
    ("argv_end_index", 516, |vm, _| token_index(vm, |t| t.1)),
    ("callfunction", 605, |vm, host| {
        if vm.argc == 0 {
            return Ok(());
        }
        vm.argc -= 1;
        let name = vm.g_string(ofs_parm(vm.argc))?;
        match vm.progs.find_function(&name) {
            Some(f) if vm.progs.functions[f].first_statement > 0 => vm.execute(host, f),
            _ => Ok(()),
        }
    }),
    ("isfunction", 607, |vm, _| {
        let name = vm.g_string(ofs_parm(0))?;
        let yes = vm.progs.find_function(&name).is_some();
        vm.ret_f(f32::from(yes));
        Ok(())
    }),
    ("parseentitydata", 613, parse_entity_data),
    ("etof", 0, ext::pf_num_for_edict),
    ("ftoe", 0, ext::pf_edict_num),
];

/// PF_randomvector: a random point inside the unit sphere.
fn randomvec(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let v = loop {
        let v: [f32; 3] = std::array::from_fn(|_| vm.rand15() as f32 * (2.0 / 32767.0) - 1.0);
        if v.iter().map(|x| x * x).sum::<f32>() < 1.0 {
            break v;
        }
    };
    vm.ret_v(v);
    Ok(())
}

/// Links every edict whose field (parm 0) passes `hit` through `.chain`, or
/// the field in parm 2.
fn chain(vm: &mut Vm, hit: impl Fn(&Vm, u32, usize) -> bool) -> Result<()> {
    let f = vm.g_i(ofs_parm(0)) as usize;
    let link = if vm.argc > 2 { vm.g_i(ofs_parm(2)) as usize } else { vm.field_ofs("chain").unwrap_or(usize::MAX) };
    let n = vm.progs.entityfields;
    let mut head = 0;
    if f < n && link < n {
        for e in 1..vm.num_edicts() as u32 {
            if !vm.is_free(e) && hit(vm, e, f) {
                vm.set_ent_i(e, link, head);
                head = e as i32;
            }
        }
    }
    vm.ret_e(head as u32);
    Ok(())
}

fn buf_index(vm: &Vm, parm: usize) -> Option<usize> {
    let i = vm.g_f(ofs_parm(parm)) as i32 - 1; // handles start at 1
    let i = usize::try_from(i).ok()?;
    vm.strbufs.get(i)?.is_some().then_some(i)
}

fn buf(vm: &mut Vm, parm: usize) -> Option<&mut StrBuf> {
    let i = buf_index(vm, parm)?;
    vm.strbufs[i].as_mut()
}

fn buf_create(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    if vm.argc > 0 && !vm.g_str(ofs_parm(0))?.eq_ignore_ascii_case(b"string") {
        vm.ret_f(-1.0);
        return Ok(());
    }
    let i = vm.strbufs.iter().position(Option::is_none).unwrap_or(vm.strbufs.len());
    if i >= NUM_STRBUFS {
        vm.ret_f(-1.0);
        return Ok(());
    }
    if i == vm.strbufs.len() {
        vm.strbufs.push(None);
    }
    vm.strbufs[i] = Some(Vec::new());
    vm.ret_f((i + 1) as f32);
    Ok(())
}

fn buf_sort(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let prefix = vm.g_f(ofs_parm(1)) as i32;
    let backwards = vm.g_f(ofs_parm(2)) != 0.0;
    let prefix = if prefix <= 0 { usize::MAX } else { prefix as usize };
    if let Some(b) = buf(vm, 0) {
        b.retain(Option::is_some);
        let key = |s: &Option<Vec<u8>>| s.as_deref().map(|s| s[..s.len().min(prefix)].to_vec());
        b.sort_by(|x, y| if backwards { key(y).cmp(&key(x)) } else { key(x).cmp(&key(y)) });
    }
    Ok(())
}

fn buf_implode(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let glue = vm.g_str(ofs_parm(1))?.to_vec();
    let Some(b) = buf(vm, 0) else {
        vm.ret_i(0);
        return Ok(());
    };
    let mut out: Vec<u8> = Vec::new();
    for s in b.iter().flatten() {
        let g: &[u8] = if out.is_empty() { b"" } else { &glue };
        if out.len() + g.len() + s.len() > TEMP_MAX {
            break;
        }
        out.extend_from_slice(g);
        out.extend_from_slice(s);
    }
    vm.ret_temp(&out);
    Ok(())
}

impl Vm {
    /// Replaces buffer `handle`'s strings (`buf_cvarlist`).
    pub fn set_strbuf(&mut self, handle: f32, strings: Vec<Vec<u8>>) {
        let i = handle as i32 - 1;
        if let Some(Some(b)) = usize::try_from(i).ok().and_then(|i| self.strbufs.get_mut(i)) {
            *b = strings.into_iter().map(Some).collect();
        }
    }
}

fn tokenize_by_separator(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let s = vm.g_str(ofs_parm(0))?.to_vec();
    let mut seps = Vec::new();
    for i in 1..vm.argc.min(8) {
        let sep = vm.g_str(ofs_parm(i))?.to_vec();
        if !sep.is_empty() {
            seps.push(sep);
        }
    }
    let (mut spans, mut start, mut i) = (Vec::new(), 0, 0);
    while !s.is_empty() && spans.len() < MAX_TOKENS {
        if i >= s.len() {
            spans.push((start, s.len()));
            break;
        }
        match seps.iter().find(|p| s[i..].starts_with(p)) {
            Some(p) => {
                spans.push((start, i));
                i += p.len();
                start = i;
            }
            None => i += 1,
        }
    }
    vm.tokens = spans.iter().map(|&(a, b)| s[a..b].to_vec()).collect();
    vm.token_spans = spans;
    vm.ret_f(vm.tokens.len() as f32);
    Ok(())
}

/// argv_start_index / argv_end_index: negative indexes count from the end.
fn token_index(vm: &mut Vm, pick: impl Fn((usize, usize)) -> usize) -> Result<()> {
    let mut i = vm.g_f(ofs_parm(0)) as i32;
    if i < 0 {
        i += vm.token_spans.len() as i32;
    }
    let v = usize::try_from(i).ok().and_then(|i| vm.token_spans.get(i)).map_or(-1.0, |&t| pick(t) as f32);
    vm.ret_f(v);
    Ok(())
}

fn field_def(vm: &Vm, parm: usize) -> Option<crate::Def> {
    let i = vm.g_f(ofs_parm(parm));
    if i < 0.0 { None } else { vm.progs.fielddefs.get(i as usize).copied() }
}

/// PR_UglyValueString of a field.
fn get_entity_field_string(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    use crate::EType;
    let e = vm.g_i(ofs_parm(1));
    let (Some(d), true) = (field_def(vm, 0), e >= 0 && (e as usize) < vm.num_edicts()) else {
        vm.ret_i(0);
        return Ok(());
    };
    let (e, o) = (e as u32, d.ofs as usize);
    let mut s = match d.etype() {
        EType::String => vm.ent_str(e, o)?.to_vec(),
        EType::Float => format!("{:.6}", vm.ent_f(e, o)).into_bytes(),
        EType::Vector => {
            let v = vm.ent_v(e, o);
            format!("{:.6} {:.6} {:.6}", v[0], v[1], v[2]).into_bytes()
        }
        EType::Entity => vm.ent_i(e, o).to_string().into_bytes(),
        EType::Function => vm.progs.function_name(vm.ent_i(e, o).max(0) as usize).into_bytes(),
        EType::Field => {
            let n = vm.progs.field_at_ofs(vm.ent_i(e, o).max(0) as usize).map_or(0, |d| d.name);
            vm.progs.string(n).to_vec()
        }
        _ => b"void".to_vec(),
    };
    s.truncate(TEMP_MAX);
    vm.ret_temp(&s);
    Ok(())
}

/// Parses `{ "k" "v" ... }` at an offset of a string into an edict; returns
/// the offset after it, 0 at the end.
fn parse_entity_data(vm: &mut Vm, host: &mut dyn Host) -> Result<()> {
    let e = vm.g_i(ofs_parm(0));
    let data = vm.g_str(ofs_parm(1))?.to_vec();
    let start = if vm.argc > 2 { (vm.g_f(ofs_parm(2)).max(0.0) as usize).min(data.len()) } else { 0 };
    vm.ret_f(0.0);
    let text = qcore::text::decode(&data[start..]);
    let Some(("{", mut rest)) = qcore::cmd::com_parse(&text) else { return Ok(()) };
    let mut pairs = Vec::new();
    loop {
        let Some((k, r)) = qcore::cmd::com_parse(rest) else {
            return Err(vm.run_error("ED_ParseEntity: EOF without closing brace"));
        };
        rest = r;
        if k == "}" {
            break;
        }
        let Some((v, r)) = qcore::cmd::com_parse(rest) else {
            return Err(vm.run_error("ED_ParseEntity: EOF without closing brace"));
        };
        rest = r;
        pairs.push((k.to_string(), v.to_string()));
    }
    let used = qcore::text::encode(&text[..text.len() - rest.len()]).len();
    if e > 0 && (e as usize) < vm.num_edicts() {
        vm.parse_edict(host, e as u32, &pairs)?;
    }
    vm.ret_f((start + used) as f32);
    Ok(())
}

#[cfg(test)]
mod tests;
