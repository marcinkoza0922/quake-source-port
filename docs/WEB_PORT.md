# Web (WASM) port

Goal: a browser-playable build of the port. The native Vulkan/SDL builds stay
as they are; the web build swaps the renderer and platform backends behind the
same Rust APIs so the host (`src/`) and the game crates barely change.

## Target and toolchain

- Target `wasm32-unknown-unknown`, glued with `wasm-bindgen` (CLI 0.2.130 is
  installed in `~/.cargo/bin`; it must match the `wasm-bindgen` crate version).
- Rendering: `wgpu` 30.0.2 with the `webgpu` feature (WebGPU in the browser).
  A WebGL2 fallback is optional, not required.
- No network in the sandbox. Only these extra crates are in the local cargo
  cache; add them with exactly these versions and build with `--offline`:
  `wgpu 30.0.2`, `naga 30.0.2`, `wasm-bindgen 0.2.130`,
  `wasm-bindgen-futures`, `js-sys 0.3.107`, `web-sys 0.3.107`, `web-time 1.1.0`,
  `pollster 1.0.1`, `bytemuck 1.25.2`, `console_error_panic_hook`.
- Only the root package (and its crate dependencies) is built for wasm; the
  `tools/*` crates stay native-only.

## Backend selection (same crate, same API)

- `qrender`: on `target_arch = "wasm32"`, or natively with the `wgpu` cargo
  feature, the public `Renderer` is a wgpu implementation; otherwise it is the
  existing Vulkan one. Public types (`types.rs`, `ViewFrustum`, `ShaderParam`,
  `GpuViewTiming`, …) are shared by both backends. `ash`/`librashader` become
  non-wasm target dependencies.
- `qplatform`: on wasm32 a web backend (DOM events, WebAudio, Gamepad API,
  pointer lock) exposes the same `Platform`, `Window`, `Event`, `AudioStream`
  API; SDL3 is a non-wasm target dependency.
- The root package gets a feature `wgpu = ["qrender/wgpu"]` so the wgpu backend
  can be tested natively, e.g.
  `cargo run --release --features wgpu -- -headless -autoquit 6 +map e1m1 "+delay 3 screenshot target/shots/wgpu"`.

## Frozen interfaces between workstreams

These signatures are fixed so the three workstreams can proceed in parallel.

### `qcore::time` (workstream core)

`pub use` of `web_time::{Instant, SystemTime, UNIX_EPOCH}` on wasm32 and of
the `std::time` ones natively (`Duration` is always `std::time::Duration`).
All of `src/` and the crates use these instead of `std::time::Instant` /
`SystemTime` (which panic on `wasm32-unknown-unknown`).

### `qcore::vfs` (workstream core)

Natively a thin wrapper over `std::fs`; on wasm32 an in-memory file tree.

```rust
pub fn read(path: &Path) -> io::Result<Vec<u8>>;
pub fn write(path: &Path, data: &[u8]) -> io::Result<()>;   // creates parent dirs
pub fn append(path: &Path, data: &[u8]) -> io::Result<()>;
pub fn exists(path: &Path) -> bool;
pub fn is_dir(path: &Path) -> bool;
pub fn is_file(path: &Path) -> bool;
pub fn create_dir_all(path: &Path) -> io::Result<()>;
pub fn remove_file(path: &Path) -> io::Result<()>;
pub fn rename(from: &Path, to: &Path) -> io::Result<()>;
pub fn read_dir(path: &Path) -> io::Result<Vec<DirEntry>>;  // DirEntry { path: PathBuf, is_dir: bool, len: u64 }
pub fn modified(path: &Path) -> io::Result<SystemTime>;     // qcore::time::SystemTime
pub fn open_read(path: &Path) -> io::Result<Box<dyn ReadSeek>>; // PAK/PK3 access
// Web population and persistence (also usable natively, where they hit std::fs):
pub fn mount_file(path: &Path, data: Vec<u8>);              // insert without marking dirty
pub fn take_dirty() -> Vec<PathBuf>;                        // paths written since the last call (wasm only; native returns empty)
```

`qcore::fs::Fs` (PAK/PK3/dir search paths) and every user-dir write in the
workspace (config.cfg, saves, screenshots, demos, condump, bug reports, …) go
through `qcore::vfs`. The web front-end mounts the user's game data under the
base dir `/quake` (so `/quake/id1/pak0.pak`) and the user dir is
`/quake-user/<game>`; `user_dir()` and `Fs::default_basedir()` return those on
wasm32.

### `qrender` web constructor (workstream render)

```rust
#[cfg(target_arch = "wasm32")]
pub async fn new_web(canvas: web_sys::HtmlCanvasElement, palette: &Palette) -> Result<Renderer>; // same palette type as `headless`
```

Size changes still go through the existing `Renderer::resize`. Natively with
`--features wgpu`, `Renderer::headless` builds the wgpu renderer offscreen
(`read_pixels`/screenshots work); `Renderer::new` with a `SurfaceProvider` may
return an error in the wgpu backend.

### `qplatform` web window (workstream platform)

`Platform::create_window` on wasm32 finds (or creates) `<canvas id="quake">`;
`Window::canvas(&self) -> web_sys::HtmlCanvasElement` (wasm32 only).
`drawable_size`/`window_size` report the canvas backing size.

## Main loop on the web

`src/lib.rs` keeps `run()` for native. On wasm32 the setup half becomes an
async `start()` exported with `#[wasm_bindgen]`, which builds the host and
then drives `host.frame(dt, events)` from `requestAnimationFrame`. The
browser cannot block, so nothing on the web path may sleep, spin, spawn
threads, or open sockets (networking is disabled; single player only).

## Front-end and build

- `web/index.html` + `web/main.js`: asks the user for their Quake data (a
  folder picker with `webkitdirectory`, or drag-and-drop of `id1/` / `.pak`
  files), stores it in IndexedDB so later visits start straight away, mounts
  it with an exported `vfs_mount(path, bytes)`, then calls `start(args)`.
  Written user files (`take_dirty`) are persisted to IndexedDB too.
- `tools/webbuild/build.sh` builds `--release --target wasm32-unknown-unknown`,
  runs `wasm-bindgen --target web` and copies `web/` into `target/web/`.
  Serve it with `python3 -m http.server -d target/web`.
- The web build is not uploaded to GitHub releases.

## Workstreams

| Stream | Owns | Done when |
|---|---|---|
| core | `qcore::time`, `qcore::vfs`, wasm portability of every crate except `qrender`/`qplatform`, `std::time`/`std::fs`/thread/socket uses in `src/` (except `src/lib.rs` and `src/platform.rs`) | `cargo check --offline --target wasm32-unknown-unknown` passes for qcore, qformats, qpool, qvm, qworld, qnet, qclient, qserver, qsound, qinput, qui, qpart; native nextest and clippy still clean |
| render | wgpu backend of `qrender` | `cargo check --offline --target wasm32-unknown-unknown -p qrender` passes; native `--features wgpu` headless screenshots of e1m1 / start show world, models, HUD and menus recognisably |
| platform | web backend of `qplatform`, `src/lib.rs` web entry + loop, `src/platform.rs`, `web/`, `tools/webbuild` | `cargo check --offline --target wasm32-unknown-unknown -p qplatform` passes; after merging the other streams the whole root package builds for wasm and `tools/webbuild/build.sh` produces a loadable page |
