//! Node graph checks: the engine walks BSP and clip nodes recursively, so a
//! file with a cycle would never return.

use super::{ClipNode, FMT, Node};
use crate::{Error, Result};

/// Deeper trees than any compiler writes; keeps the recursive walks on the stack.
const MAX_DEPTH: u32 = 4096;

pub(super) fn check(nodes: &[Node], clipnodes: &[ClipNode], num_planes: usize) -> Result<()> {
    check_tree("nodes", &nodes.iter().map(|n| n.children).collect::<Vec<_>>())?;
    check_tree("clipnodes", &clipnodes.iter().map(|n| n.children).collect::<Vec<_>>())?;
    match nodes.iter().find(|n| usize::try_from(n.plane).map_or(true, |p| p >= num_planes)) {
        Some(n) => Err(Error::new(FMT, format!("bad node plane {}", n.plane))),
        None => Ok(()),
    }
}

/// Errors if the nodes contain a cycle or a path longer than [`MAX_DEPTH`].
/// Negative and out-of-range children are leaves here.
pub(super) fn check_tree(what: &str, children: &[[i32; 2]]) -> Result<()> {
    const OPEN: u32 = u32::MAX;
    // 0 = not visited, OPEN = on the current path, else longest path below + 1
    let mut height = vec![0u32; children.len()];
    let mut stack = Vec::new();
    for root in 0..children.len() {
        if height[root] != 0 {
            continue;
        }
        height[root] = OPEN;
        stack.push((root, 0));
        while let Some((n, side)) = stack.last_mut() {
            let n = *n;
            if *side < 2 {
                let c = children[n][*side] as usize;
                *side += 1;
                match height.get(c) {
                    Some(0) => {
                        height[c] = OPEN;
                        stack.push((c, 0));
                    }
                    Some(&OPEN) => return Err(Error::new(FMT, format!("cycle in {what} at {c}"))),
                    _ => {}
                }
                continue;
            }
            stack.pop();
            let below = children[n].iter().filter_map(|&c| height.get(c as usize)).max();
            height[n] = below.copied().unwrap_or(0) + 1;
            if height[n] > MAX_DEPTH {
                return Err(Error::new(FMT, format!("{what} deeper than {MAX_DEPTH}")));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trees_and_shared_subtrees_pass() {
        assert!(check_tree("nodes", &[]).is_ok());
        assert!(check_tree("nodes", &[[1, 2], [2, -1], [-1, -2]]).is_ok());
        assert!(check_tree("nodes", &[[99, -1]]).is_ok());
    }

    #[test]
    fn cycles_and_endless_chains_fail() {
        assert!(check_tree("nodes", &[[0, -1]]).is_err());
        assert!(check_tree("nodes", &[[1, -1], [-1, 2], [0, -1]]).is_err());
        let chain: Vec<[i32; 2]> = (1..=MAX_DEPTH as i32 + 1).map(|i| [i, -1]).collect();
        assert!(check_tree("nodes", &chain).is_err());
    }
}
