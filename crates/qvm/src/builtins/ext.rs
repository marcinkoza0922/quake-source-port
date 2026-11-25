//! Extended VM-internal builtins (FTE_STRINGS, DP_QC_*, EXT_BITSHIFT,
//! FTE_QC_INTCONV, DP_QC_ENTITYDATA/FINDFLOAT/COPYENTITY/NUM_FOR_EDICT).
//! Ported from vkQuake `pr_ext.c`.

use super::*;

pub(super) fn pf_strstrofs(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let s = vm.g_str(ofs_parm(0))?.to_vec();
    let sub = vm.g_str(ofs_parm(1))?.to_vec();
    let first = if vm.argc > 2 { vm.g_f(ofs_parm(2)) as i32 } else { 0 };
    if first != 0 && (first < 0 || first as usize > s.len()) {
        vm.ret_f(-1.0);
        return Ok(());
    }
    let start = first.max(0) as usize;
    if sub.is_empty() {
        vm.ret_f(start as f32);
        return Ok(());
    }
    let found = s[start..].windows(sub.len()).position(|w| w == sub.as_slice());
    vm.ret_f(found.map_or(-1.0, |i| (start + i) as f32));
    Ok(())
}

pub(super) fn pf_strpad(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let pad = vm.g_f(ofs_parm(0)) as i32;
    let src = vm.var_string(1)?;
    let max = crate::STRINGTEMP_LENGTH - 1;
    let mut out = Vec::new();
    if pad < 0 {
        let want = (-pad) as usize;
        let spaces = want.saturating_sub(src.len()).min(max);
        out.resize(spaces, b' ');
        let take = src.len().min(max - out.len());
        out.extend_from_slice(&src[..take]);
    } else {
        let want = (pad as usize).min(max);
        let take = src.len().min(max);
        out.extend_from_slice(&src[..take]);
        if out.len() < want {
            out.resize(want, b' ');
        }
    }
    vm.ret_temp(&out);
    Ok(())
}

pub(super) fn pf_infoadd(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let info = vm.g_str(ofs_parm(0))?.to_vec();
    let key = vm.g_str(ofs_parm(1))?.to_vec();
    let val = vm.var_string(2)?;
    if key.is_empty() {
        let h = vm.g_i(ofs_parm(0));
        vm.ret_i(h);
        return Ok(());
    }
    let max = crate::STRINGTEMP_LENGTH - 1;
    let mut out = Vec::new();
    let mut i = 0;
    while i < info.len() {
        let pair_start = i;
        if info[i] != b'\\' {
            break;
        }
        i += 1;
        let kstart = i;
        while i < info.len() && info[i] != b'\\' {
            i += 1;
        }
        let k = &info[kstart..i];
        if i >= info.len() {
            break;
        }
        i += 1; // skip '\\'
        while i < info.len() && info[i] != b'\\' {
            i += 1;
        }
        if k != key.as_slice() && out.len() + (i - pair_start) < max {
            out.extend_from_slice(&info[pair_start..i]);
        }
    }
    if !val.is_empty()
        && !key.contains(&b'\\')
        && !val.contains(&b'\\')
        && out.len() + 2 + key.len() + val.len() < max
    {
        out.push(b'\\');
        out.extend_from_slice(&key);
        out.push(b'\\');
        out.extend_from_slice(&val);
    }
    vm.ret_temp(&out);
    Ok(())
}

pub(super) fn pf_infoget(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let info = vm.g_str(ofs_parm(0))?.to_vec();
    let key = vm.g_str(ofs_parm(1))?.to_vec();
    let mut i = 0;
    while i < info.len() {
        if info[i] != b'\\' {
            break;
        }
        i += 1;
        let kstart = i;
        while i < info.len() && info[i] != b'\\' {
            i += 1;
        }
        let k = &info[kstart..i];
        if i >= info.len() {
            break;
        }
        i += 1;
        let vstart = i;
        while i < info.len() && info[i] != b'\\' {
            i += 1;
        }
        if k == key.as_slice() {
            let v = &info[vstart..i];
            let take = v.len().min(crate::STRINGTEMP_LENGTH - 1);
            vm.ret_temp(&v[..take]);
            return Ok(());
        }
    }
    vm.ret_i(0);
    Ok(())
}

