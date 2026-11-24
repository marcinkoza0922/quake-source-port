use super::*;

pub(super) fn sys_vecs(vm: &Vm) -> Result<[usize; 3]> {
    let g = |n| vm.global_ofs(n).ok_or_else(|| vm.run_error(format!("missing global {n}")));
    Ok([g("v_forward")?, g("v_right")?, g("v_up")?])
}

pub(super) fn pf_makevectors(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let [f, r, u] = sys_vecs(vm)?;
    let (a, b, c) = angle_vectors(vm.g_v(ofs_parm(0)));
    vm.set_g_v(f, a);
    vm.set_g_v(r, b);
    vm.set_g_v(u, c);
    Ok(())
}

pub(super) fn normalize_f(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l == 0.0 { v } else { v.map(|x| x * (1.0 / l)) }
}

pub(super) fn pf_vectorvectors(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let [fo, ro, uo] = sys_vecs(vm)?;
    let f = normalize_f(vm.g_v(ofs_parm(0)));
    let r = if f[0] == 0.0 && f[1] == 0.0 {
        [0.0, if f[2] != 0.0 { -1.0 } else { 0.0 }, 0.0]
    } else {
        normalize_f([f[1], -f[0], 0.0])
    };
    let u = [r[1] * f[2] - r[2] * f[1], r[2] * f[0] - r[0] * f[2], r[0] * f[1] - r[1] * f[0]];
    vm.set_g_v(fo, f);
    vm.set_g_v(ro, r);
    vm.set_g_v(uo, u);
    Ok(())
}

pub(super) fn pf_break(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    Err(vm.run_error("break statement"))
}

pub(super) fn pf_random(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    // sv_gameplayfix_random 1
    let n = (vm.rand15() as f32 + 0.5) * (1.0 / 32768.0);
    vm.ret_f(n);
    Ok(())
}

pub(super) fn pf_normalize(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let v = vm.g_v(ofs_parm(0));
    let l = (v[0] as f64 * v[0] as f64 + v[1] as f64 * v[1] as f64 + v[2] as f64 * v[2] as f64).sqrt();
    let r = if l == 0.0 { [0.0; 3] } else { v.map(|x| (x as f64 * (1.0 / l)) as f32) };
    vm.ret_v(r);
    Ok(())
}

pub(super) fn pf_vlen(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let v = vm.g_v(ofs_parm(0));
    let l = (v[0] as f64 * v[0] as f64 + v[1] as f64 * v[1] as f64 + v[2] as f64 * v[2] as f64).sqrt();
    vm.ret_f(l as f32);
    Ok(())
}

pub(super) fn yaw_of(v: [f32; 3]) -> f32 {
    if v[1] == 0.0 && v[0] == 0.0 {
        return 0.0;
    }
    let mut yaw = ((v[1] as f64).atan2(v[0] as f64) * 180.0 / std::f64::consts::PI) as i32 as f32;
    if yaw < 0.0 {
        yaw += 360.0;
    }
    yaw
}

pub(super) fn pf_vectoyaw(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let y = yaw_of(vm.g_v(ofs_parm(0)));
    vm.ret_f(y);
    Ok(())
}

/// VectorAngles with an up vector (DP_QC_VECTOANGLES_WITH_ROLL); the pitch is
/// the model's, like the one-argument builtin's.
fn angles_with_roll(f: [f32; 3], up: [f32; 3]) -> [f32; 3] {
    const DEG: f32 = 180.0 / std::f32::consts::PI;
    if f[0] == 0.0 && f[1] == 0.0 {
        return if f[2] > 0.0 { [90.0, (-up[1]).atan2(-up[0]) * DEG, 0.0] } else { [-90.0, up[1].atan2(up[0]) * DEG, 0.0] };
    }
    let pitch = -f[2].atan2((f[0] * f[0] + f[1] * f[1]).sqrt());
    let yaw = f[1].atan2(f[0]);
    let ((sp, cp), (sy, cy)) = (pitch.sin_cos(), yaw.sin_cos());
    let dot = |a: [f32; 3]| a[0] * up[0] + a[1] * up[1] + a[2] * up[2];
    let roll = -dot([-sy, cy, 0.0]).atan2(dot([sp * cy, sp * sy, cp]));
    [-pitch * DEG, yaw * DEG, roll * DEG]
}

