//! Wall-clock and monotonic time that also works on `wasm32-unknown-unknown`,
//! where `std::time::Instant::now` / `SystemTime::now` panic.
pub use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
pub use std::time::{Instant, SystemTime, UNIX_EPOCH};
#[cfg(target_arch = "wasm32")]
pub use web_time::{Instant, SystemTime, UNIX_EPOCH};
