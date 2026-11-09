// Copyright (c) Rune Skovbo Johansen
// SPDX-License-Identifier: MPL-2.0
//! Slope materials and detail density adapted from the C# LandscapeLayer sample.
use crate::common::RandomHash;
use crate::geogrid::{GeoGrid, GridPoint, GridRequest, MAX_VERTICES, SurfacePresence};
use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LandscapeRequest {
    pub region: GridRequest,
    pub seed: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaterialWeights {
    climate: f64,
    cliff: f64,
    path: f64,
}
impl MaterialWeights {
    pub fn climate(self) -> f64 {
        self.climate
    }
    pub fn cliff(self) -> f64 {
        self.cliff
    }
    pub fn path(self) -> f64 {
        self.path
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LandscapeVertex {
    pub height: f64,
    pub normal: [f64; 3],
    pub material: MaterialWeights,
    pub vegetation_suitability: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LandscapeCell {
    pub surface: SurfacePresence,
    pub road_excluded: bool,
    /// Grass/detail instances per grid cell, not a species or mesh selection.
    pub vegetation_count: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LandscapeGrid {
    region: GridRequest,
    spacing: f64,
    vertices: Vec<LandscapeVertex>,
    cells: Vec<LandscapeCell>,
}
impl LandscapeGrid {
    pub fn request(&self) -> GridRequest {
        self.region
    }
    pub fn spacing(&self) -> f64 {
        self.spacing
    }
    pub fn vertices(&self) -> &[LandscapeVertex] {
        &self.vertices
    }
    pub fn cells(&self) -> &[LandscapeCell] {
        &self.cells
    }
    pub fn vertex(&self, point: GridPoint) -> Option<&LandscapeVertex> {
        let x = point.x.checked_sub(self.region.min.x)?;
        let z = point.z.checked_sub(self.region.min.z)?;
        if x < 0
            || z < 0
            || x > i64::from(self.region.cells_x)
            || z > i64::from(self.region.cells_z)
        {
            return None;
        }
        self.vertices
            .get(z as usize * (self.region.cells_x as usize + 1) + x as usize)
    }
    pub fn cell(&self, point: GridPoint) -> Option<&LandscapeCell> {
        let x = point.x.checked_sub(self.region.min.x)?;
        let z = point.z.checked_sub(self.region.min.z)?;
        if x < 0
            || z < 0
            || x >= i64::from(self.region.cells_x)
            || z >= i64::from(self.region.cells_z)
        {
            return None;
        }
        self.cells
            .get(z as usize * self.region.cells_x as usize + x as usize)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LandscapeError {
    InvalidRequest,
    InvalidHalo,
    Budget,
    InvalidPlantability { point: GridPoint, value: f64 },
}
impl fmt::Display for LandscapeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest => f.write_str("invalid Landscape grid request"),
            Self::InvalidHalo => f.write_str("Landscape requires its exact finite GeoGrid halo"),
            Self::Budget => f.write_str("Landscape vertex budget exceeded"),
            Self::InvalidPlantability { point, value } => write!(
                f,
                "invalid climate plantability {value} at {}, {}",
                point.x, point.z
            ),
        }
    }
}
impl Error for LandscapeError {}

impl LandscapeRequest {
    /// Fetch this region from the same complete GeoGrid composition snapshot.
    /// Streaming demand must cover the halo until its terrain consumer unloads.
    pub fn source_request(self) -> Result<GridRequest, LandscapeError> {
        if self.region.cells_x == 0 || self.region.cells_z == 0 {
            return Err(LandscapeError::InvalidRequest);
        }
        let min = GridPoint {
            x: self
                .region
                .min
                .x
                .checked_sub(1)
                .ok_or(LandscapeError::InvalidRequest)?,
            z: self
                .region
                .min
                .z
                .checked_sub(1)
                .ok_or(LandscapeError::InvalidRequest)?,
        };
        let cells_x = self
            .region
            .cells_x
            .checked_add(2)
            .ok_or(LandscapeError::Budget)?;
        let cells_z = self
            .region
            .cells_z
            .checked_add(2)
            .ok_or(LandscapeError::Budget)?;
        if cells_x as u64 >= MAX_VERTICES as u64 || cells_z as u64 >= MAX_VERTICES as u64 {
            return Err(LandscapeError::Budget);
        }
        let count = (cells_x as usize + 1)
            .checked_mul(cells_z as usize + 1)
            .ok_or(LandscapeError::Budget)?;
        if count > MAX_VERTICES {
            return Err(LandscapeError::Budget);
        }
        min.x
            .checked_add(i64::from(cells_x))
            .ok_or(LandscapeError::InvalidRequest)?;
        min.z
            .checked_add(i64::from(cells_z))
            .ok_or(LandscapeError::InvalidRequest)?;
        Ok(GridRequest {
            min,
            cells_x,
            cells_z,
        })
    }

    /// Climate suitability is independent of color and stable for each global
    /// vertex within a terrain revision. The result leaves climate colors alone.
    pub fn build(
        self,
        halo: GeoGrid,
        plantability: impl Fn(GridPoint) -> f64,
    ) -> Result<LandscapeGrid, LandscapeError> {
        let source = self.source_request()?;
        if halo.request() != source {
            return Err(LandscapeError::InvalidHalo);
        }
        let width = self.region.cells_x as usize + 1;
        let height = self.region.cells_z as usize + 1;
        let mut vertices = Vec::with_capacity(width * height);
        let mut eligible_vertices = Vec::with_capacity(width * height);
        for z in 0..=self.region.cells_z {
            for x in 0..=self.region.cells_x {
                let point = GridPoint {
                    x: self.region.min.x + i64::from(x),
                    z: self.region.min.z + i64::from(z),
                };
                let sample = halo.vertex(point).ok_or(LandscapeError::InvalidHalo)?;
                let left = halo
                    .vertex(GridPoint {
                        x: point.x - 1,
                        z: point.z,
                    })
                    .ok_or(LandscapeError::InvalidHalo)?
                    .height;
                let right = halo
                    .vertex(GridPoint {
                        x: point.x + 1,
                        z: point.z,
                    })
                    .ok_or(LandscapeError::InvalidHalo)?
                    .height;
                let down = halo
                    .vertex(GridPoint {
                        x: point.x,
                        z: point.z - 1,
                    })
                    .ok_or(LandscapeError::InvalidHalo)?
                    .height;
                let up = halo
                    .vertex(GridPoint {
                        x: point.x,
                        z: point.z + 1,
                    })
                    .ok_or(LandscapeError::InvalidHalo)?
                    .height;
                let n = [left - right, 2. * halo.spacing(), down - up];
                let length = n[0].hypot(n[1]).hypot(n[2]);
                let normal = n.map(|value| value / length);
                let cliff = if normal[1] < 0.65 { 1. } else { 0. };
                let path = (sample.location_coverage + sample.road_coverage).min(1.) * (1. - cliff);
                let material = MaterialWeights {
                    climate: 1. - cliff - path,
                    cliff,
                    path,
                };
                let suitability = plantability(point);
                if !unit(suitability) {
                    return Err(LandscapeError::InvalidPlantability {
                        point,
                        value: suitability,
                    });
                }
                let eligible = sample.passable && sample.surface == SurfacePresence::Solid;
                eligible_vertices.push(eligible);
                vertices.push(LandscapeVertex {
                    height: sample.height,
                    normal,
                    material,
                    vegetation_suitability: if eligible {
                        material.climate * suitability
                    } else {
                        0.
                    },
                });
            }
        }
        let mut cells =
            Vec::with_capacity(self.region.cells_x as usize * self.region.cells_z as usize);
        for z in 0..self.region.cells_z {
            for x in 0..self.region.cells_x {
                let point = GridPoint {
                    x: self.region.min.x + i64::from(x),
                    z: self.region.min.z + i64::from(z),
                };
                let surface = halo.cell(point).ok_or(LandscapeError::InvalidHalo)?;
                let road_excluded = halo
                    .cell_has_road(point)
                    .ok_or(LandscapeError::InvalidHalo)?;
                let i = z as usize * width + x as usize;
                let corners = [
                    vertices[i],
                    vertices[i + 1],
                    vertices[i + width],
                    vertices[i + width + 1],
                ];
                let mean = corners
                    .iter()
                    .map(|v| v.vegetation_suitability)
                    .sum::<f64>()
                    * 0.25;
                let maximum = corners
                    .iter()
                    .map(|v| v.vegetation_suitability)
                    .fold(0., f64::max);
                let blocked = surface != SurfacePresence::Solid
                    || road_excluded
                    || [
                        eligible_vertices[i],
                        eligible_vertices[i + 1],
                        eligible_vertices[i + width],
                        eligible_vertices[i + width + 1],
                    ]
                    .contains(&false);
                let vegetation_count = if !blocked && mean > 0.4 {
                    (maximum * 10. + jitter(self.seed, point))
                        .round_ties_even()
                        .clamp(0., 10.) as u8
                } else {
                    0
                };
                cells.push(LandscapeCell {
                    surface,
                    road_excluded,
                    vegetation_count,
                });
            }
        }
        Ok(LandscapeGrid {
            region: self.region,
            spacing: halo.spacing(),
            vertices,
            cells,
        })
    }
}
fn unit(value: f64) -> bool {
    value.is_finite() && (0. ..=1.).contains(&value)
}
fn jitter(seed: u64, point: GridPoint) -> f64 {
    let x = point.x as u64;
    let z = point.z as u64;
    let value = RandomHash::new(seed as u32).hash_values(&[
        (seed >> 32) as u32,
        x as u32,
        (x >> 32) as u32,
        z as u32,
        (z >> 32) as u32,
        9,
    ]);
    f64::from(value) / 4294967296. - 0.5
}
