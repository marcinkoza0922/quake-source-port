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
