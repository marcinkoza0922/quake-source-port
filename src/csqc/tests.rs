//! End-to-end CSQC tests using a synthetic `progs.dat` bytecode builder.

use qclient::ClientState;
use qcore::cmd::Cbuf;
use qcore::cvar::Cvars;
use qcore::fs::Fs;
use qnet::protocol::STAT_HEALTH;
use crate::gfx::Renderer;
use qrender::Draw2D;
use qvm::ofs_parm;

use super::{Csqc, CsqcEnv};
use crate::assets::Level;
use crate::console::Console;
use crate::draw::Canvas;

// QC opcodes (progs.h)
const OP_DONE: u16 = 0;
const OP_STORE_F: u16 = 31;
const OP_STORE_V: u16 = 32;
const OP_CALL0: u16 = 51;
const OP_CALL1: u16 = 52;
const OP_CALL4: u16 = 55;
const OP_CALL5: u16 = 56;
const OP_RETURN: u16 = 43;

/// Tiny v6 `progs.dat` assembler for unit-testing CSQC entry points and builtins.
struct ProgsAsm {
    strings: Vec<u8>,
    statements: Vec<[u16; 4]>,
    functions: Vec<(i32, i32, i32, i32, u32, u32, i32, [u8; 8])>,
    globals: Vec<u32>,
    globaldefs: Vec<(u16, u16, u32)>,
}

impl ProgsAsm {
    fn new() -> Self {
        let mut a = Self {
            strings: vec![0],
            statements: vec![[OP_DONE, 0, 0, 0]],
            functions: vec![(0, 0, 0, 0, 0, 0, 0, [0; 8])],
            globals: vec![0; 64],
            globaldefs: Vec::new(),
        };
        a.add_global_def("time", 2, 28);
        a.add_global_def("self", 4, 29);
        a
    }

    fn str(&mut self, s: &str) -> u32 {
        let ofs = self.strings.len() as u32;
        self.strings.extend_from_slice(s.as_bytes());
        self.strings.push(0);
        ofs
    }

    fn add_global_def(&mut self, name: &str, ty: u16, ofs: u16) {
        let n = self.str(name);
        self.globaldefs.push((ty, ofs, n));
    }

    fn const_f(&mut self, v: f32) -> u16 {
        let ofs = self.globals.len() as u16;
        self.globals.push(v.to_bits());
        ofs
    }

    fn const_v(&mut self, v: [f32; 3]) -> u16 {
        let ofs = self.globals.len() as u16;
        for c in v {
            self.globals.push(c.to_bits());
        }
        ofs
    }

    fn const_s(&mut self, s: &str) -> u16 {
        let sofs = self.str(s);
        let ofs = self.globals.len() as u16;
        self.globals.push(sofs);
        ofs
    }

    fn builtin(&mut self, name: &str, num: i32, nparms: i32) -> u16 {
        let n = self.str(name);
        let idx = self.functions.len() as u32;
        self.functions.push((-num, 0, 0, 0, n, 0, nparms, [1; 8]));
        let ofs = self.globals.len() as u16;
        self.globals.push(idx);
        ofs
    }

    fn func(&mut self, name: &str, body: &[[u16; 4]]) -> u32 {
        let n = self.str(name);
        let first = self.statements.len() as i32;
        self.statements.extend_from_slice(body);
        self.statements.push([OP_DONE, 0, 0, 0]);
        let idx = self.functions.len() as u32;
        self.functions.push((first, 0, 0, 0, n, 0, 0, [0; 8]));
        idx
    }

    fn build(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let hdr = 60usize;
        let ofs_stmt = hdr;
        let len_stmt = self.statements.len() * 8;
        let ofs_gdef = ofs_stmt + len_stmt;
        let len_gdef = self.globaldefs.len() * 8;
        let ofs_fdef = ofs_gdef + len_gdef;
        let ofs_func = ofs_fdef;
        let len_func = self.functions.len() * 36;
        let ofs_str = ofs_func + len_func;
        let len_str = self.strings.len();
        let ofs_glob = ofs_str + len_str;

        let hdr_words: [i32; 15] = [
            qvm::PROG_VERSION,
            0, // crc (not required for CSQC)
            ofs_stmt as i32, self.statements.len() as i32,
            ofs_gdef as i32, self.globaldefs.len() as i32,
            ofs_fdef as i32, 0,
            ofs_func as i32, self.functions.len() as i32,
            ofs_str as i32, len_str as i32,
            ofs_glob as i32, self.globals.len() as i32,
            0, // entityfields
        ];
        for w in hdr_words { out.extend_from_slice(&w.to_le_bytes()); }
        for st in &self.statements {
            for x in st { out.extend_from_slice(&x.to_le_bytes()); }
        }
        for &(ty, ofs, n) in &self.globaldefs {
            out.extend_from_slice(&ty.to_le_bytes());
            out.extend_from_slice(&ofs.to_le_bytes());
            out.extend_from_slice(&n.to_le_bytes());
        }
        for &(fs, ps, loc, prof, nm, fl, np, sz) in &self.functions {
            for w in [fs, ps, loc, prof, nm as i32, fl as i32, np] {
                out.extend_from_slice(&w.to_le_bytes());
            }
            out.extend_from_slice(&sz);
        }
        out.extend_from_slice(&self.strings);
        for &g in &self.globals {
            out.extend_from_slice(&g.to_le_bytes());
        }
        out
    }
}

