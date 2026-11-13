//! `qinspect portals FILE..` (also bare FILE..): renderer-matched teleporter sheets, GPU-free.
pub fn run(files: &[String]) -> Result<(), String> {
    if files.is_empty() {
        return Err("usage: qinspect [portals] FILE..".into());
    }
    for path in files {
        let data = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        let bsp = qformats::bsp::Bsp::parse(&data).map_err(|e| format!("{path}: {e}"))?;
        for (i, p) in qrender::portals::inspect(&bsp).iter().enumerate() {
            let distance = [110.0, 80.0, 55.0, 35.0, 20.0, 10.0]
                .into_iter()
                .find(|d| {
                    // Sample the complete line: an empty camera leaf alone can lie
                    // behind a wall on a portal's inaccessible back side.
                    (1..=20).all(|step| {
                        let at = d * step as f32 / 20.0;
                        let eye = std::array::from_fn(|k| p.center[k] + p.normal[k] * at);
                        !solid(&bsp, eye)
                    })
                })
                .unwrap_or(-1.0);
            println!(
                "{} {} {} {} {} {} {} {} {}",
                i,
                p.center[0],
                p.center[1],
                p.center[2],
                p.normal[0],
                p.normal[1],
                p.normal[2],
                p.yaw,
                distance
            );
        }
    }
    Ok(())
}

/// Check the render BSP hull; solid has Quake contents code -2.
fn solid(bsp: &qformats::bsp::Bsp, point: [f32; 3]) -> bool {
    let mut index = 0i32;
    for _ in 0..bsp.nodes.len() + 1 {
        if index < 0 {
            return bsp
                .leafs
                .get((-index - 1) as usize)
                .is_none_or(|l| l.contents == -2);
        }
        let Some(node) = bsp.nodes.get(index as usize) else {
            return true;
        };
        let Some(plane) = bsp.planes.get(node.plane as usize) else {
            return true;
        };
        let distance: f32 = point.iter().zip(plane.normal).map(|(a, b)| a * b).sum();
        index = node.children[usize::from(distance < plane.dist)];
    }
    true
}