pub(super) fn pf_vectoangles(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let v = vm.g_v(ofs_parm(0));
    if vm.argc >= 2 {
        let a = angles_with_roll(v, vm.g_v(ofs_parm(1)));
        vm.ret_v(a);
        return Ok(());
    }
    let (pitch, yaw);
    if v[1] == 0.0 && v[0] == 0.0 {
        yaw = 0.0;
        pitch = if v[2] > 0.0 { 90.0 } else { 270.0 };
    } else {
        yaw = yaw_of(v);
        let fwd = (v[0] * v[0] + v[1] * v[1]).sqrt();
        let mut p = ((v[2] as f64).atan2(fwd as f64) * 180.0 / std::f64::consts::PI) as i32 as f32;
        if p < 0.0 {
            p += 360.0;
        }
        pitch = p;
    }
    vm.ret_v([pitch, yaw, 0.0]);
    Ok(())
}

pub(super) fn pf_dprint(vm: &mut Vm, host: &mut dyn Host) -> Result<()> {
    let s = vm.var_string(0)?;
    host.dprint(&String::from_utf8_lossy(&s));
    Ok(())
}

/// PF_ftos formatting.
pub fn ftos(v: f32) -> String {
    if v == (v as i32) as f32 { format!("{}", v as i32) } else { format!("{v:5.1}") }
}

pub(super) fn pf_ftos(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let s = ftos(vm.g_f(ofs_parm(0)));
    vm.ret_temp(s.as_bytes());
    Ok(())
}

pub(super) fn pf_vtos(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let v = vm.g_v(ofs_parm(0));
    let s = format!("'{:5.1} {:5.1} {:5.1}'", v[0], v[1], v[2]);
    vm.ret_temp(s.as_bytes());
    Ok(())
}

pub(super) fn pf_substring(vm: &mut Vm, host: &mut dyn Host) -> Result<()> {
    let s = vm.g_str(ofs_parm(0))?.to_vec();
    let (mut start, mut length) = (vm.g_f(ofs_parm(1)) as i32, vm.g_f(ofs_parm(2)) as i32);
    let mut slen = s.len() as i32;
    if start < 0 {
        start += slen;
    }
    if length < 0 {
        length = slen - start + (length + 1);
    }
    if start < 0 {
        start = 0;
    }
    if start >= slen || length <= 0 {
        let h = vm.strings.engine(b"");
        vm.ret_i(h);
        return Ok(());
    }
    slen -= start;
    length = length.min(slen);
    if length as usize >= crate::STRINGTEMP_LENGTH {
        length = crate::STRINGTEMP_LENGTH as i32 - 1;
        host.print("PF_substring: truncation\n");
    }
    vm.ret_temp(&s[start as usize..(start + length) as usize]);
    Ok(())
}

fn chrconv_number(c: u8, base: u8, conv: i32) -> u8 {
    let i = c.wrapping_sub(base);
    let nb = match conv {
        1 => b'0',
        2 => b'0' + 128,
        3 => b'0' - 30,
        4 => b'0' + 128 - 30,
        _ => base,
    };
    i.wrapping_add(nb)
}

fn chrchar_alpha(c: u8, basec: u8, baset: u8, convc: i32, convt: i32, idx: usize) -> u8 {
    let i = c.wrapping_sub(baset.wrapping_add(basec));
    let nt = match convt {
        1 => 0,
        2 => 128,
        5 | 6 => 128 * u8::from((idx & 1) == (convt - 5) as usize),
        _ => baset,
    };
    let nc = match convc {
        1 => b'a',
        2 => b'A',
        _ => basec,
    };
    i.wrapping_add(nc).wrapping_add(nt)
}

/// FTE_STRINGS `strconv` (port of vkQuake `PF_strconv`).
pub(super) fn pf_strconv(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let ccase = vm.g_f(ofs_parm(0)) as i32;
    let redalpha = vm.g_f(ofs_parm(1)) as i32;
    let rednum = vm.g_f(ofs_parm(2)) as i32;
    let mut s = vm.var_string(3)?;
    for (idx, c) in s.iter_mut().enumerate() {
        let ch = *c;
        *c = match ch {
            b'0'..=b'9' => chrconv_number(ch, b'0', rednum),
            176..=185 => chrconv_number(ch, b'0' + 128, rednum),
            146..=155 => chrconv_number(ch, b'0' + 128 - 30, rednum),
            18..=27 => chrconv_number(ch, b'0' - 30, rednum),
            b'a'..=b'z' => chrchar_alpha(ch, b'a', 0, ccase, redalpha, idx),
            b'A'..=b'Z' => chrchar_alpha(ch, b'A', 0, ccase, redalpha, idx),
            225..=250 => chrchar_alpha(ch, b'a', 128, ccase, redalpha, idx),
            193..=218 => chrchar_alpha(ch, b'A', 128, ccase, redalpha, idx),
            _ if (ch & 127) < 16 || redalpha == 0 => ch,
            _ => match redalpha {
                1 => ch & 127,
                2 => ch | 128,
                _ => ch,
            },
        };
    }
    s.truncate(crate::STRINGTEMP_LENGTH - 1);
    vm.ret_temp(&s);
    Ok(())
}

