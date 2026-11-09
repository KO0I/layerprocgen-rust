// Copyright (c) Rune Skovbo Johansen
// SPDX-License-Identifier: MPL-2.0
//! Retained paths adapted from the C# terrain sample's CultivationLayer.
use crate::location::{Location, LocationConnection, LocationConnections};
use crate::path::{Path, PathError, PathOptions, Point2, TerrainSample, plan_path};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::{Rc, Weak};
use std::{error::Error, fmt};

type RoadKey = (String, String);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PatchBounds {
    pub min: Point2,
    pub max: Point2,
}
impl PatchBounds {
    pub fn new(min: Point2, max: Point2) -> Result<Self, CultivationError> {
        let result = Self { min, max };
        if !result.valid() {
            return Err(CultivationError::InvalidInput);
        }
        Ok(result)
    }
    fn valid(self) -> bool {
        [self.min.x, self.min.z, self.max.x, self.max.z]
            .iter()
            .all(|v| v.is_finite())
            && self.min.x < self.max.x
            && self.min.z < self.max.z
    }
    fn overlaps(self, other: Self) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.z <= other.max.z
            && self.max.z >= other.min.z
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Road {
    pub connection: LocationConnection,
    pub path: Path,
    pub bounds: PatchBounds,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CultivationError {
    InvalidInput,
    MissingLocation(String),
    Planning {
        connection: LocationConnection,
        reason: PathError,
    },
}
impl fmt::Display for CultivationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput => {
                f.write_str("invalid cultivation locations, connections, bounds or options")
            }
            Self::MissingLocation(id) => write!(f, "connection references missing location {id}"),
            Self::Planning { connection, reason } => {
                write!(f, "road {} to {}: {reason}", connection.a, connection.b)
            }
        }
    }
}
impl Error for CultivationError {}

struct RetainedRoad {
    road: Rc<Road>,
    users: usize,
}
#[derive(Default)]
struct Retained {
    roads: BTreeMap<RoadKey, RetainedRoad>,
}

/// Demand for candidate roads touching one terrain patch. Drop to unload.
/// A returned `Rc<Road>` can outlive demand without remaining in the layer cache.
pub struct CultivationPatch {
    retained: Weak<RefCell<Retained>>,
    keys: Vec<RoadKey>,
}
impl Drop for CultivationPatch {
    fn drop(&mut self) {
        let Some(retained) = self.retained.upgrade() else {
            return;
        };
        let mut state = retained.borrow_mut();
        for key in &self.keys {
            let entry = state.roads.get_mut(key).expect("live patch road");
            entry.users -= 1;
            if entry.users == 0 {
                state.roads.remove(key);
            }
        }
    }
}

/// One immutable terrain revision and shared planar chart.
/// The sampler must continue to describe that revision for this layer's lifetime.
/// Use caller-resolved building approach points as location positions.
pub struct CultivationLayer<S> {
    candidates: Vec<(LocationConnection, Point2, Point2, PatchBounds)>,
    options: PathOptions,
    sample: S,
    retained: Rc<RefCell<Retained>>,
}
impl<S: FnMut(Point2) -> Option<TerrainSample>> CultivationLayer<S> {
    pub fn new(
        locations: &[Location],
        connections: &LocationConnections,
        options: PathOptions,
        sample: S,
    ) -> Result<Self, CultivationError> {
        let mut sites = BTreeMap::new();
        for site in locations {
            if site.id.is_empty()
                || !site.position.x.is_finite()
                || !site.position.z.is_finite()
                || sites.insert(site.id.clone(), site.position).is_some()
            {
                return Err(CultivationError::InvalidInput);
            }
        }
        let mut candidates = BTreeMap::new();
        for edge in &connections.connections {
            if edge.a >= edge.b {
                return Err(CultivationError::InvalidInput);
            }
            let a = *sites
                .get(&edge.a)
                .ok_or_else(|| CultivationError::MissingLocation(edge.a.clone()))?;
            let b = *sites
                .get(&edge.b)
                .ok_or_else(|| CultivationError::MissingLocation(edge.b.clone()))?;
            if !crate::path::valid_input(a, b, options) {
                return Err(CultivationError::InvalidInput);
            }
            let bounds = segment_bounds(a, b, options.corridor + radius(options));
            if !bounds.valid() {
                return Err(CultivationError::InvalidInput);
            }
            if candidates
                .insert(
                    (edge.a.clone(), edge.b.clone()),
                    (edge.clone(), a, b, bounds),
                )
                .is_some()
            {
                return Err(CultivationError::InvalidInput);
            }
        }
        if !crate::path::valid_input(Point2 { x: 0., z: 0. }, Point2 { x: 1., z: 0. }, options) {
            return Err(CultivationError::InvalidInput);
        }
        Ok(Self {
            candidates: candidates.into_values().collect(),
            options,
            sample,
            retained: Rc::new(RefCell::new(Retained::default())),
        })
    }

