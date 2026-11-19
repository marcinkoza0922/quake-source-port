use std::sync::Arc;

use qsound::*;

fn tone(name: &str, len: usize) -> Arc<Sfx> {
    Arc::new(Sfx {
        name: name.into(),
        data: vec![10000; len],
        loop_start: None,
    })
}

fn sys() -> (SoundSystem, Mixer) {
    SoundSystem::new(44100, |_| None)
}

fn start(s: &SoundSystem, ent: i32, chan: i32, sfx: &Arc<Sfx>, origin: Vec3) {
    s.event(SoundEvent::Start {
        entity: ent,
        channel: chan,
        sfx: sfx.clone(),
        origin,
        volume: 1.0,
        attenuation: 1.0,
    });
}

fn listener(s: &SoundSystem) {
    s.update(Listener {
        origin: [0.0; 3],
        right: [0.0, -1.0, 0.0],
        view_entity: 1,
        ..Default::default()
    });
}

#[test]
fn panning() {
    let (s, mut m) = sys();
    listener(&s);
    let t = tone("t", 44100);
    start(&s, 2, 1, &t, [0.0, -100.0, 0.0]); // to the right
    start(&s, 3, 1, &t, [0.0, 100.0, 0.0]); // to the left
    start(&s, 4, 1, &t, [100.0, 0.0, 0.0]); // ahead
    m.process_commands();
    let (l, r) = m.channel_volume(2, 1).unwrap();
    assert!(r > l && l == 0, "{l} {r}");
    let (l, r) = m.channel_volume(3, 1).unwrap();
    assert!(l > r && r == 0);
    let (l, r) = m.channel_volume(4, 1).unwrap();
    assert_eq!(l, r);
    // Far beyond the clip distance: inaudible, never started.
    start(&s, 5, 1, &t, [5000.0, 0.0, 0.0]);
    m.process_commands();
    assert!(m.channel_volume(5, 1).is_none());
    // Painted output is louder on the right for a right-only source.
    let (s, mut m) = sys();
    listener(&s);
    start(&s, 2, 1, &t, [0.0, -100.0, 0.0]);
    let mut out = vec![0i16; 512];
    m.paint(&mut out);
    assert!(out[1] > 0 && out[0] == 0);
}

#[test]
fn entity_channel_override() {
    let (s, mut m) = sys();
    listener(&s);
    let (a, b) = (tone("a", 44100), tone("b", 44100));
    start(&s, 2, 1, &a, [10.0, 0.0, 0.0]);
    start(&s, 2, 1, &b, [10.0, 0.0, 0.0]); // replaces
    start(&s, 2, 2, &a, [10.0, 0.0, 0.0]); // other channel, kept
    start(&s, 2, 0, &a, [10.0, 0.0, 0.0]); // channel 0 never overrides
    start(&s, 2, 0, &a, [10.0, 0.0, 0.0]);
    m.process_commands();
    let mut ch = m.dynamic_channels();
    ch.sort();
    assert_eq!(ch.len(), 4);
    assert!(ch.contains(&(2, 1, "b".into())));
    // -1 overrides any channel of the entity (first match).
    start(&s, 2, -1, &a, [10.0, 0.0, 0.0]);
    m.process_commands();
    assert_eq!(m.dynamic_channels().len(), 4);
    s.event(SoundEvent::Stop {
        entity: 2,
        channel: 2,
    });
    m.process_commands();
    assert!(m.channel_volume(2, 2).is_none());
    // Monster sounds don't override player sounds when channels are full.
    let (s, mut m) = sys();
    listener(&s);
    for e in 0..MAX_DYNAMIC_CHANNELS as i32 {
        let _ = e;
        start(&s, 1, 0, &a, [0.0; 3]);
    }
    start(&s, 9, 1, &b, [10.0, 0.0, 0.0]);
    m.process_commands();
    assert!(m.channel_volume(9, 1).is_none());
}

#[test]
fn static_and_ambient_loop() {
    let (s, mut m) = sys();
    listener(&s);
    s.event(SoundEvent::Static {
        sfx: tone("s", 100),
        origin: [50.0, 0.0, 0.0],
        volume: 1.0,
        attenuation: 1.0,
    });
    let mut out = vec![0i16; 2000];
    m.paint(&mut out);
    m.paint(&mut out);
    assert!(out[1998] != 0, "static sound loops");
    assert_eq!(m.active_channels(), 1);
}

fn pak_fs() -> Option<qcore::fs::Fs> {
    let base: std::path::PathBuf = std::env::var("QUAKE_BASEDIR")
        .map(Into::into)
        .unwrap_or_else(|_| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../vkQuake directory with maps")
        });
    let mut fs = qcore::fs::Fs::new(base);
    fs.add_game_dir("id1");
    fs.exists("sound/weapons/guncock.wav").then_some(fs)
}

