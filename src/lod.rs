// Copyright (c) Rune Skovbo Johansen
// SPDX-License-Identifier: MPL-2.0
//! Sparse coverage selection adapted from the C# TerrainLODManager sample.
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
pub const MAX_TILES: usize = 8192;
pub const MAX_WORK: usize = 262_144;
/// Level zero is finest. Parent coordinates use Euclidean floor division.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct LodTile {
    pub domain: u32,
    pub level: u8,
    pub x: i32,
    pub z: i32,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LodSelection {
    pub active: Vec<LodTile>,
    pub missing: Vec<LodTile>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LodError {
    InvalidLevel,
    CoordinateOverflow,
    OverlappingRoots,
    InputLimit,
    WorkLimit,
}
impl fmt::Display for LodError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for LodError {}
fn validate(tile: LodTile) -> Result<(), LodError> {
    if tile.level > 30 {
        return Err(LodError::InvalidLevel);
    }
    let scale = 1_i64 << tile.level;
    for coordinate in [tile.x, tile.z] {
        let min = i64::from(coordinate) * scale;
        let max = min + scale - 1;
        if min < i64::from(i32::MIN) || max > i64::from(i32::MAX) {
            return Err(LodError::CoordinateOverflow);
        }
    }
    Ok(())
}
fn parent(tile: LodTile) -> LodTile {
    LodTile {
        level: tile.level + 1,
        x: tile.x.div_euclid(2),
        z: tile.z.div_euclid(2),
        ..tile
    }
}
fn children(tile: LodTile) -> [LodTile; 4] {
    let child = LodTile {
        level: tile.level - 1,
        x: tile.x * 2,
        z: tile.z * 2,
        ..tile
    };
    [
        child,
        LodTile {
            x: child.x + 1,
            ..child
        },
        LodTile {
            z: child.z + 1,
            ..child
        },
        LodTile {
            x: child.x + 1,
            z: child.z + 1,
            ..child
        },
    ]
}
fn charge(work: &mut usize) -> Result<(), LodError> {
    *work += 1;
    if *work > MAX_WORK {
        Err(LodError::WorkLimit)
    } else {
        Ok(())
    }
}
/// An available parent yields only to four ready immediate children. Roots
/// define the coverage boundary; coarser tiles and outside domains are ignored.
/// Missing branches are reported sparsely. Duplicate available tiles coalesce.
pub fn select_coverage(available: &[LodTile], roots: &[LodTile]) -> Result<LodSelection, LodError> {
    if available.len() > MAX_TILES || roots.len() > MAX_TILES {
        return Err(LodError::InputLimit);
    }
    for &tile in available.iter().chain(roots) {
        validate(tile)?;
    }
    let mut work = 0;
    let root_set: BTreeSet<_> = roots.iter().copied().collect();
    if root_set.len() != roots.len() {
        return Err(LodError::OverlappingRoots);
    }
    let mut max_levels = BTreeMap::new();
    for &root in &root_set {
        max_levels
            .entry(root.domain)
            .and_modify(|level: &mut u8| *level = (*level).max(root.level))
            .or_insert(root.level);
        let mut ancestor = root;
        while ancestor.level < 30 {
            charge(&mut work)?;
            ancestor = parent(ancestor);
            if root_set.contains(&ancestor) {
                return Err(LodError::OverlappingRoots);
            }
        }
    }
    let available_set: BTreeSet<_> = available.iter().copied().collect();
    let mut occupied = BTreeSet::new();
    for &tile in &available_set {
        if let Some(&max_level) = max_levels.get(&tile.domain) {
            let mut ancestor = tile;
            if ancestor.level <= max_level {
                charge(&mut work)?;
                occupied.insert(ancestor);
                while ancestor.level < max_level {
                    charge(&mut work)?;
                    ancestor = parent(ancestor);
                    occupied.insert(ancestor);
                }
            }
        }
    }
    let mut result = LodSelection::default();
    let mut pending: Vec<_> = root_set.into_iter().collect();
    while let Some(tile) = pending.pop() {
        charge(&mut work)?;
        if !occupied.contains(&tile) {
            result.missing.push(tile);
        } else if available_set.contains(&tile) {
            if tile.level > 0 {
                let children = children(tile);
                if children.iter().all(|child| available_set.contains(child)) {
                    pending.extend(children);
                    continue;
                }
            }
            result.active.push(tile);
        } else if tile.level == 0 {
            result.missing.push(tile);
        } else {
            pending.extend(children(tile));
        }
    }
    result.active.sort_unstable();
    result.missing.sort_unstable();
    Ok(result)
}
