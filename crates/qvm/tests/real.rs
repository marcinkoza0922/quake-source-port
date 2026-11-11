use std::path::Path;

use qcore::fs::Fs;
use qvm::{Builtin, Host, Result, SpawnFilter, Vm, ofs_parm};

#[derive(Default)]
struct Stub {
    calls: Vec<Builtin>,
}

impl Host for Stub {
    fn builtin(&mut self, vm: &mut Vm, b: Builtin) -> Result<()> {
        self.calls.push(b);
        match b {
            Builtin::Spawn => {
                let e = vm.ed_alloc()?;
                vm.ret_e(e);
            }
            Builtin::Remove => {
                let e = vm.g_i(ofs_parm(0)) as u32;
                vm.ed_free(e);
            }
            Builtin::Error | Builtin::Objerror => {
                let m = vm.var_string(0)?;
                return Err(vm.run_error(String::from_utf8_lossy(&m)));
            }
            _ => vm.ret_i(0),
        }
        Ok(())
    }
    fn ext_builtin(&mut self, vm: &mut Vm, _name: &str) -> Result<()> {
        vm.ret_i(0);
        Ok(())
    }
}

fn basedir() -> Option<std::path::PathBuf> {
    let mut base = Fs::default_basedir();
    if base.is_relative() && !base.exists() {
        base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(base);
    }
    base.join("id1").is_dir().then_some(base)
}

fn load(game: &str) -> Option<(Fs, Vm)> {
    let mut fs = Fs::new(basedir()?);
    fs.add_game_dir("id1");
    if game != "id1" && !fs.add_game_dir(game) {
        return None;
    }
    let data = fs.open("progs.dat")?;
    let vm = Vm::load(&data, 8192, 8).unwrap_or_else(|e| panic!("{game}: {e}"));
    Some((fs, vm))
}

fn spawn_map(fs: &Fs, vm: &mut Vm, map: &str) -> qvm::SpawnStats {
    let bsp = qformats::Bsp::parse(&fs.open(map).unwrap()).unwrap();
    let mut host = Stub::default();
    vm.execute_by_name(&mut host, "main").unwrap();
    vm.time = 1.0;
    let st = vm.load_from_file(&mut host, &bsp.entities, SpawnFilter { skill: 1, ..Default::default() })
        .unwrap_or_else(|e| panic!("{map}: {e}"));
    assert!(host.calls.contains(&Builtin::PrecacheModel));
    st
}

#[test]
fn id1() {
    let Some((fs, mut vm)) = load("id1") else {
        eprintln!("skip: no game data");
        return;
    };
    assert_eq!(vm.progs.header_crc, qvm::PROGHEADER_CRC);
    assert!(vm.field_ofs("origin").is_some());
    let mut host = Stub::default();
    // float(float v) anglemod
    vm.set_g_f(ofs_parm(0), -10.0);
    vm.argc = 1;
    vm.execute_by_name(&mut host, "anglemod").unwrap();
    assert_eq!(vm.g_f(qvm::OFS_RETURN), 350.0);
    let st = spawn_map(&fs, &mut vm, "maps/start.bsp");
    assert!(st.spawned > 50, "{st:?}");
    assert_eq!(vm.classname(0), "worldspawn");
    assert_eq!(vm.depth(), 0);
}

#[test]
fn mods() {
    for (game, map) in [("ad", "maps/start.bsp"), ("mg1", "maps/start.bsp")] {
        let Some((fs, mut vm)) = load(game) else {
            eprintln!("skip: no {game}");
            continue;
        };
        if fs.exists(map) {
            let st = spawn_map(&fs, &mut vm, map);
            assert!(st.spawned > 10, "{game}: {st:?}");
        }
    }
}

struct Failing;
impl Host for Failing {
    fn builtin(&mut self, vm: &mut Vm, b: Builtin) -> Result<()> {
        Err(vm.run_error(format!("boom {}", b.name())))
    }
}

#[test]
fn runtime_error_has_trace() {
    let Some((_, mut vm)) = load("id1") else { return };
    let e = vm.execute_by_name(&mut Failing, "SUB_Remove").unwrap_err();
    assert_eq!(e.msg, "boom remove");
    assert!(e.trace[0].contains("SUB_Remove"), "{e}");
    assert_eq!(vm.depth(), 0);
    // VM still usable after an error
    vm.set_g_f(ofs_parm(0), 370.0);
    vm.execute_by_name(&mut Stub::default(), "anglemod").unwrap();
    assert_eq!(vm.g_f(qvm::OFS_RETURN), 10.0);
}