#[test]
fn mix_id1_sounds() {
    let Some(fs) = pak_fs() else {
        eprintln!("skip: no id1");
        return;
    };
    let (mut s, mut m) = SoundSystem::new(48000, move |p| fs.open(p));
    let gun = s.precache("weapons/guncock.wav").expect("guncock");
    let water = s.precache("ambience/water1.wav").expect("water");
    assert!(water.loop_start.is_some());
    assert!(s.precache("nope/missing.wav").is_none());
    // 11025 Hz source → 48000 Hz output.
    assert!(gun.data.len() > 40000);
    s.update(Listener {
        right: [0.0, -1.0, 0.0],
        view_entity: 1,
        ambient_levels: Some([255, 0, 0, 0]),
        frametime: 1.0,
        ..Default::default()
    });
    s.event(SoundEvent::Start {
        entity: 1,
        channel: 1,
        sfx: gun,
        origin: [0.0; 3],
        volume: 1.0,
        attenuation: 1.0,
    });
    let mut out = vec![0i16; 4800];
    let mut energy = 0i64;
    for _ in 0..20 {
        m.paint(&mut out);
        energy += out.iter().map(|&x| i64::from(x).abs()).sum::<i64>();
    }
    assert!(energy > 0);
    assert!(m.active_channels() >= 1, "ambient water keeps playing");
    assert!(!s.play_cd_track(99, true));
}

#[test]
fn music_stream() {
    let Some(fs) = pak_fs() else { return };
    if !fs.exists("music/track02.ogg") {
        return;
    }
    let (mut s, mut m) = SoundSystem::new(48000, move |p| fs.open(p));
    assert!(s.play_cd_track(2, true));
    let mut out = vec![0i16; 9600];
    let mut energy = 0i64;
    for _ in 0..50 {
        m.paint(&mut out);
        energy += out.iter().map(|&x| i64::from(x).abs()).sum::<i64>();
    }
    assert!(energy > 0 && m.music_playing());
    s.stop_music();
    m.paint(&mut out);
    assert!(!m.music_playing());
}

#[test]
fn split_screen_listeners() {
    let (s, mut m) = sys();
    let mut others = [([0.0; 3], 0); MAX_OTHER_LISTENERS];
    others[0] = ([4000.0, 0.0, 0.0], 7);
    s.update(Listener { origin: [0.0; 3], right: [0.0, -1.0, 0.0], view_entity: 1, others, num_others: 1, ..Default::default() });
    let t = tone("t", 44100);
    // Out of player 1's range but next to player 2: played, centred.
    start(&s, 5, 1, &t, [4050.0, 0.0, 0.0]);
    // Player 2's own sound: full volume.
    start(&s, 7, 1, &t, [4000.0, 0.0, 0.0]);
    // Heard by nobody.
    start(&s, 6, 1, &t, [2000.0, 3000.0, 0.0]);
    m.process_commands();
    let (l, r) = m.channel_volume(5, 1).unwrap();
    assert!(l > 0 && l == r, "{l} {r}");
    let (l2, r2) = m.channel_volume(7, 1).unwrap();
    assert!(l2 > l && l2 == r2);
    assert!(m.channel_volume(6, 1).is_none());
}

fn sine_sfx(name: &str, freq: f32, rate: u32, len: usize) -> Arc<Sfx> {
    let data = (0..len)
        .map(|i| ((std::f32::consts::TAU * freq * i as f32 / rate as f32).sin() * 14000.0) as i16)
        .collect();
    Arc::new(Sfx { name: name.into(), data, loop_start: None })
}

#[test]
fn hrtf_produces_binaural_output_with_head_shadow() {
    let (s, mut m) = sys();
    s.set_hrtf(true);
    s.update(Listener {
        origin: [0.0; 3],
        forward: [1.0, 0.0, 0.0],
        right: [0.0, -1.0, 0.0],
        up: [0.0, 0.0, 1.0],
        view_entity: 1,
        ..Default::default()
    });
    let hi = sine_sfx("ogre/ogwake.wav", 4000.0, 44100, 4096);
    // Sound directly to the right.
    start(&s, 2, 1, &hi, [0.0, -100.0, 0.0]);
    let mut out = vec![0i16; 2048];
    m.paint(&mut out);
    let left_e: i64 = out.chunks_exact(2).skip(64).map(|f| i64::from(f[0].abs())).sum();
    let right_e: i64 = out.chunks_exact(2).skip(64).map(|f| i64::from(f[1].abs())).sum();
    assert!(right_e > left_e * 2, "expected strong right bias: l={left_e} r={right_e}");
    assert!(left_e > 0, "HRTF keeps shadowed contralateral ear audible");
}

#[test]
fn occlusion_muffles_and_attenuates_blocked_sounds() {
    let hi = sine_sfx("shambler/sattck1.wav", 4500.0, 44100, 4096);
    let measure = |occluded: bool| -> i64 {
        let (s, mut m) = sys();
        s.set_occlusion(true);
        listener(&s);
        start(&s, 2, 1, &hi, [100.0, 0.0, 0.0]);
        if occluded {
            s.set_channel_occlusion(2, 1, 1.0);
        }
        let mut out = vec![0i16; 4096];
        m.paint(&mut out);
        out.iter().skip(256).map(|&x| i64::from(x.abs())).sum()
    };
    let clear = measure(false);
    let blocked = measure(true);
    assert!(clear > blocked * 5, "occluded HF should drop sharply: clear={clear} blocked={blocked}");
    assert!(blocked > 0, "occluded sound stays faintly audible");
}