fn cmp_bytes(a: &[u8], b: &[u8], len: Option<usize>, fold_case: bool) -> i32 {
    let n = len.unwrap_or(usize::MAX);
    for i in 0..n {
        let ca = a.get(i).copied().map(|c| if fold_case { c.to_ascii_lowercase() } else { c });
        let cb = b.get(i).copied().map(|c| if fold_case { c.to_ascii_lowercase() } else { c });
        match (ca, cb) {
            (Some(x), Some(y)) if x == y => continue,
            (Some(x), Some(y)) => return x as i32 - y as i32,
            (Some(x), None) => return x as i32,
            (None, Some(y)) => return -(y as i32),
            (None, None) => return 0,
        }
    }
    0
}

pub(super) fn pf_strncmp(vm: &mut Vm, _: &mut dyn Host, fold_case: bool) -> Result<()> {
    let a = vm.g_str(ofs_parm(0))?.to_vec();
    let b = vm.g_str(ofs_parm(1))?.to_vec();
    let r = if vm.argc > 2 {
        let len = (vm.g_f(ofs_parm(2)) as i32).max(0) as usize;
        let aofs = if vm.argc > 3 { vm.g_f(ofs_parm(3)) as i32 } else { 0 };
        let bofs = if vm.argc > 4 { vm.g_f(ofs_parm(4)) as i32 } else { 0 };
        let aofs = if aofs < 0 || aofs as usize > a.len() { a.len() } else { aofs as usize };
        let bofs = if bofs < 0 || bofs as usize > b.len() { b.len() } else { bofs as usize };
        cmp_bytes(&a[aofs..], &b[bofs..], Some(len), fold_case)
    } else {
        cmp_bytes(&a, &b, None, fold_case)
    };
    vm.ret_f(r as f32);
    Ok(())
}

pub(super) fn pf_strtrim(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let s = vm.g_str(ofs_parm(0))?.to_vec();
    let ws = |c: &u8| matches!(*c, b' ' | b'\t' | b'\n' | b'\r');
    let start = s.iter().position(|c| !ws(c)).unwrap_or(s.len());
    let end = s.iter().rposition(|c| !ws(c)).map_or(start, |i| i + 1);
    let slice = &s[start..end];
    let take = slice.len().min(crate::STRINGTEMP_LENGTH - 1);
    vm.ret_temp(&slice[..take]);
    Ok(())
}

pub(super) fn pf_strcase(vm: &mut Vm, _: &mut dyn Host, upper: bool) -> Result<()> {
    let mut s = vm.g_str(ofs_parm(0))?.to_vec();
    for c in &mut s {
        *c = if upper { c.to_ascii_uppercase() } else { c.to_ascii_lowercase() };
    }
    s.truncate(crate::STRINGTEMP_LENGTH - 1);
    vm.ret_temp(&s);
    Ok(())
}

pub(super) fn pf_strreplace(vm: &mut Vm, _: &mut dyn Host, ci: bool) -> Result<()> {
    let search = vm.g_str(ofs_parm(0))?.to_vec();
    let replace = vm.g_str(ofs_parm(1))?.to_vec();
    let subject = vm.g_str(ofs_parm(2))?.to_vec();
    let max = crate::STRINGTEMP_LENGTH - 1;
    if search.is_empty() {
        let take = subject.len().min(max);
        vm.ret_temp(&subject[..take]);
        return Ok(());
    }
    let mut out = Vec::with_capacity(subject.len().min(max));
    let mut i = 0;
    while i < subject.len() && out.len() < max {
        let matched = i + search.len() <= subject.len()
            && if ci {
                subject[i..i + search.len()].eq_ignore_ascii_case(&search)
            } else {
                &subject[i..i + search.len()] == search.as_slice()
            };
        if matched {
            let take = replace.len().min(max - out.len());
            out.extend_from_slice(&replace[..take]);
            i += search.len();
        } else {
            out.push(subject[i]);
            i += 1;
        }
    }
    vm.ret_temp(&out);
    Ok(())
}

