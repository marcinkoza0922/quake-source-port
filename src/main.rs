//! quake-source-port binary. The engine lives in the library (`src/lib.rs`) so Android
//! can load the same code as a `cdylib` (`libmain.so`); see `android/README.md`.

fn main() {
    quake_source_port::run();
}