#[test]
fn environmental_reverb_leaves_ui_dry_and_adds_world_tail() {
    let (s, mut m) = sys();
    listener(&s);
    s.set_reverb(ReverbParams { enabled: true, room_size: 0.8, damping: 0.25, wet: 0.45 });
    let short_shot = sine_sfx("weapons/guncock.wav", 600.0, 44100, 512);
    start(&s, 2, 1, &short_shot, [80.0, 0.0, 0.0]);
    let mut out = vec![0i16; 2048];
    m.paint(&mut out);
    // Sound has finished; subsequent buffer should still contain reverb tail.
    let mut tail = vec![0i16; 4096];
    m.paint(&mut tail);
    let tail_energy: i64 = tail.iter().map(|&x| i64::from(x.abs())).sum();
    assert!(tail_energy > 2000, "expected reverb tail after gunshot, got {tail_energy}");

    // UI sound produces no reverb tail.
    let (s, mut m) = sys();
    listener(&s);
    s.set_reverb(ReverbParams { enabled: true, room_size: 0.8, damping: 0.25, wet: 0.45 });
    let click = sine_sfx("misc/menu1.wav", 600.0, 44100, 512);
    s.event(SoundEvent::Local(click));
    m.paint(&mut out);
    m.paint(&mut tail);
    let ui_tail: i64 = tail.iter().map(|&x| i64::from(x.abs())).sum();
    assert_eq!(ui_tail, 0, "UI sounds must stay dry");
}

#[test]
fn category_volumes_scale_each_bucket_independently() {
    let (s, mut m) = sys();
    listener(&s);
    s.set_category_volumes(CategoryVolumes {
        ambient: 0.0,
        weapons: 1.0,
        monsters: 0.0,
        world: 1.0,
    });
    let monster = tone("ogre/ogwake.wav", 4096);
    start(&s, 2, 1, &monster, [50.0, 0.0, 0.0]);
    let mut out = vec![0i16; 1024];
    m.paint(&mut out);
    assert!(out.iter().all(|&x| x == 0), "muted monster category should be silent");

    let weapon = tone("weapons/guncock.wav", 4096);
    start(&s, 3, 1, &weapon, [50.0, 0.0, 0.0]);
    m.paint(&mut out);
    assert!(out.iter().any(|&x| x != 0), "unmuted weapon category should play");
}

#[test]
fn underwater_muffling_respects_water_fx_toggle() {
    let hi = sine_sfx("weapons/guncock.wav", 4500.0, 44100, 24000);
    let run = |water_fx: bool| -> i64 {
        let (s, mut m) = sys();
        s.set_water_fx(water_fx);
        s.update(Listener {
            origin: [0.0; 3],
            right: [0.0, -1.0, 0.0],
            view_entity: 1,
            underwater: true,
            ..Default::default()
        });
        start(&s, 1, 1, &hi, [0.0; 3]);
        let mut out = vec![0i16; 16384];
        m.paint(&mut out);
        // Second block after fade-in completes while sine is still playing.
        m.paint(&mut out);
        out.iter().map(|&x| i64::from(x.abs())).sum()
    };
    let dry = run(false);
    let muffled = run(true);
    assert!(dry > muffled * 10, "underwater filter should strongly muffle HF: dry={dry} muffled={muffled}");
}

fn wav_bytes(freq: f32, rate: u32, frames: usize) -> Vec<u8> {
    let data_bytes = (frames * 2) as u32;
    let mut b = Vec::with_capacity(44 + data_bytes as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_bytes.to_le_bytes());
    for i in 0..frames {
        let s = ((std::f32::consts::TAU * freq * i as f32 / rate as f32).sin() * 14000.0) as i16;
        b.extend_from_slice(&s.to_le_bytes());
    }
    b
}

#[test]
fn music_crossfade_blends_outgoing_track() {
    let (s, mut m) = sys();
    s.set_music_crossfade(0.5);
    let track_a = wav_bytes(300.0, 44100, 44100);
    assert!(s.play_music_bytes(track_a, "wav", true));
    let mut out = vec![0i16; 2048];
    // Let track A play and ramp in.
    for _ in 0..25 {
        m.paint(&mut out);
    }
    assert!(m.music_playing());
    // Stop music: music_playing becomes false immediately, while fade-out still produces non-zero audio.
    s.stop_music();
    m.paint(&mut out);
    assert!(!m.music_playing());
    let fade_energy: i64 = out.iter().map(|&x| i64::from(x.abs())).sum();
    assert!(fade_energy > 1000, "expected smooth fade-out tail, got {fade_energy}");
    // After crossfade completes, output returns to silence.
    for _ in 0..30 {
        m.paint(&mut out);
    }
    let after: i64 = out.iter().map(|&x| i64::from(x.abs())).sum();
    assert_eq!(after, 0, "expected silence after crossfade finishes, got {after}");
}