    /// Plans missing candidates before publishing any demand. An error rolls back
    /// this request; existing leases and roads remain valid.
    pub fn retain_patch(
        &mut self,
        bounds: PatchBounds,
    ) -> Result<CultivationPatch, CultivationError> {
        if !bounds.valid() {
            return Err(CultivationError::InvalidInput);
        }
        let mut staged = Vec::new();
        for (edge, a, b, envelope) in &self.candidates {
            if !envelope.overlaps(bounds) {
                continue;
            }
            let key = (edge.a.clone(), edge.b.clone());
            let existing = self
                .retained
                .borrow()
                .roads
                .get(&key)
                .map(|entry| Rc::clone(&entry.road));
            let road = if let Some(road) = existing {
                road
            } else {
                let path = plan_path(&mut self.sample, *a, *b, self.options).map_err(|reason| {
                    CultivationError::Planning {
                        connection: edge.clone(),
                        reason,
                    }
                })?;
                let mut road_bounds = segment_bounds(
                    path.points[0].position,
                    path.points[0].position,
                    radius(self.options),
                );
                for point in &path.points {
                    let p = point.position;
                    road_bounds.min.x = road_bounds.min.x.min(p.x - radius(self.options));
                    road_bounds.min.z = road_bounds.min.z.min(p.z - radius(self.options));
                    road_bounds.max.x = road_bounds.max.x.max(p.x + radius(self.options));
                    road_bounds.max.z = road_bounds.max.z.max(p.z + radius(self.options));
                }
                Rc::new(Road {
                    connection: edge.clone(),
                    path,
                    bounds: road_bounds,
                })
            };
            staged.push((key, road));
        }
        let mut retained = self.retained.borrow_mut();
        let mut keys = Vec::with_capacity(staged.len());
        for (key, road) in staged {
            let entry = retained
                .roads
                .entry(key.clone())
                .or_insert(RetainedRoad { road, users: 0 });
            entry.users += 1;
            keys.push(key);
        }
        Ok(CultivationPatch {
            retained: Rc::downgrade(&self.retained),
            keys,
        })
    }

    /// Returns retained roads whose full-width bounding boxes touch the patch.
    /// This query does not itself acquire streaming demand.
    pub fn roads_overlapping(
        &self,
        bounds: PatchBounds,
    ) -> Result<Vec<Rc<Road>>, CultivationError> {
        if !bounds.valid() {
            return Err(CultivationError::InvalidInput);
        }
        Ok(self
            .retained
            .borrow()
            .roads
            .values()
            .filter(|entry| entry.road.bounds.overlaps(bounds))
            .map(|entry| Rc::clone(&entry.road))
            .collect())
    }
    /// Returns the complete immutable road set for composition snapshots.
    pub fn retained_roads(&self) -> Vec<Rc<Road>> {
        self.retained
            .borrow()
            .roads
            .values()
            .map(|entry| Rc::clone(&entry.road))
            .collect()
    }

    pub fn retained_road_count(&self) -> usize {
        self.retained.borrow().roads.len()
    }
}
fn radius(options: PathOptions) -> f64 {
    (options.width.surface * 0.5 + options.width.shoulder).max(options.width.material * 0.5)
}
fn segment_bounds(a: Point2, b: Point2, padding: f64) -> PatchBounds {
    PatchBounds {
        min: Point2 {
            x: a.x.min(b.x) - padding,
            z: a.z.min(b.z) - padding,
        },
        max: Point2 {
            x: a.x.max(b.x) + padding,
            z: a.z.max(b.z) + padding,
        },
    }
}
