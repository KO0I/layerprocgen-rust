// Copyright (c) Rune Skovbo Johansen
// SPDX-License-Identifier: MPL-2.0
//! Owned analytic effects for consumers with an existing scalar base sampler.
use super::{
    Effect, GeoGridError, PatchBounds, PathPoint, PathWidth, Point2, blend_segments, contains,
    segment_bounds, valid_point,
};
use std::collections::HashMap;
const MAX_REFERENCES: usize = 1_048_576;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceEffectSample {
    pub height: f64,
    pub location_coverage: f64,
    pub road_coverage: f64,
}
#[derive(Clone, Debug)]
pub struct SurfaceEffects {
    locations: Stage,
    roads: Stage,
}
#[derive(Clone, Copy, Debug)]
struct Segment {
    a: PathPoint,
    b: PathPoint,
    width: PathWidth,
    bounds: PatchBounds,
}
#[derive(Clone, Debug)]
struct Stage {
    segments: Vec<Segment>,
    buckets: HashMap<(i32, i32), Vec<usize>>,
    bounds: Option<PatchBounds>,
    cell: f64,
    elevation_bound: f64,
}
impl SurfaceEffects {
    pub(super) fn new(locations: &[Effect], roads: &[Effect]) -> Result<Self, GeoGridError> {
        Ok(Self {
            locations: Stage::new(locations)?,
            roads: Stage::new(roads)?,
        })
    }
    pub fn sample(
        &self,
        point: Point2,
        source_height: f64,
    ) -> Result<SurfaceEffectSample, GeoGridError> {
        if !valid_point(point) || !source_height.is_finite() {
            return Err(GeoGridError::InvalidInput);
        }
        let (height, location) = self.locations.sample(point, source_height);
        let (height, road) = self.roads.sample(point, height);
        Ok(SurfaceEffectSample {
            height,
            location_coverage: location * (1. - road),
            road_coverage: road,
        })
    }
    pub fn bounds(&self) -> Option<PatchBounds> {
        merge(self.locations.bounds, self.roads.bounds)
    }
    pub fn elevation_bound(&self) -> f64 {
        self.locations
            .elevation_bound
            .max(self.roads.elevation_bound)
    }
}
impl Stage {
    fn new(effects: &[Effect]) -> Result<Self, GeoGridError> {
        let mut segments = Vec::new();
        let mut bounds = None;
        let mut elevation_bound = 0_f64;
        for effect in effects {
            bounds = merge(bounds, Some(effect.bounds));
            for i in 0..effect.points.len().max(2) - 1 {
                let a = effect.points[i.min(effect.points.len() - 1)];
                let b = effect.points[(i + 1).min(effect.points.len() - 1)];
                segments.push(Segment {
                    a,
                    b,
                    width: effect.width,
                    bounds: segment_bounds(a, b, effect.width),
                });
                elevation_bound = elevation_bound
                    .max(a.height.abs().max(b.height.abs()) + 4. * (b.height - a.height).abs());
            }
        }
        let mut stage = Self {
            segments,
            buckets: HashMap::new(),
            bounds,
            cell: 1.,
            elevation_bound,
        };
        let Some(bounds) = bounds else {
            return Ok(stage);
        };
        stage.cell = ((bounds.max.x - bounds.min.x).max(bounds.max.z - bounds.min.z) / 128.)
            .max(f64::MIN_POSITIVE);
        let mut references = 0usize;
        for (id, segment) in stage.segments.iter().enumerate() {
            let (x0, z0) = stage.key(segment.bounds.min);
            let (x1, z1) = stage.key(segment.bounds.max);
            let width = i64::from(x1) - i64::from(x0) + 1;
            let height = i64::from(z1) - i64::from(z0) + 1;
            if width <= 0 || height <= 0 || width > 256 || height > 256 {
                return Err(GeoGridError::Budget);
            }
            references = references
                .checked_add(width as usize * height as usize)
                .ok_or(GeoGridError::Budget)?;
            if references > MAX_REFERENCES {
                return Err(GeoGridError::Budget);
            }
            for z in z0..=z1 {
                for x in x0..=x1 {
                    stage.buckets.entry((x, z)).or_default().push(id);
                }
            }
        }
        Ok(stage)
    }
    fn key(&self, p: Point2) -> (i32, i32) {
        let min = self.bounds.expect("indexed stage").min;
        (
            ((p.x - min.x) / self.cell).floor() as i32,
            ((p.z - min.z) / self.cell).floor() as i32,
        )
    }
    fn sample(&self, p: Point2, source: f64) -> (f64, f64) {
        if self.bounds.is_none_or(|bounds| !contains(bounds, p)) {
            return (source, 0.);
        }
        let Some(ids) = self.buckets.get(&self.key(p)) else {
            return (source, 0.);
        };
        blend_segments(
            p,
            source,
            ids.iter()
                .map(|&id| &self.segments[id])
                .filter(|segment| contains(segment.bounds, p))
                .map(|s| (s.a, s.b, s.width)),
        )
    }
}
fn merge(a: Option<PatchBounds>, b: Option<PatchBounds>) -> Option<PatchBounds> {
    match (a, b) {
        (None, b) => b,
        (a, None) => a,
        (Some(a), Some(b)) => Some(PatchBounds {
            min: Point2 {
                x: a.min.x.min(b.min.x),
                z: a.min.z.min(b.min.z),
            },
            max: Point2 {
                x: a.max.x.max(b.max.x),
                z: a.max.z.max(b.max.z),
            },
        }),
    }
}
