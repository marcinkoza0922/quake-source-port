use super::*;

const MESH: &str = r#"MD5Version 10 // comment
commandline ""
numJoints 2
numMeshes 1
joints {
  "Root" -1 ( 0 0 0 ) ( 0 0 0 )
  "Arm" 0 ( 10 0 0 ) ( 0 0 0 )
}
mesh {
  shader "thing"
  numverts 3
  vert 0 ( 0 0 ) 0 1
  vert 1 ( 1 0 ) 1 1
  vert 2 ( 0 1 ) 2 2
  numtris 1
  tri 0 0 1 2
  numweights 4
  weight 0 0 1.0 ( 0 0 0 )
  weight 1 1 1.0 ( 0 0 0 )
  weight 2 0 0.5 ( 0 8 0 )
  weight 3 1 0.5 ( -10 8 0 )
}
"#;

const ANIM: &str = r#"MD5Version 10
commandline ""
numFrames 2
numJoints 2
frameRate 24
numAnimatedComponents 1
hierarchy {
  "Root" -1 0 0
  "Arm" 0 1 0
}
bounds {
  ( 0 0 0 ) ( 1 1 1 )
  ( 0 0 0 ) ( 1 1 1 )
}
baseframe {
  ( 0 0 0 ) ( 0 0 0 )
  ( 10 0 0 ) ( 0 0 0 )
}
frame 0 {
  10
}
frame 1 {
  20
}
"#;

fn pos(m: &Md5, frame: usize, v: usize) -> [f32; 3] {
    let s = &m.md3.surfaces[0];
    s.verts[frame * s.num_verts + v].xyz.map(|x| x as f32 * XYZ_SCALE)
}

#[test]
fn bind_pose_without_anim() {
    let m = Md5::parse(MESH.as_bytes(), None).unwrap();
    assert_eq!(m.shaders, ["thing"]);
    assert_eq!(m.md3.frames.len(), 1);
    assert_eq!(pos(&m, 0, 0), [0.0, 0.0, 0.0]);
    assert_eq!(pos(&m, 0, 1), [10.0, 0.0, 0.0]);
    assert_eq!(pos(&m, 0, 2), [0.0, 8.0, 0.0]);
}

#[test]
fn frames_move_animated_joints() {
    let m = Md5::parse(MESH.as_bytes(), Some(ANIM.as_bytes())).unwrap();
    assert_eq!(m.md3.frames.len(), 2);
    assert_eq!(pos(&m, 0, 1), [10.0, 0.0, 0.0]);
    assert_eq!(pos(&m, 1, 1), [20.0, 0.0, 0.0]);
    // the half-weighted vertex follows the arm halfway
    assert_eq!(pos(&m, 1, 2), [5.0, 8.0, 0.0]);
}

#[test]
fn normal_round_trip() {
    for n in [[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.6, 0.0, -0.8]] {
        let [lat, lng] = encode_normal(n).map(|x| x as f32 * std::f32::consts::TAU / 255.0);
        let d = [lng.cos() * lat.sin(), lng.sin() * lat.sin(), lat.cos()];
        assert!((0..3).all(|i| (d[i] - n[i]).abs() < 0.05), "{n:?} -> {d:?}");
    }
}

/// MG3's ogre_rocket: the root's mesh parent is an armature joint listed
/// last that the animation doesn't drive; the animation's own hierarchy
/// (root at -1) decides how poses chain, as in vkQuake.
#[test]
fn forward_parent_to_undriven_joint() {
    let mesh = MESH.replace(
        "numJoints 2\nnumMeshes 1\njoints {\n  \"Root\" -1 ( 0 0 0 ) ( 0 0 0 )\n  \"Arm\" 0 ( 10 0 0 ) ( 0 0 0 )\n}",
        "numJoints 3\nnumMeshes 1\njoints {\n  \"Root\" 2 ( 0 0 0 ) ( 0 0 0 )\n  \"Arm\" 0 ( 10 0 0 ) ( 0 0 0 )\n  \"Armature\" -1 ( 0 0 0 ) ( 0 0 0 )\n}",
    );
    assert!(mesh.contains("Armature"));
    let m = Md5::parse(mesh.as_bytes(), Some(ANIM.as_bytes())).unwrap();
    assert_eq!(pos(&m, 1, 1), [20.0, 0.0, 0.0]);
    assert_eq!(pos(&m, 1, 2), [5.0, 8.0, 0.0]);
    // a joint that is its own parent is still rejected
    let bad = mesh.replace("\"Armature\" -1", "\"Armature\" 2");
    assert!(Md5::parse(bad.as_bytes(), Some(ANIM.as_bytes())).is_err());
}
