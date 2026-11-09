// Copyright (c) Rune Skovbo Johansen
// SPDX-License-Identifier: MPL-2.0
//! Global-lattice composition adapted from GeoGridLayer and PathsBurstMethod.
use crate::cultivation::{CultivationLayer, CultivationPatch, PatchBounds, Road};
use crate::path::{PathPoint, PathWidth, Point2, TerrainSample};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::{error::Error, fmt};

const MAX_COORDINATE: f64 = 1e12;
const MAX_ITEMS: usize = 4096;
const MAX_SEGMENTS: usize = 262_144;
const MAX_VERTICES: usize = 1_048_576;
const MAX_GRID_WORK: usize = 20_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridPoint {
    pub x: i64,
    pub z: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridRequest {
    pub min: GridPoint,
    pub cells_x: u32,
    pub cells_z: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SurfacePad {
    pub id: String,
    pub center: Point2,
    pub height: f64,
    pub width: PathWidth,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SurfaceProtection {
    BuriedVolume {
        id: String,
        footprint: PatchBounds,
        floor: f64,
        roof: f64,
        cover: f64,
    },
    Reserved {
        id: String,
        footprint: PatchBounds,
    },
    Opening {
        id: String,
        footprint: PatchBounds,
    },
}
impl SurfaceProtection {
    fn id(&self) -> &str {
        match self {
            Self::BuriedVolume { id, .. }
            | Self::Reserved { id, .. }
            | Self::Opening { id, .. } => id,
        }
    }
    fn footprint(&self) -> PatchBounds {
        match *self {
            Self::BuriedVolume { footprint, .. }
            | Self::Reserved { footprint, .. }
            | Self::Opening { footprint, .. } => footprint,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfacePresence {
    Solid,
    Reserved,
    Opening,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocationDistance {
    pub toward_center: Point2,
    pub edge_distance: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeoSample {
    pub height: f64,
    pub requested_height: f64,
    pub minimum_cover_height: Option<f64>,
    pub location_coverage: f64,
    pub road_coverage: f64,
    pub nearest_location: Option<LocationDistance>,
    pub surface: SurfacePresence,
    pub passable: bool,
    pub cost: f64,
}
impl GeoSample {
    pub fn cover_limited(self) -> bool {
        self.height > self.requested_height
    }
}

#[derive(Debug)]
pub enum GeoGridError {
    InvalidInput,
    DuplicateId(String),
    ConflictingRoad {
        a: String,
        b: String,
    },
    MissingTerrain(Point2),
    InvalidTerrain(Point2),
    InsufficientCover {
        id: String,
        point: Point2,
        base: f64,
        minimum: f64,
    },
    RoadCrossesReservation {
        a: String,
        b: String,
        protection: String,
    },
    Budget,
    Cultivation(crate::cultivation::CultivationError),
}
impl fmt::Display for GeoGridError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput => f.write_str("invalid GeoGrid input"),
            Self::DuplicateId(id) => write!(f, "duplicate GeoGrid ID {id}"),
            Self::ConflictingRoad { a, b } => write!(f, "conflicting road geometry for {a} to {b}"),
            Self::MissingTerrain(p) => write!(f, "missing base terrain at {}, {}", p.x, p.z),
            Self::InvalidTerrain(p) => write!(f, "invalid base terrain at {}, {}", p.x, p.z),
            Self::InsufficientCover {
                id, base, minimum, ..
            } => write!(
                f,
                "base terrain {base} violates {id} minimum cover height {minimum}"
            ),
            Self::RoadCrossesReservation { a, b, protection } => write!(
                f,
                "road {a} to {b} crosses surface reservation {protection}"
            ),
            Self::Budget => f.write_str("GeoGrid work or allocation budget exceeded"),
            Self::Cultivation(e) => write!(f, "{e}"),
        }
    }
}
impl Error for GeoGridError {}

#[derive(Clone, Debug, PartialEq)]
struct Effect {
    points: Vec<PathPoint>,
    width: PathWidth,
    bounds: PatchBounds,
}

/// Complete location/protection snapshot in one chart and one terrain revision.
/// The base callback must return the same sample for the same point throughout
/// this layer's lifetime. Underground rooms are protections, never surface pads.
pub struct GeoGridLayer<S> {
    spacing: f64,
    base: S,
    pads: Vec<SurfacePad>,
    locations: Vec<Effect>,
    protections: Vec<SurfaceProtection>,
}

/// A final composition snapshot. Road payloads are immutable and independent of
/// later cultivation unloading. Locations remain the only pre-route effects.
pub struct GeoGridComposition<'a, S> {
    layer: &'a GeoGridLayer<S>,
    roads: Vec<Effect>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeoGrid {
    request: GridRequest,
    pub spacing: f64,
    pub vertices: Vec<GeoSample>,
    pub cells: Vec<SurfacePresence>,
}
impl GeoGrid {
    pub fn vertex(&self, point: GridPoint) -> Option<&GeoSample> {
        let x = point.x.checked_sub(self.request.min.x)?;
        let z = point.z.checked_sub(self.request.min.z)?;
        if x < 0
            || z < 0
            || x > i64::from(self.request.cells_x)
            || z > i64::from(self.request.cells_z)
        {
            return None;
        }
        self.vertices
            .get(z as usize * (self.request.cells_x as usize + 1) + x as usize)
    }
    pub fn cell(&self, point: GridPoint) -> Option<SurfacePresence> {
        let x = point.x.checked_sub(self.request.min.x)?;
        let z = point.z.checked_sub(self.request.min.z)?;
        if x < 0
            || z < 0
            || x >= i64::from(self.request.cells_x)
            || z >= i64::from(self.request.cells_z)
        {
            return None;
        }
        self.cells
            .get(z as usize * self.request.cells_x as usize + x as usize)
            .copied()
    }
}

/// Owns road demand until the terrain patch unloads.
pub struct GeoGridPatch {
    pub grid: GeoGrid,
    _demand: CultivationPatch,
}

impl<S: Fn(Point2) -> Option<TerrainSample>> GeoGridLayer<S> {
    pub fn new(
        spacing: f64,
        mut pads: Vec<SurfacePad>,
        mut protections: Vec<SurfaceProtection>,
        base: S,
    ) -> Result<Self, GeoGridError> {
        if !spacing.is_finite()
            || spacing <= 0.
            || spacing > MAX_COORDINATE
            || pads.len() > MAX_ITEMS
            || protections.len() > MAX_ITEMS
        {
            return Err(GeoGridError::InvalidInput);
        }
        pads.sort_by(|a, b| a.id.cmp(&b.id));
        protections.sort_by(|a, b| a.id().cmp(b.id()));
        for pair in pads.windows(2) {
            if pair[0].id == pair[1].id {
                return Err(GeoGridError::DuplicateId(pair[0].id.clone()));
            }
        }
        for pair in protections.windows(2) {
            if pair[0].id() == pair[1].id() {
                return Err(GeoGridError::DuplicateId(pair[0].id().into()));
            }
        }
        let mut locations = Vec::with_capacity(pads.len());
        for pad in &pads {
            if pad.id.is_empty() {
                return Err(GeoGridError::InvalidInput);
            }
            locations.push(effect(
                vec![PathPoint {
                    position: pad.center,
                    height: pad.height,
                }],
                pad.width,
            )?);
        }
        for protection in &protections {
            if protection.id().is_empty() || !valid_bounds(protection.footprint()) {
                return Err(GeoGridError::InvalidInput);
            }
            if let SurfaceProtection::BuriedVolume {
                floor, roof, cover, ..
            } = *protection
                && (!coordinate(floor)
                    || !coordinate(roof)
                    || floor >= roof
                    || !cover.is_finite()
                    || cover < 0.
                    || !coordinate(roof + cover))
            {
                return Err(GeoGridError::InvalidInput);
            }
        }
        Ok(Self {
            spacing,
            base,
            pads,
            locations,
            protections,
        })
    }

    pub fn sample_vertex(&self, point: GridPoint) -> Result<GeoSample, GeoGridError> {
        self.sample(point, &[])
    }

    /// Samples the same global triangle grid as terrain output. Openings and
    /// reservations are non-walkable even when they fall between grid vertices.
    pub fn terrain_sample(&self, p: Point2) -> Result<TerrainSample, GeoGridError> {
        if !valid_point(p) {
            return Err(GeoGridError::InvalidInput);
        }
        let x = (p.x / self.spacing).floor();
        let z = (p.z / self.spacing).floor();
        if x.abs() > ((1u64 << 50) as f64) || z.abs() > ((1u64 << 50) as f64) {
            return Err(GeoGridError::InvalidInput);
        }
        let origin = GridPoint {
            x: x as i64,
            z: z as i64,
        };
        let a = self.sample_vertex(origin)?;
        let b = self.sample_vertex(GridPoint {
            x: origin.x + 1,
            z: origin.z,
        })?;
        let c = self.sample_vertex(GridPoint {
            x: origin.x,
            z: origin.z + 1,
        })?;
        let d = self.sample_vertex(GridPoint {
            x: origin.x + 1,
            z: origin.z + 1,
        })?;
        let q = self.position(origin)?;
        let u = ((p.x - q.x) / self.spacing).clamp(0., 1.);
        let v = ((p.z - q.z) / self.spacing).clamp(0., 1.);
        let height = if u + v <= 1. {
            a.height + (b.height - a.height) * u + (c.height - a.height) * v
        } else {
            d.height + (c.height - d.height) * (1. - u) + (b.height - d.height) * (1. - v)
        };
        let cost = if u + v <= 1. {
            a.cost + (b.cost - a.cost) * u + (c.cost - a.cost) * v
        } else {
            d.cost + (c.cost - d.cost) * (1. - u) + (b.cost - d.cost) * (1. - v)
        };
        Ok(TerrainSample {
            height,
            cost,
            passable: a.passable
                && b.passable
                && c.passable
                && d.passable
                && self.cell_presence(origin)? == SurfacePresence::Solid,
        })
    }

    pub fn with_roads(
        &self,
        roads: impl IntoIterator<Item = Rc<Road>>,
    ) -> Result<GeoGridComposition<'_, S>, GeoGridError> {
        let mut canonical = BTreeMap::new();
        let mut count = 0usize;
        let mut segments = 0usize;
        for road in roads {
            count += 1;
            if count > MAX_ITEMS {
                return Err(GeoGridError::Budget);
            }
            let a = &road.connection.a;
            let b = &road.connection.b;
            if a.is_empty() || a >= b {
                return Err(GeoGridError::InvalidInput);
            }
            if road.path.points.len() > MAX_SEGMENTS {
                return Err(GeoGridError::Budget);
            }
            let prepared = effect(road.path.points.clone(), road.path.width)?;
            let key = (a.clone(), b.clone());
            if let Some(previous) = canonical.get(&key) {
                if previous != &prepared {
                    return Err(GeoGridError::ConflictingRoad {
                        a: a.clone(),
                        b: b.clone(),
                    });
                }
                continue;
            }
            segments = segments
                .checked_add(prepared.points.len().max(2) - 1)
                .ok_or(GeoGridError::Budget)?;
            let checks = segments
                .checked_mul(self.protections.len())
                .ok_or(GeoGridError::Budget)?;
            if segments > MAX_SEGMENTS || checks > MAX_GRID_WORK {
                return Err(GeoGridError::Budget);
            }
            for protection in &self.protections {
                if matches!(protection, SurfaceProtection::BuriedVolume { .. }) {
                    continue;
                }
                let bounds = expanded(protection.footprint(), effect_radius(prepared.width));
                let crosses = if prepared.points.len() == 1 {
                    contains(bounds, prepared.points[0].position)
                } else {
                    prepared
                        .points
                        .windows(2)
                        .any(|pair| segment_intersects(pair[0].position, pair[1].position, bounds))
                };
                if crosses {
                    return Err(GeoGridError::RoadCrossesReservation {
                        a: a.clone(),
                        b: b.clone(),
                        protection: protection.id().into(),
                    });
                }
            }
            canonical.insert(key, prepared);
        }
        Ok(GeoGridComposition {
            layer: self,
            roads: canonical.into_values().collect(),
        })
    }

    pub fn grid(&self, request: GridRequest) -> Result<GeoGrid, GeoGridError> {
        self.build_grid(request, &[])
    }

    pub fn compose_patch<T>(
        &self,
        cultivation: &mut CultivationLayer<T>,
        request: GridRequest,
    ) -> Result<GeoGridPatch, GeoGridError>
    where
        T: FnMut(Point2) -> Option<TerrainSample>,
    {
        let (max, _) = self.validate_request(request, 0)?;
        let bounds = PatchBounds {
            min: self.position(request.min)?,
            max: self.position(max)?,
        };
        let demand = cultivation
            .retain_patch(bounds)
            .map_err(GeoGridError::Cultivation)?;
        let composition = self.with_roads(cultivation.retained_roads())?;
        let grid = composition.grid(request)?;
        Ok(GeoGridPatch {
            grid,
            _demand: demand,
        })
    }

    fn position(&self, p: GridPoint) -> Result<Point2, GeoGridError> {
        if p.x.unsigned_abs() > (1u64 << 50) || p.z.unsigned_abs() > (1u64 << 50) {
            return Err(GeoGridError::InvalidInput);
        }
        let result = Point2 {
            x: p.x as f64 * self.spacing,
            z: p.z as f64 * self.spacing,
        };
        if !valid_point(result)
            || result.x + self.spacing == result.x
            || result.z + self.spacing == result.z
        {
            return Err(GeoGridError::InvalidInput);
        }
        Ok(result)
    }
    fn cell_presence(&self, point: GridPoint) -> Result<SurfacePresence, GeoGridError> {
        let min = self.position(point)?;
        let max = self.position(GridPoint {
            x: point.x.checked_add(1).ok_or(GeoGridError::InvalidInput)?,
            z: point.z.checked_add(1).ok_or(GeoGridError::InvalidInput)?,
        })?;
        Ok(self.presence(PatchBounds { min, max }))
    }
    fn presence(&self, support: PatchBounds) -> SurfacePresence {
        let mut presence = SurfacePresence::Solid;
        for protection in &self.protections {
            if !overlap(support, protection.footprint()) {
                continue;
            }
            match protection {
                SurfaceProtection::Opening { .. } => return SurfacePresence::Opening,
                SurfaceProtection::Reserved { .. } => presence = SurfacePresence::Reserved,
                SurfaceProtection::BuriedVolume { .. } => {}
            }
        }
        presence
    }
    fn sample(&self, point: GridPoint, roads: &[Effect]) -> Result<GeoSample, GeoGridError> {
        let p = self.position(point)?;
        let base = (self.base)(p).ok_or(GeoGridError::MissingTerrain(p))?;
        if !coordinate(base.height) || !base.cost.is_finite() || base.cost < 0. {
            return Err(GeoGridError::InvalidTerrain(p));
        }
        let (location_height, mut location_coverage) = blend(p, base.height, &self.locations);
        let (mut requested_height, mut road_coverage) = blend(p, location_height, roads);
        let support = PatchBounds {
            min: self.position(GridPoint {
                x: point.x - 1,
                z: point.z - 1,
            })?,
            max: self.position(GridPoint {
                x: point.x + 1,
                z: point.z + 1,
            })?,
        };
        let surface = self.presence(support);
        if surface != SurfacePresence::Solid {
            requested_height = base.height;
            location_coverage = 0.;
            road_coverage = 0.;
        }
        let mut minimum: Option<f64> = None;
        for protection in &self.protections {
            if let SurfaceProtection::BuriedVolume {
                id,
                footprint,
                roof,
                cover,
                ..
            } = protection
            {
                if !overlap(support, *footprint) {
                    continue;
                }
                let required = roof + cover;
                if base.height < required {
                    return Err(GeoGridError::InsufficientCover {
                        id: id.clone(),
                        point: p,
                        base: base.height,
                        minimum: required,
                    });
                }
                minimum = Some(minimum.map_or(required, |v| v.max(required)));
            }
        }
        let nearest_location = self
            .pads
            .iter()
            .map(|pad| {
                let toward_center = Point2 {
                    x: pad.center.x - p.x,
                    z: pad.center.z - p.z,
                };
                LocationDistance {
                    toward_center,
                    edge_distance: toward_center.x.hypot(toward_center.z) - pad.width.surface * 0.5,
                }
            })
            .min_by(|a, b| a.edge_distance.total_cmp(&b.edge_distance));
        Ok(GeoSample {
            height: minimum.map_or(requested_height, |v| v.max(requested_height)),
            requested_height,
            minimum_cover_height: minimum,
            location_coverage: location_coverage * (1. - road_coverage),
            road_coverage,
            nearest_location,
            surface,
            passable: base.passable && surface == SurfacePresence::Solid,
            cost: base.cost
                + nearest_location
                    .map_or(0., |d| ((6. - d.edge_distance) / 12.).clamp(0., 1.) * 20.),
        })
    }
    fn validate_request(
        &self,
        r: GridRequest,
        road_segments: usize,
    ) -> Result<(GridPoint, usize), GeoGridError> {
        if r.cells_x as u64 >= MAX_VERTICES as u64 || r.cells_z as u64 >= MAX_VERTICES as u64 {
            return Err(GeoGridError::Budget);
        }
        if r.cells_x == 0 || r.cells_z == 0 {
            return Err(GeoGridError::InvalidInput);
        }
        let max = GridPoint {
            x: r.min
                .x
                .checked_add(i64::from(r.cells_x))
                .ok_or(GeoGridError::InvalidInput)?,
            z: r.min
                .z
                .checked_add(i64::from(r.cells_z))
                .ok_or(GeoGridError::InvalidInput)?,
        };
        self.position(r.min)?;
        self.position(max)?;
        let vertices = (r.cells_x as usize + 1)
            .checked_mul(r.cells_z as usize + 1)
            .ok_or(GeoGridError::Budget)?;
        let work = vertices
            .checked_mul(self.pads.len() * 2 + self.protections.len() * 2 + road_segments + 1)
            .ok_or(GeoGridError::Budget)?;
        if vertices > MAX_VERTICES || work > MAX_GRID_WORK {
            return Err(GeoGridError::Budget);
        }
        Ok((max, vertices))
    }
    fn build_grid(&self, r: GridRequest, roads: &[Effect]) -> Result<GeoGrid, GeoGridError> {
        let segments = roads.iter().map(|e| e.points.len().max(2) - 1).sum();
        let (_, count) = self.validate_request(r, segments)?;
        let mut vertices = Vec::with_capacity(count);
        for z in 0..=r.cells_z {
            for x in 0..=r.cells_x {
                vertices.push(self.sample(
                    GridPoint {
                        x: r.min.x + i64::from(x),
                        z: r.min.z + i64::from(z),
                    },
                    roads,
                )?);
            }
        }
        let mut cells = Vec::with_capacity(r.cells_x as usize * r.cells_z as usize);
        for z in 0..r.cells_z {
            for x in 0..r.cells_x {
                cells.push(self.cell_presence(GridPoint {
                    x: r.min.x + i64::from(x),
                    z: r.min.z + i64::from(z),
                })?);
            }
        }
        Ok(GeoGrid {
            request: r,
            spacing: self.spacing,
            vertices,
            cells,
        })
    }
}
impl<S: Fn(Point2) -> Option<TerrainSample>> GeoGridComposition<'_, S> {
    pub fn sample_vertex(&self, point: GridPoint) -> Result<GeoSample, GeoGridError> {
        self.layer.sample(point, &self.roads)
    }
    pub fn grid(&self, request: GridRequest) -> Result<GeoGrid, GeoGridError> {
        self.layer.build_grid(request, &self.roads)
    }
}
fn coordinate(value: f64) -> bool {
    value.is_finite() && value.abs() <= MAX_COORDINATE
}
fn valid_point(p: Point2) -> bool {
    coordinate(p.x) && coordinate(p.z)
}
fn valid_bounds(b: PatchBounds) -> bool {
    valid_point(b.min) && valid_point(b.max) && b.min.x < b.max.x && b.min.z < b.max.z
}
fn contains(b: PatchBounds, p: Point2) -> bool {
    p.x >= b.min.x && p.x <= b.max.x && p.z >= b.min.z && p.z <= b.max.z
}
fn overlap(a: PatchBounds, b: PatchBounds) -> bool {
    a.min.x <= b.max.x && a.max.x >= b.min.x && a.min.z <= b.max.z && a.max.z >= b.min.z
}
fn expanded(b: PatchBounds, r: f64) -> PatchBounds {
    PatchBounds {
        min: Point2 {
            x: b.min.x - r,
            z: b.min.z - r,
        },
        max: Point2 {
            x: b.max.x + r,
            z: b.max.z + r,
        },
    }
}
fn effect_radius(w: PathWidth) -> f64 {
    (w.surface * 0.5 + w.shoulder).max(w.material * 0.5)
}
fn effect(points: Vec<PathPoint>, width: PathWidth) -> Result<Effect, GeoGridError> {
    if points.is_empty()
        || points.len() > MAX_SEGMENTS
        || !coordinate(width.surface)
        || width.surface <= 0.
        || !coordinate(width.shoulder)
        || width.shoulder < 0.
        || !coordinate(width.material)
        || width.material < 0.
        || points
            .iter()
            .any(|p| !valid_point(p.position) || !coordinate(p.height))
    {
        return Err(GeoGridError::InvalidInput);
    }
    let mut bounds = PatchBounds {
        min: points[0].position,
        max: points[0].position,
    };
    for p in &points {
        bounds.min.x = bounds.min.x.min(p.position.x);
        bounds.min.z = bounds.min.z.min(p.position.z);
        bounds.max.x = bounds.max.x.max(p.position.x);
        bounds.max.z = bounds.max.z.max(p.position.z);
    }
    Ok(Effect {
        points,
        width,
        bounds: expanded(bounds, effect_radius(width)),
    })
}
fn smooth(t: f64) -> f64 {
    let t = t.clamp(0., 1.);
    t * t * (3. - 2. * t)
}
fn measure(p: Point2, a: PathPoint, b: PathPoint) -> (f64, f64) {
    let dx = b.position.x - a.position.x;
    let dz = b.position.z - a.position.z;
    let px = p.x - a.position.x;
    let pz = p.z - a.position.z;
    let n = dx.hypot(dz);
    if n == 0. {
        return (px.hypot(pz), a.height);
    }
    let t = ((px * (dx / n) + pz * (dz / n)) / n).clamp(-4., 5.);
    let u = t.clamp(0., 1.);
    (
        (px - dx * u).hypot(pz - dz * u),
        a.height + (b.height - a.height) * t,
    )
}
fn blend(p: Point2, source: f64, effects: &[Effect]) -> (f64, f64) {
    let (mut max_log, mut total, mut height, mut influence, mut coverage) =
        (f64::NEG_INFINITY, 0., 0., 0., 0_f64);
    for effect in effects {
        if !contains(effect.bounds, p) {
            continue;
        }
        let inner = effect.width.surface * 0.5;
        let shoulder = effect.width.shoulder;
        let outer = inner + shoulder;
        let material = effect.width.material * 0.5;
        for i in 0..effect.points.len().max(2) - 1 {
            let a = effect.points[i.min(effect.points.len() - 1)];
            let b = effect.points[(i + 1).min(effect.points.len() - 1)];
            let (distance, target) = measure(p, a, b);
            if material > 0. {
                coverage = coverage.max(((material - distance) / material.min(0.5)).clamp(0., 1.));
            }
            if distance > outer || (shoulder > 0. && distance >= outer) {
                continue;
            }
            let (log_weight, amount) = if shoulder == 0. {
                (0., 1.)
            } else {
                (
                    8. * ((outer - distance).ln() - shoulder.ln()),
                    smooth((outer - distance) / shoulder),
                )
            };
            if log_weight > max_log {
                let scale = (max_log - log_weight).exp();
                total *= scale;
                height *= scale;
                influence *= scale;
                max_log = log_weight;
            }
            let weight = (log_weight - max_log).exp();
            total += weight;
            height += target * weight;
            influence += amount * weight;
        }
    }
    (
        if total == 0. {
            source
        } else {
            source + (height / total - source) * (influence / total).clamp(0., 1.)
        },
        coverage,
    )
}
fn segment_intersects(a: Point2, b: Point2, bounds: PatchBounds) -> bool {
    let mut lo = 0_f64;
    let mut hi = 1_f64;
    for (start, delta, min, max) in [
        (a.x, b.x - a.x, bounds.min.x, bounds.max.x),
        (a.z, b.z - a.z, bounds.min.z, bounds.max.z),
    ] {
        if delta == 0. {
            if start < min || start > max {
                return false;
            }
            continue;
        }
        let p = (min - start) / delta;
        let q = (max - start) / delta;
        lo = lo.max(p.min(q));
        hi = hi.min(p.max(q));
        if lo > hi {
            return false;
        }
    }
    true
}
