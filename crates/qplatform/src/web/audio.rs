//! WebAudio playback through a `ScriptProcessorNode` (main-thread callback,
//! no worklet or shared memory needed). The context starts suspended until
//! a user gesture; listeners resume it on the first key, click or touch.

use std::cell::RefCell;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{AudioContext, AudioContextOptions, AudioProcessingEvent, EventTarget, ScriptProcessorNode};

use super::{Listener, js_err, window};
use crate::Result;

type Fill = Box<dyn FnMut(&mut [i16]) + Send>;

/// Frames per callback: ~43 ms at 48 kHz, enough slack for a busy main thread.
const FRAMES: u32 = 2048;

/// Playback stream; the callback fills interleaved stereo i16 frames.
pub struct AudioStream {
    ctx: AudioContext,
    node: ScriptProcessorNode,
    _cb: Closure<dyn FnMut(AudioProcessingEvent)>,
    _unlock: Vec<Listener>,
    freq: u32,
}

impl AudioStream {
    pub(super) fn open(freq: u32, fill: Fill) -> Result<Self> {
        let opts = AudioContextOptions::new();
        opts.set_sample_rate(freq as f32);
        let ctx = AudioContext::new_with_context_options(&opts).map_err(js_err)?;
        let node = ctx
            .create_script_processor_with_buffer_size_and_number_of_input_channels_and_number_of_output_channels(FRAMES, 0, 2)
            .map_err(js_err)?;
        let fill = RefCell::new(fill);
        let mut pcm: Vec<i16> = Vec::new();
        let (mut l, mut r): (Vec<f32>, Vec<f32>) = (Vec::new(), Vec::new());
        let cb = Closure::<dyn FnMut(AudioProcessingEvent)>::new(move |e: AudioProcessingEvent| {
            let Ok(out) = e.output_buffer() else { return };
            let n = out.length() as usize;
            pcm.clear();
            pcm.resize(n * 2, 0);
            (fill.borrow_mut())(&mut pcm);
            l.clear();
            r.clear();
            for f in pcm.as_chunks::<2>().0 {
                l.push(f[0] as f32 / 32768.0);
                r.push(f[1] as f32 / 32768.0);
            }
            let _ = out.copy_to_channel(&l, 0);
            let _ = out.copy_to_channel(&r, 1);
        });
        node.set_onaudioprocess(Some(cb.as_ref().unchecked_ref()));
        node.connect_with_audio_node(&ctx.destination()).map_err(js_err)?;
        let w: EventTarget = window()?.into();
        let mut unlock = Vec::new();
        for name in ["keydown", "mousedown", "touchstart"] {
            let c = ctx.clone();
            unlock.push(Listener::new(&w, name, move |_| {
                let _ = c.resume();
            })?);
        }
        let _ = ctx.suspend();
        Ok(Self { ctx, node, _cb: cb, _unlock: unlock, freq })
    }

    /// Start (or resume) playback; may wait for a user gesture.
    pub fn resume(&self) -> Result<()> {
        self.ctx.resume().map(drop).map_err(js_err)
    }

    pub fn pause(&self) -> Result<()> {
        self.ctx.suspend().map(drop).map_err(js_err)
    }

    pub fn freq(&self) -> u32 {
        self.freq
    }
}

impl Drop for AudioStream {
    fn drop(&mut self) {
        self.node.set_onaudioprocess(None);
        let _ = self.node.disconnect();
        let _ = self.ctx.close();
    }
}
