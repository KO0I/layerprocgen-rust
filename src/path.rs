// Copyright (c) Rune Skovbo Johansen
// SPDX-License-Identifier: MPL-2.0
//! Callback-driven route search and spline tangents adapted from the C# terrain sample.
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::{error::Error, fmt};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point2 {
    pub x: f64,
    pub z: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainSample {
    pub height: f64,
    pub cost: f64,
    pub passable: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathWidth {
    /// Full width of the walkable surface, in the sampler's coordinate units.
    pub surface: f64,
    /// Additional width on each side of the surface.
    pub shoulder: f64,
    /// Full width of the material footprint.
    pub material: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathOptions {
    pub grid_spacing: f64,
    pub corridor: f64,
    pub sample_spacing: f64,
    pub max_grade: f64,
    pub max_expansions: usize,
    /// Maximum sample lookups, including cache hits, across search and smoothing.
    pub max_samples: usize,
    pub width: PathWidth,
}
impl Default for PathOptions {
    fn default() -> Self {
        Self {
            grid_spacing: 2.,
            corridor: 60.,
            sample_spacing: 0.5,
            max_grade: 0.6,
            max_expansions: 10_000,
            max_samples: 400_000,
            width: PathWidth {
                surface: 2.,
                shoulder: 0.5,
                material: 2.5,
            },
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathPoint {
    pub position: Point2,
    pub height: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    pub raw: Vec<PathPoint>,
    pub points: Vec<PathPoint>,
    pub width: PathWidth,
    pub expanded: usize,
    /// Number of unique terrain callback invocations.
    pub sampled: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathError {
    InvalidInput,
    InvalidTerrain,
    NoPath,
    ExpansionBudget,
    SampleBudget,
}
impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "invalid path input",
            Self::InvalidTerrain => "terrain returned an invalid sample",
            Self::NoPath => "no path found",
            Self::ExpansionBudget => "path expansion budget exceeded",
            Self::SampleBudget => "terrain sample budget exceeded",
        })
    }
}
impl Error for PathError {}
impl Point2 {
    fn add(self, b: Self) -> Self {
        Self {
            x: self.x + b.x,
            z: self.z + b.z,
        }
    }
    fn sub(self, b: Self) -> Self {
        Self {
            x: self.x - b.x,
            z: self.z - b.z,
        }
    }
    fn scale(self, t: f64) -> Self {
        Self {
            x: self.x * t,
            z: self.z * t,
        }
    }
    fn length(self) -> f64 {
        self.x.hypot(self.z)
    }
    fn unit(self) -> Self {
        let n = self.length();
        if n == 0. {
            Self { x: 0., z: 0. }
        } else {
            self.scale(1. / n)
        }
    }
    fn finite(self) -> bool {
        self.x.is_finite() && self.z.is_finite()
    }
}
fn distance(a: Point2, b: Point2) -> f64 {
    a.sub(b).length()
}
fn lerp(a: Point2, b: Point2, t: f64) -> Point2 {
    a.add(b.sub(a).scale(t))
}
fn distance_to_segment(p: Point2, a: Point2, b: Point2) -> f64 {
    let ab = b.sub(a);
    let length = ab.length();
    if length == 0. {
        return distance(p, a);
    }
    let u = ab.scale(1. / length);
    let ap = p.sub(a);
    let along = (ap.x * u.x + ap.z * u.z).clamp(0., length);
    distance(p, a.add(u.scale(along)))
}
fn steps(length: f64, spacing: f64, limit: usize) -> Result<usize, PathError> {
    let n = (length / spacing).ceil().max(1.);
    if !n.is_finite() || n > limit as f64 || n >= usize::MAX as f64 {
        return Err(PathError::SampleBudget);
    }
    Ok(n as usize)
}
struct SamplerCache<F> {
    sample: F,
    values: HashMap<(u64, u64), Option<TerrainSample>>,
    max_samples: usize,
    attempts: usize,
}
impl<F: FnMut(Point2) -> Option<TerrainSample>> SamplerCache<F> {
    fn get(&mut self, p: Point2) -> Result<Option<TerrainSample>, PathError> {
        if self.attempts >= self.max_samples {
            return Err(PathError::SampleBudget);
        }
        self.attempts += 1;
        if !p.finite() {
            return Err(PathError::InvalidInput);
        }
        let x = if p.x == 0. { 0. } else { p.x };
        let z = if p.z == 0. { 0. } else { p.z };
        let key = (x.to_bits(), z.to_bits());
        if let Some(v) = self.values.get(&key) {
            return Ok(*v);
        }
        if self.values.len() >= self.max_samples {
            return Err(PathError::SampleBudget);
        }
        let value = (self.sample)(p);
        if let Some(s) = value
            && (!s.height.is_finite() || !s.cost.is_finite() || s.cost < 0.)
        {
            return Err(PathError::InvalidTerrain);
        }
        self.values.insert(key, value);
        Ok(value)
    }
    fn usable(&mut self, p: Point2) -> Result<Option<TerrainSample>, PathError> {
        Ok(self.get(p)?.filter(|s| s.passable))
    }
}
struct Search<F> {
    terrain: SamplerCache<F>,
    start: Point2,
    goal: Point2,
    options: PathOptions,
}
impl<F: FnMut(Point2) -> Option<TerrainSample>> Search<F> {
    fn half_width(&self) -> f64 {
        (self.options.width.surface * 0.5 + self.options.width.shoulder)
            .max(self.options.width.material * 0.5)
    }
    fn in_corridor(&self, p: Point2) -> bool {
        distance_to_segment(p, self.start, self.goal)
            <= self.options.corridor
                + 32.
                    * f64::EPSILON
                    * distance(p, self.start)
                        .max(distance(self.start, self.goal))
                        .max(1.)
    }
    fn point(&mut self, p: Point2) -> Result<Option<PathPoint>, PathError> {
        Ok(self.terrain.usable(p)?.map(|s| PathPoint {
            position: p,
            height: s.height,
        }))
    }
    fn edge(&mut self, a: Point2, b: Point2) -> Result<Option<f64>, PathError> {
        let delta = b.sub(a);
        let length = delta.length();
        if !length.is_finite() {
            return Err(PathError::InvalidInput);
        }
        let along_steps = steps(
            length,
            self.options.sample_spacing,
            self.options.max_samples,
        )?;
        let radius = self.half_width();
        let mut cross_steps = steps(
            radius * 2.,
            self.options.sample_spacing,
            self.options.max_samples,
        )?;
        if cross_steps % 2 != 0 {
            cross_steps = cross_steps.checked_add(1).ok_or(PathError::SampleBudget)?;
        }
        if cross_steps >= self.options.max_samples {
            return Err(PathError::SampleBudget);
        }
        let normal = if length == 0. {
            Point2 { x: 0., z: 1. }
        } else {
            Point2 {
                x: -delta.z / length,
                z: delta.x / length,
            }
        };
        let mut previous = vec![0.; cross_steps + 1];
        let mut previous_center = None::<TerrainSample>;
        let mut cost = 0.;
        let along_distance = length / along_steps as f64;
        let cross_distance = radius * 2. / cross_steps as f64;
        for i in 0..=along_steps {
            let center = if i == along_steps {
                b
            } else {
                lerp(a, b, i as f64 / along_steps as f64)
            };
            if !self.in_corridor(center) {
                return Ok(None);
            }
            let mut prior_cross = None::<f64>;
            let mut center_sample = None;
            for (j, prior) in previous.iter_mut().enumerate() {
                let offset = -radius + 2. * radius * j as f64 / cross_steps as f64;
                let p = if j == cross_steps / 2 {
                    center
                } else {
                    center.add(normal.scale(offset))
                };
                let sample = match self.terrain.usable(p)? {
                    Some(s) => s,
                    None => return Ok(None),
                };
                if let Some(h) = prior_cross
                    && (sample.height - h).abs()
                        > self.options.max_grade * cross_distance
                            + 16. * f64::EPSILON * sample.height.abs().max(h.abs()).max(1.)
                {
                    return Ok(None);
                }
                if i > 0
                    && (sample.height - *prior).abs()
                        > self.options.max_grade * along_distance
                            + 16. * f64::EPSILON * sample.height.abs().max(prior.abs()).max(1.)
                {
                    return Ok(None);
                }
                *prior = sample.height;
                prior_cross = Some(sample.height);
                if j == cross_steps / 2 {
                    center_sample = Some(sample);
                }
            }
            let current = center_sample.expect("even cross-step count includes center");
            if let Some(prev) = previous_center
                && along_distance > 0.
            {
                let penalty = 6. * (current.height - prev.height).abs() / along_distance;
                cost += along_distance * (1. + penalty * penalty)
                    + (prev.cost * 0.5 + current.cost * 0.5) * 12. * along_distance
                        / self.options.grid_spacing;
                if !cost.is_finite() {
                    return Err(PathError::InvalidTerrain);
                }
            }
            previous_center = Some(current);
        }
        Ok(Some(cost))
    }
    fn dense_polyline(&mut self, input: &[Point2]) -> Result<Option<Vec<PathPoint>>, PathError> {
        let Some(first) = self.point(input[0])? else {
            return Ok(None);
        };
        let mut output = vec![first];
        for pair in input.windows(2) {
            let count = steps(
                distance(pair[0], pair[1]),
                self.options.sample_spacing,
                self.options.max_samples,
            )?;
            for j in 1..=count {
                let p = if j == count {
                    pair[1]
                } else {
                    lerp(pair[0], pair[1], j as f64 / count as f64)
                };
                let Some(point) = self.point(p)? else {
                    return Ok(None);
                };
                output.push(point);
            }
        }
        Ok(Some(output))
    }
    fn smooth(&mut self, raw: &[Point2]) -> Result<Option<Vec<PathPoint>>, PathError> {
        if raw.len() < 2 {
            return self.dense_polyline(raw);
        }
        let mut curve = vec![raw[0]];
        let mut curve_cost = 0.;
        let mut raw_cost = 0.;
        for pair in raw.windows(2) {
            raw_cost += self.edge(pair[0], pair[1])?.ok_or(PathError::NoPath)?;
        }
        for i in 1..raw.len() {
            let a = raw[i.saturating_sub(2)];
            let b = raw[i - 1];
            let c = raw[i];
            let d = raw[(i + 1).min(raw.len() - 1)];
            let ab = b.sub(a);
            let bc = c.sub(b);
            let cd = d.sub(c);
            let bcl = bc.length();
            let bt = ab
                .unit()
                .add(bc.unit())
                .unit()
                .scale((if i == 1 { bcl } else { ab.length().min(bcl) }) / 3.);
            let ct = bc.unit().add(cd.unit()).unit().scale(
                -(if i == raw.len() - 1 {
                    bcl
                } else {
                    bcl.min(cd.length())
                }) / 3.,
            );
            let p1 = b.add(bt);
            let p2 = c.add(ct);
            let control_length = distance(b, p1) + distance(p1, p2) + distance(p2, c);
            let count = steps(
                control_length,
                self.options.sample_spacing,
                self.options.max_samples,
            )?
            .max(3);
            for j in 1..=count {
                let p = if j == count {
                    c
                } else {
                    let t = j as f64 / count as f64;
                    let q0 = lerp(b, p1, t);
                    let q1 = lerp(p1, p2, t);
                    let q2 = lerp(p2, c, t);
                    lerp(lerp(q0, q1, t), lerp(q1, q2, t), t)
                };
                let prev = *curve.last().expect("curve starts with a point");
                let Some(cost) = self.edge(prev, p)? else {
                    return Ok(None);
                };
                curve_cost += cost;
                curve.push(p);
            }
        }
        if curve_cost > raw_cost * 1.05 + 1e-9 {
            return Ok(None);
        }
        self.dense_polyline(&curve)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Node {
    Grid(i32, i32),
    Goal,
}
#[derive(Clone, Copy, Debug)]
struct Entry {
    node: Node,
    g: f64,
    f: f64,
}
impl PartialEq for Entry {
    fn eq(&self, b: &Self) -> bool {
        self.node == b.node
            && self.g.to_bits() == b.g.to_bits()
            && self.f.to_bits() == b.f.to_bits()
    }
}
impl Eq for Entry {}
impl PartialOrd for Entry {
    fn partial_cmp(&self, b: &Self) -> Option<Ordering> {
        Some(self.cmp(b))
    }
}
impl Ord for Entry {
    fn cmp(&self, b: &Self) -> Ordering {
        b.f.total_cmp(&self.f)
            .then_with(|| b.g.total_cmp(&self.g))
            .then_with(|| b.node.cmp(&self.node))
    }
}
const OFFSETS: [(i32, i32); 24] = [
    (2, 0),
    (-2, 0),
    (0, 2),
    (0, -2),
    (2, 2),
    (-2, 2),
    (-2, -2),
    (2, -2),
    (2, 1),
    (-2, 1),
    (-2, -1),
    (2, -1),
    (1, 2),
    (-1, 2),
    (-1, -2),
    (1, -2),
    (3, 0),
    (-3, 0),
    (0, 3),
    (0, -3),
    (3, 3),
    (-3, 3),
    (-3, -3),
    (3, -3),
];
pub(crate) fn valid_input(start: Point2, goal: Point2, o: PathOptions) -> bool {
    let w = o.width;
    if !start.finite()
        || !goal.finite()
        || !o.grid_spacing.is_finite()
        || o.grid_spacing <= 0.
        || !o.corridor.is_finite()
        || o.corridor < 0.
        || !o.sample_spacing.is_finite()
        || o.sample_spacing <= 0.
        || !o.max_grade.is_finite()
        || o.max_grade < 0.
        || o.max_expansions == 0
        || o.max_samples == 0
        || !w.surface.is_finite()
        || w.surface <= 0.
        || !w.shoulder.is_finite()
        || w.shoulder < 0.
        || !w.material.is_finite()
        || w.material < 0.
    {
        return false;
    }
    let radius = (w.surface * 0.5 + w.shoulder).max(w.material * 0.5);
    let extent = distance(start, goal) + o.corridor + radius + 3. * o.grid_spacing;
    extent.is_finite()
        && extent / o.grid_spacing < (i32::MAX as f64 - 4.)
        && (start.x + extent).is_finite()
        && (start.x - extent).is_finite()
        && (start.z + extent).is_finite()
        && (start.z - extent).is_finite()
        && start.x + o.grid_spacing != start.x
        && start.z + o.grid_spacing != start.z
}
fn node_position(node: Node, start: Point2, goal: Point2, spacing: f64) -> Point2 {
    match node {
        Node::Grid(x, z) => Point2 {
            x: start.x + x as f64 * spacing,
            z: start.z + z as f64 * spacing,
        },
        Node::Goal => goal,
    }
}
/// Samples an immutable terrain snapshot. Validation is discrete at sample_spacing.
/// All widths and heights must use the same units as the local planar coordinates.
pub fn plan_path(
    sample: impl FnMut(Point2) -> Option<TerrainSample>,
    start: Point2,
    goal: Point2,
    options: PathOptions,
) -> Result<Path, PathError> {
    if !valid_input(start, goal, options) {
        return Err(PathError::InvalidInput);
    }
    let mut search = Search {
        terrain: SamplerCache {
            sample,
            values: HashMap::new(),
            max_samples: options.max_samples,
            attempts: 0,
        },
        start,
        goal,
        options,
    };
    if start == goal {
        if search.edge(start, goal)?.is_none() {
            return Err(PathError::NoPath);
        }
        let point = search.point(start)?.ok_or(PathError::NoPath)?;
        return Ok(Path {
            raw: vec![point],
            points: vec![point],
            width: options.width,
            expanded: 0,
            sampled: search.terrain.values.len(),
        });
    }
    let origin = Node::Grid(0, 0);
    let mut open = BinaryHeap::new();
    let mut scores = HashMap::new();
    let mut parents = HashMap::new();
    scores.insert(origin, 0.);
    open.push(Entry {
        node: origin,
        g: 0.,
        f: distance(start, goal),
    });
    let mut expanded = 0;
    let mut found = false;
    while let Some(entry) = open.pop() {
        if scores.get(&entry.node).copied() != Some(entry.g) {
            continue;
        }
        if entry.node == Node::Goal {
            found = true;
            break;
        }
        if expanded >= options.max_expansions {
            return Err(PathError::ExpansionBudget);
        }
        expanded += 1;
        let current = node_position(entry.node, start, goal, options.grid_spacing);
        let Node::Grid(x, z) = entry.node else {
            unreachable!()
        };
        let mut candidates = Vec::with_capacity(25);
        if distance(current, goal) <= 2. * options.grid_spacing {
            candidates.push(Node::Goal);
        }
        for (dx, dz) in OFFSETS {
            if let (Some(nx), Some(nz)) = (x.checked_add(dx), z.checked_add(dz)) {
                candidates.push(Node::Grid(nx, nz));
            }
        }
        for next in candidates {
            let position = node_position(next, start, goal, options.grid_spacing);
            if !position.finite() || !search.in_corridor(position) {
                continue;
            }
            let Some(edge_cost) = search.edge(current, position)? else {
                continue;
            };
            let g = entry.g + edge_cost;
            let f = g + distance(position, goal);
            if !g.is_finite() || !f.is_finite() {
                return Err(PathError::InvalidTerrain);
            }
            if scores.get(&next).is_none_or(|old| g < *old) {
                scores.insert(next, g);
                parents.insert(next, entry.node);
                open.push(Entry { node: next, g, f });
            }
        }
    }
    if !found {
        return Err(PathError::NoPath);
    }
    let mut nodes = vec![Node::Goal];
    while *nodes.last().unwrap() != origin {
        nodes.push(
            *parents
                .get(nodes.last().unwrap())
                .expect("discovered node has parent"),
        );
    }
    nodes.reverse();
    let positions: Vec<Point2> = nodes
        .into_iter()
        .map(|n| node_position(n, start, goal, options.grid_spacing))
        .fold(Vec::new(), |mut v, p| {
            if v.last().copied() != Some(p) {
                v.push(p);
            }
            v
        });
    let mut raw = Vec::with_capacity(positions.len());
    for &p in &positions {
        raw.push(search.point(p)?.ok_or(PathError::NoPath)?);
    }
    let points = match search.smooth(&positions)? {
        Some(p) => p,
        None => search
            .dense_polyline(&positions)?
            .ok_or(PathError::NoPath)?,
    };
    Ok(Path {
        raw,
        points,
        width: options.width,
        expanded,
        sampled: search.terrain.values.len(),
    })
}
