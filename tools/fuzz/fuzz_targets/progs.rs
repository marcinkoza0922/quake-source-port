//! Hostile progs.dat: load, then run its functions with the engine builtins stubbed.
#![no_main]
use libfuzzer_sys::fuzz_target;
use qvm::{Builtin, Host, Progs, Vm};

struct Stub;
impl Host for Stub {
    fn builtin(&mut self, vm: &mut Vm, _: Builtin) -> qvm::Result<()> {
        vm.ret_f(0.0);
        Ok(())
    }
    fn ext_builtin(&mut self, vm: &mut Vm, _: &str) -> qvm::Result<()> {
        vm.ret_f(0.0);
        Ok(())
    }
}

fuzz_target!(|data: &[u8]| {
    let Ok(p) = Progs::parse_csqc(data) else { return };
    let n = p.functions.len();
    let Ok(mut vm) = Vm::new_csqc(p, 256) else { return };
    let _ = vm.load_from_file(&mut Stub, "{\"classname\" \"worldspawn\"}{\"classname\" \"x\" \"origin\" \"1 2 3\"}", qvm::SpawnFilter::default());
    for f in 1..n.min(24) {
        let _ = vm.execute(&mut Stub, f);
    }
});