pub(super) fn pf_stoi(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let s = String::from_utf8_lossy(vm.g_str(ofs_parm(0))?).trim_start().to_string();
    let v = if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(h, 16).unwrap_or(0) as i32
    } else {
        atof(s.as_bytes()) as i32
    };
    vm.ret_i(v);
    Ok(())
}

pub(super) fn pf_itos(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let s = format!("{}", vm.g_i(ofs_parm(0)));
    vm.ret_temp(s.as_bytes());
    Ok(())
}

pub(super) fn pf_stoh(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let s = String::from_utf8_lossy(vm.g_str(ofs_parm(0))?).trim_start().to_string();
    let h = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(&s);
    let end = h.bytes().take_while(|b| b.is_ascii_hexdigit()).count();
    let v = u32::from_str_radix(&h[..end], 16).unwrap_or(0) as i32;
    vm.ret_i(v);
    Ok(())
}

pub(super) fn pf_htos(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let s = format!("{:08x}", vm.g_i(ofs_parm(0)) as u32);
    vm.ret_temp(s.as_bytes());
    Ok(())
}

pub(super) fn pf_findfloat(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let start = vm.g_i(ofs_parm(0)).max(0) as usize;
    let fld = vm.g_i(ofs_parm(1)) as usize;
    let match_bits = vm.globals[ofs_parm(2)];
    if fld < vm.progs.entityfields {
        for e in (start + 1)..vm.num_edicts() {
            if !vm.is_free(e as u32) && vm.edict(e as u32)[fld] == match_bits {
                vm.ret_e(e as u32);
                return Ok(());
            }
        }
    }
    vm.ret_e(0);
    Ok(())
}

pub(super) fn pf_findflags(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let start = vm.g_i(ofs_parm(0)).max(0) as usize;
    let fld = vm.g_i(ofs_parm(1)) as usize;
    let mask = vm.g_f(ofs_parm(2)) as i32;
    if fld < vm.progs.entityfields {
        for e in (start + 1)..vm.num_edicts() {
            if !vm.is_free(e as u32) && (vm.ent_f(e as u32, fld) as i32 & mask) != 0 {
                vm.ret_e(e as u32);
                return Ok(());
            }
        }
    }
    vm.ret_e(0);
    Ok(())
}

pub(super) fn pf_copyentity(vm: &mut Vm, host: &mut dyn Host) -> Result<()> {
    let src = vm.g_i(ofs_parm(0));
    let dst = if vm.argc >= 2 { vm.g_i(ofs_parm(1)) } else { vm.ed_alloc()? as i32 };
    if src >= 0 && dst >= 0 && (src as usize) < vm.num_edicts() && (dst as usize) < vm.num_edicts() {
        let data = vm.edict(src as u32).to_vec();
        vm.edict_mut(dst as u32).copy_from_slice(&data);
        host.copied_entity(vm, src as u32, dst as u32);
    }
    vm.ret_e(dst.max(0) as u32);
    Ok(())
}

pub(super) fn pf_edict_num(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let n = vm.g_f(ofs_parm(0)) as i32;
    if n >= 0 && (n as usize) < vm.num_edicts() {
        vm.ret_e(n as u32);
    } else {
        vm.ret_e(0);
    }
    Ok(())
}

pub(super) fn pf_num_for_edict(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let e = vm.g_i(ofs_parm(0));
    vm.ret_f(e as f32);
    Ok(())
}

pub(super) fn pf_wasfreed(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let e = vm.g_i(ofs_parm(0));
    let freed = e < 0 || (e as usize) >= vm.num_edicts() || vm.is_free(e as u32);
    vm.ret_f(i32::from(freed) as f32);
    Ok(())
}

pub(super) fn pf_checkbuiltin(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let fnum = vm.g_i(ofs_parm(0));
    vm.ret_f(i32::from(vm.is_builtin_supported(fnum)) as f32);
    Ok(())
}

pub(super) fn pf_builtin_find(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let name = String::from_utf8_lossy(vm.g_str(ofs_parm(0))?).into_owned();
    let num = lookup_builtin_num(&name).unwrap_or(0);
    vm.ret_f(num as f32);
    Ok(())
}
