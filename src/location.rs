// Copyright (c) Rune Skovbo Johansen
// SPDX-License-Identifier: MPL-2.0
//! Location connectivity adapted from the C# terrain sample's LocationLayer.
//!
//! Callers supply existing sites in one shared planar chart. IDs determine the
//! within-cell cycle order. Unlike the Unity sample, sites are never relocated.
use crate::path::Point2;
use std::collections::{BTreeMap, BTreeSet};
use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocationCell {
    pub x: i32,
    pub z: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Location {
    pub id: String,
    pub position: Point2,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocationConnection {
    pub a: String,
    pub b: String,
    pub owner: LocationCell,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocationError {
    InvalidCellSize,
    InvalidLocation,
    DuplicateId,
    CellOutOfRange,
}
impl fmt::Display for LocationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidCellSize => "location cell size must be finite and positive",
            Self::InvalidLocation => "location ID must be nonempty and coordinates finite",
            Self::DuplicateId => "location IDs must be unique",
            Self::CellOutOfRange => "location cell is outside the supported coordinate range",
        })
    }
}
impl Error for LocationError {}

/// Immutable connections for a complete supplied patch, sorted by endpoint IDs.
///
/// Missing cells are treated as empty. Streaming callers must supply a one-cell
/// halo before treating interior results as complete. Ownership is a half-open
/// grid cell containing the midpoint, including for negative coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct LocationConnections {
    pub connections: Vec<LocationConnection>,
}
impl LocationConnections {
    pub fn build(locations: &[Location], cell_size: f64) -> Result<Self, LocationError> {
        if !cell_size.is_finite() || cell_size <= 0. {
            return Err(LocationError::InvalidCellSize);
        }
        let mut cells: BTreeMap<LocationCell, Vec<&Location>> = BTreeMap::new();
        let mut ids = BTreeSet::new();
        for location in locations {
            if location.id.is_empty()
                || !location.position.x.is_finite()
                || !location.position.z.is_finite()
            {
                return Err(LocationError::InvalidLocation);
            }
            if !ids.insert(&location.id) {
                return Err(LocationError::DuplicateId);
            }
            cells
                .entry(cell(location.position, cell_size)?)
                .or_default()
                .push(location);
        }
        for sites in cells.values_mut() {
            sites.sort_by(|a, b| a.id.cmp(&b.id));
        }
        let mut edges = BTreeMap::new();
        let mut connect = |a: &Location, b: &Location| -> Result<(), LocationError> {
            let (a, b) = if a.id < b.id { (a, b) } else { (b, a) };
            let midpoint = Point2 {
                x: a.position.x * 0.5 + b.position.x * 0.5,
                z: a.position.z * 0.5 + b.position.z * 0.5,
            };
            edges.insert(
                (a.id.clone(), b.id.clone()),
                LocationConnection {
                    a: a.id.clone(),
                    b: b.id.clone(),
                    owner: cell(midpoint, cell_size)?,
                },
            );
            Ok(())
        };
        for (index, sites) in &cells {
            if sites.len() == 2 {
                connect(sites[0], sites[1])?;
            } else if sites.len() >= 3 {
                let mut longest = 0;
                for i in 1..sites.len() {
                    if distance(sites[i], sites[(i + 1) % sites.len()])
                        > distance(sites[longest], sites[(longest + 1) % sites.len()])
                    {
                        longest = i;
                    }
                }
                for i in 0..sites.len() {
                    if i != longest {
                        connect(sites[i], sites[(i + 1) % sites.len()])?;
                    }
                }
            }
            // Each adjacent pair is visited once; canonical IDs break distance ties.
            for neighbor in [
                index
                    .x
                    .checked_add(1)
                    .map(|x| LocationCell { x, z: index.z }),
                index
                    .z
                    .checked_add(1)
                    .map(|z| LocationCell { x: index.x, z }),
            ]
            .into_iter()
            .flatten()
            {
                let Some(other) = cells.get(&neighbor) else {
                    continue;
                };
                let mut best: Option<(&Location, &Location)> = None;
                for a in sites {
                    for b in other {
                        if best.is_none_or(|(p, q)| {
                            distance(a, b)
                                .total_cmp(&distance(p, q))
                                .then_with(|| pair(a, b).cmp(&pair(p, q)))
                                .is_lt()
                        }) {
                            best = Some((a, b));
                        }
                    }
                }
                if let Some((a, b)) = best {
                    connect(a, b)?;
                }
            }
        }
        Ok(Self {
            connections: edges.into_values().collect(),
        })
    }