/// Minimal C printf for one spec.
pub(super) fn pad(s: String, width: usize, left: bool, zero: bool) -> String {
    if s.len() >= width {
        return s;
    }
    let n = width - s.len();
    if left {
        s + &" ".repeat(n)
    } else if zero {
        let (sign, rest) = if s.starts_with('-') || s.starts_with('+') { s.split_at(1) } else { ("", s.as_str()) };
        format!("{sign}{}{rest}", "0".repeat(n))
    } else {
        " ".repeat(n) + &s
    }
}

pub(super) fn pf_sprintf(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let fmt = vm.g_str(ofs_parm(0))?.to_vec();
    let mut out: Vec<u8> = Vec::new();
    let mut arg = 1;
    let mut i = 0;
    while i < fmt.len() {
        let c = fmt[i];
        i += 1;
        if c != b'%' {
            out.push(c);
            continue;
        }
        if i < fmt.len() && fmt[i] == b'%' {
            out.push(b'%');
            i += 1;
            continue;
        }
        let (mut left, mut zero, mut plus) = (false, false, false);
        while i < fmt.len() && b"#0- +".contains(&fmt[i]) {
            match fmt[i] {
                b'-' => left = true,
                b'0' => zero = true,
                b'+' => plus = true,
                _ => {}
            }
            i += 1;
        }
        let num = |i: &mut usize, arg: &mut usize| -> Option<usize> {
            if *i < fmt.len() && fmt[*i] == b'*' {
                *i += 1;
                let v = if *arg < vm.argc { vm.g_f(ofs_parm(*arg)) as i32 } else { 0 };
                *arg += 1;
                return Some(v.max(0) as usize);
            }
            let st = *i;
            while *i < fmt.len() && fmt[*i].is_ascii_digit() {
                *i += 1;
            }
            std::str::from_utf8(&fmt[st..*i]).ok()?.parse().ok()
        };
        let width = num(&mut i, &mut arg).unwrap_or(0);
        let prec = if i < fmt.len() && fmt[i] == b'.' {
            i += 1;
            Some(num(&mut i, &mut arg).unwrap_or(0))
        } else {
            None
        };
        while i < fmt.len() && b"hlLjzt".contains(&fmt[i]) {
            i += 1;
        }
        let Some(&conv) = fmt.get(i) else { break };
        i += 1;
        let p = ofs_parm(arg);
        let have = arg < vm.argc && arg < 8;
        arg += 1;
        let fa = if have { vm.g_f(p) } else { 0.0 };
        let sign = |s: String, v: f32| if plus && v >= 0.0 { format!("+{s}") } else { s };
        let s: Vec<u8> = match conv {
            b'd' | b'i' => sign(format!("{}", fa as i32), fa).into_bytes(),
            b'u' => format!("{}", fa as i32 as u32).into_bytes(),
            b'x' => format!("{:x}", fa as i32).into_bytes(),
            b'X' => format!("{:X}", fa as i32).into_bytes(),
            b'c' => vec![fa as i32 as u8],
            b'f' | b'F' => sign(format!("{:.*}", prec.unwrap_or(6), fa), fa).into_bytes(),
            b'e' | b'E' => sign(format!("{:.*e}", prec.unwrap_or(6), fa), fa).into_bytes(),
            b'g' | b'G' => sign(fmt_g(fa), fa).into_bytes(),
            b'v' | b'V' => {
                let v = if have { vm.g_v(p) } else { [0.0; 3] };
                let f = |x: f32| match prec {
                    Some(pr) => format!("{x:.pr$}"),
                    None => fmt_g(x),
                };
                format!("{} {} {}", f(v[0]), f(v[1]), f(v[2])).into_bytes()
            }
            b's' | b'S' => {
                let mut s = if have { vm.strings.get(vm.g_i(p))?.to_vec() } else { Vec::new() };
                if let Some(pr) = prec {
                    s.truncate(pr);
                }
                s
            }
            other => vec![b'%', other],
        };
        let s = pad(String::from_utf8_lossy(&s).into_owned(), width, left, zero && !matches!(conv, b's' | b'S'));
        out.extend_from_slice(s.as_bytes());
    }
    out.truncate(crate::STRINGTEMP_LENGTH - 1);
    vm.ret_temp(&out);
    Ok(())
}