#[test]
fn csqc_hud_clip_events_and_commands_roundtrip() {
    let Ok(mut renderer) = Renderer::headless(640, 480, &[[0; 3]; 256]) else {
        return; // skip cleanly on machines without Vulkan
    };
    let mut a = ProgsAsm::new();
    let f_regcmd = a.builtin("registercommand", 352, 1);
    let f_getstati = a.builtin("getstati", 330, 1);
    let f_getstatf = a.builtin("getstatf", 331, 3);
    let f_itof = a.builtin("itof", 0, 1);
    let f_drawfill = a.builtin("drawfill", 323, 4);
    let f_drawstr = a.builtin("drawstring", 326, 5);
    let f_setclip = a.builtin("drawsetcliparea", 324, 4);
    let f_rstclip = a.builtin("drawresetcliparea", 325, 0);
    let f_readbyte = a.builtin("readbyte", 360, 0);
    let f_cvarset = a.builtin("cvar_set", 72, 2);
    let f_ftos = a.builtin("ftos", 26, 1);

    let c_cmd = a.const_s("csqc_ping");
    let c_hp_stat = a.const_f(STAT_HEALTH as f32);
    let c_items_stat = a.const_f(qnet::protocol::STAT_ITEMS as f32);
    let c_bit_start = a.const_f(23.0);
    let c_bit_count = a.const_f(9.0);
    let c_pos = a.const_v([10.0, 20.0, 0.0]);
    let c_sz = a.const_v([100.0, 20.0, 0.0]);
    let c_rgb = a.const_v([1.0, 0.5, 0.0]);
    let c_alpha = a.const_f(0.75);
    let c_scale = a.const_v([8.0, 8.0, 0.0]);
    let c_text = a.const_s("^1HP");
    let c_clip0 = a.const_f(4.0);
    let c_clip1 = a.const_f(8.0);
    let c_clipw = a.const_f(200.0);
    let c_cliph = a.const_f(100.0);
    let c_one = a.const_f(1.0);
    let c_cv_hp = a.const_s("_test_hp");
    let c_cv_bits = a.const_s("_test_bits");
    let c_cv_ev = a.const_s("_test_ev");
    let p0 = ofs_parm(0) as u16;
    let p1 = ofs_parm(1) as u16;
    let p2 = ofs_parm(2) as u16;
    let p3 = ofs_parm(3) as u16;
    let p4 = ofs_parm(4) as u16;
    let ret = qvm::OFS_RETURN as u16;

    // CSQC_Init: registercommand("csqc_ping")
    a.func("CSQC_Init", &[
        [OP_STORE_F, c_cmd, p0, 0],
        [OP_CALL1, f_regcmd, 0, 0],
    ]);
    // CSQC_DrawHud: read int health via getstati->itof -> _test_hp; read bit-slice via getstatf -> _test_bits; draw
    a.func("CSQC_DrawHud", &[
        [OP_STORE_F, c_hp_stat, p0, 0],
        [OP_CALL1, f_getstati, 0, 0],
        [OP_STORE_F, ret, p0, 0],
        [OP_CALL1, f_itof, 0, 0],
        [OP_STORE_F, ret, p0, 0],
        [OP_CALL1, f_ftos, 0, 0],
        [OP_STORE_F, c_cv_hp, p0, 0],
        [OP_STORE_F, ret, p1, 0],
        [OP_CALL1, f_cvarset, 0, 0],
        [OP_STORE_F, c_items_stat, p0, 0],
        [OP_STORE_F, c_bit_start, p1, 0],
        [OP_STORE_F, c_bit_count, p2, 0],
        [54, f_getstatf, 0, 0], // OP_CALL3
        [OP_STORE_F, ret, p0, 0],
        [OP_CALL1, f_ftos, 0, 0],
        [OP_STORE_F, c_cv_bits, p0, 0],
        [OP_STORE_F, ret, p1, 0],
        [OP_CALL1, f_cvarset, 0, 0],
        [OP_STORE_F, c_clip0, p0, 0],
        [OP_STORE_F, c_clip1, p1, 0],
        [OP_STORE_F, c_clipw, p2, 0],
        [OP_STORE_F, c_cliph, p3, 0],
        [OP_CALL4, f_setclip, 0, 0],
        [OP_STORE_V, c_pos, p0, 0],
        [OP_STORE_V, c_sz, p1, 0],
        [OP_STORE_V, c_rgb, p2, 0],
        [OP_STORE_F, c_alpha, p3, 0],
        [OP_CALL4, f_drawfill, 0, 0],
        [OP_STORE_V, c_pos, p0, 0],
        [OP_STORE_F, c_text, p1, 0],
        [OP_STORE_V, c_scale, p2, 0],
        [OP_STORE_V, c_rgb, p3, 0],
        [OP_STORE_F, c_alpha, p4, 0],
        [OP_CALL5, f_drawstr, 0, 0],
        [OP_CALL0, f_rstclip, 0, 0],
    ]);
    // CSQC_Parse_Event: b = readbyte(); cvar_set("_test_ev", ftos(b))
    a.func("CSQC_Parse_Event", &[
        [OP_CALL0, f_readbyte, 0, 0],
        [OP_STORE_F, ret, p0, 0],
        [OP_CALL1, f_ftos, 0, 0],
        [OP_STORE_F, c_cv_ev, p0, 0],
        [OP_STORE_F, ret, p1, 0],
        [OP_CALL1, f_cvarset, 0, 0],
    ]);
    // CSQC_ConsoleCommand: return 1.0
    a.func("CSQC_ConsoleCommand", &[
        [OP_RETURN, c_one, 0, 0],
    ]);

    let bytes = a.build();
    let fs = Fs::new(std::path::Path::new("."));
    let mut cvars = Cvars::new();
    cvars.register("pr_checkextension", "1", 0);
    cvars.register("cl_nocsqc", "0", 0);
    cvars.register("_test_hp", "0", 0);
    cvars.register("_test_bits", "0", 0);
    cvars.register("_test_ev", "0", 0);
    let mut cl = ClientState::new();
    cl.stats[STAT_HEALTH] = 87;
    cl.stats[qnet::protocol::STAT_ITEMS] = 0x15 | (0x3a << 23);
    let mut level = Level::default();
    let mut con = Console::new();
    let mut cbuf = Cbuf::new();
    let mut canvas = Canvas::new(640, 480);

    let mut csqc = Csqc::default();
    {
        let mut env = CsqcEnv {
            fs: &fs,
            cvars: &mut cvars,
            cl: &cl,
            level: &mut level,
            r: &mut renderer,
            snd: None,
            con: &mut con,
            cbuf: &mut cbuf,
            canvas: Some(&mut canvas),
            base_view: None,
            is_demo: false,
            realtime: 1.0,
            frametime: 0.016,
            width: 640,
            height: 480,
            virt: [320.0, 240.0],
            show_scores: false,
            not_menu: true,
        };
        assert!(csqc.load_bytes(&bytes, &mut env));
        assert!(csqc.has_simple_hud());
        assert!(csqc.has_command("csqc_ping"));
        assert!(csqc.console_command("csqc_ping", &mut env));
        csqc.draw_hud(&mut env);
        csqc.parse_event(&[42], &mut env);
    }

    assert_eq!(cvars.value_of("_test_hp") as i32, 87);
    assert_eq!(cvars.value_of("_test_bits") as i32, 0x3a);
    assert_eq!(cvars.value_of("_test_ev") as i32, 42);
    // Verify Clip(Some), Fill, 2 red Char glyphs ('H', 'P'), Clip(None)
    assert!(matches!(canvas.list.first(), Some(Draw2D::Clip(Some([8.0, 16.0, 400.0, 200.0])))));
    assert!(matches!(canvas.list.last(), Some(Draw2D::Clip(None))));
    let red_chars: Vec<u8> = canvas
        .list
        .iter()
        .filter_map(|d| match *d {
            Draw2D::Char { ch, color, .. } if color[0] > 0.9 && color[1] < 0.1 => Some(ch),
            _ => None,
        })
        .collect();
    assert_eq!(red_chars, vec![b'H', b'P']);
}