    pub fn owned_by(&self, owner: LocationCell) -> impl Iterator<Item = &LocationConnection> {
        self.connections
            .iter()
            .filter(move |edge| edge.owner == owner)
    }
}
fn pair<'a>(a: &'a Location, b: &'a Location) -> (&'a str, &'a str) {
    if a.id < b.id {
        (&a.id, &b.id)
    } else {
        (&b.id, &a.id)
    }
}
fn distance(a: &Location, b: &Location) -> f64 {
    (a.position.x * 0.25 - b.position.x * 0.25).hypot(a.position.z * 0.25 - b.position.z * 0.25)
}
fn cell(point: Point2, size: f64) -> Result<LocationCell, LocationError> {
    let x = (point.x / size).floor();
    let z = (point.z / size).floor();
    if x < i32::MIN as f64 || x > i32::MAX as f64 || z < i32::MIN as f64 || z > i32::MAX as f64 {
        return Err(LocationError::CellOutOfRange);
    }
    Ok(LocationCell {
        x: x as i32,
        z: z as i32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn site(id: &str, x: f64, z: f64) -> Location {
        Location {
            id: id.into(),
            position: Point2 { x, z },
        }
    }
    #[test]
    fn triangle_removes_longest_and_preserves_input() {
        let sites = vec![site("a", 1., 1.), site("b", 4., 1.), site("c", 4., 5.)];
        let before = sites.clone();
        let graph = LocationConnections::build(&sites, 10.).unwrap();
        assert_eq!(
            graph
                .connections
                .iter()
                .map(|e| (e.a.as_str(), e.b.as_str()))
                .collect::<Vec<_>>(),
            vec![("a", "b"), ("b", "c")]
        );
        assert_eq!(sites, before);
        let mut reversed = sites;
        reversed.reverse();
        assert_eq!(graph, LocationConnections::build(&reversed, 10.).unwrap());
    }
    #[test]
    fn cardinal_neighbors_choose_closest_pair_once() {
        let sites = vec![
            site("a", 1., 1.),
            site("b", 9., 1.),
            site("c", 11., 1.),
            site("d", 19., 1.),
            site("diagonal", 21., 11.),
        ];
        let graph = LocationConnections::build(&sites, 10.).unwrap();
        assert_eq!(graph.connections.len(), 3);
        assert!(
            graph
                .connections
                .iter()
                .any(|e| e.a == "b" && e.b == "c" && e.owner == LocationCell { x: 1, z: 0 })
        );
    }
    #[test]
    fn negative_boundary_has_one_midpoint_owner() {
        let graph =
            LocationConnections::build(&[site("a", -1., -1.), site("b", 1., -1.)], 10.).unwrap();
        assert_eq!(graph.connections.len(), 1);
        assert_eq!(graph.owned_by(LocationCell { x: 0, z: -1 }).count(), 1);
        assert_eq!(graph.owned_by(LocationCell { x: -1, z: -1 }).count(), 0);
    }
    #[test]
    fn ties_are_stable_and_empty_and_single_cells_have_no_edges() {
        let sites = vec![site("b", 2., 2.), site("a", 2., 2.), site("c", 12., 2.)];
        let graph = LocationConnections::build(&sites, 10.).unwrap();
        assert!(graph.connections.iter().any(|e| e.a == "a" && e.b == "c"));
        assert_eq!(graph.connections.len(), 2);
        assert!(
            LocationConnections::build(&[], 10.)
                .unwrap()
                .connections
                .is_empty()
        );
        assert!(
            LocationConnections::build(&sites[..1], 10.)
                .unwrap()
                .connections
                .is_empty()
        );
    }
    #[test]
    fn invalid_input_is_rejected() {
        assert_eq!(
            LocationConnections::build(&[], 0.),
            Err(LocationError::InvalidCellSize)
        );
        assert_eq!(
            LocationConnections::build(&[site("a", 0., 0.), site("a", 1., 1.)], 10.),
            Err(LocationError::DuplicateId)
        );
        assert_eq!(
            LocationConnections::build(&[site("a", f64::NAN, 0.)], 10.),
            Err(LocationError::InvalidLocation)
        );
        assert_eq!(
            LocationConnections::build(&[site("a", 1e100, 0.)], 10.),
            Err(LocationError::CellOutOfRange)
        );
    }
}
