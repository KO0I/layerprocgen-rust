use layerprocgen::path::{PathOptions, PathWidth, Point2, TerrainSample, plan_path};
use rfl_world::{BaseTerrain, SurfaceTerrain, SurfaceTerrainConfig};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn number(v: &Value, k: &str) -> f64 {
    v[k].as_f64().unwrap()
}
fn triple(v: &Value) -> [f64; 3] {
    std::array::from_fn(|i| v[i].as_f64().unwrap())
}
fn terrain(root: &Path, m: &Value) -> SurfaceTerrain {
    let heights = fs::read(root.join("heights.f32"))
        .unwrap()
        .chunks_exact(4)
        .map(|p| f32::from_le_bytes(p.try_into().unwrap()))
        .collect();
    SurfaceTerrain::new(SurfaceTerrainConfig {
        body_radius: number(m, "radius"),
        base: BaseTerrain::ClimateMap {
            width: m["width"].as_u64().unwrap() as usize,
            rows: m["rows"].as_u64().unwrap() as usize,
            heights,
        },
        basin_seed: m["seed"].as_u64().unwrap() as u32,
        elevation_scale: 1.,
        ocean_scale: 1.,
        basins: fs::read(root.join("basins.u8")).unwrap(),
        sea_level: Some(number(m, "seaLevel")),
        impacts: vec![],
        clamp_depth: true,
    })
    .unwrap()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(std::env::args().nth(1).expect("fixture directory"));
    let m: Value = serde_json::from_slice(&fs::read(root.join("manifest.json"))?)?;
    let surface = terrain(&root, &m);
    let center = triple(&m["frame"]["center"]);
    let right = triple(&m["frame"]["right"]);
    let forward = triple(&m["frame"]["forward"]);
    let radius = number(&m, "radius");
    let direction = |p: Point2| {
        let q = std::array::from_fn::<_, 3, _>(|i| {
            center[i] + (right[i] * p.x + forward[i] * p.z) / radius
        });
        let n = q.iter().map(|v| v * v).sum::<f64>().sqrt();
        q.map(|v| v / n)
    };
    let height = |p: Point2| surface.elevation(direction(p)).unwrap();
    let sample = |s: &SurfaceTerrain, p: Point2| {
        if p.x < -128. || p.x > 128. || p.z < -64. || p.z > 64. {
            return None;
        }
        let h = s.elevation(direction(p)).unwrap();
        Some(TerrainSample {
            height: h,
            cost: 0.,
            passable: h > number(&m, "seaLevel"),
        })
    };
    let point = |v: &Value| Point2 {
        x: number(v, "x"),
        z: number(v, "z"),
    };
    let start = point(&m["sites"][0]["chart"]);
    let goal = point(&m["sites"][1]["chart"]);
    let options = PathOptions {
        corridor: 30.,
        sample_spacing: 1.,
        width: PathWidth {
            surface: 4.,
            shoulder: 1.,
            material: 5.,
        },
        ..PathOptions::default()
    };
    let route = plan_path(|p| sample(&surface, p), start, goal, options)?;
    assert_eq!(
        route,
        plan_path(|p| sample(&surface, p), start, goal, options)?,
        "same immutable world must repeat exactly"
    );
    let recreated = terrain(&root, &m);
    assert_eq!(
        route,
        plan_path(|p| sample(&recreated, p), start, goal, options)?,
        "reloading the fixed world must repeat exactly"
    );
    assert_eq!(route.points.first().unwrap().position, start);
    assert_eq!(route.points.last().unwrap().position, goal);
    for (site, p) in m["sites"].as_array().unwrap().iter().zip([start, goal]) {
        assert!(
            (height(p) - number(site, "h")).abs() < 1e-8,
            "existing RFL site elevation"
        );
        let body_position = direction(p).map(|v| v * (radius + height(p)));
        for (a, b) in body_position.into_iter().zip(triple(&site["p"])) {
            assert!((a - b).abs() < 1e-8, "existing RFL site body position");
        }
    }
    let grid = &m["grid"];
    let ground = fs::read(root.join("ground.f64"))?;
    let width = grid["width"].as_u64().unwrap() as usize;
    let mut max_error: f64 = 0.;
    for (i, b) in ground.chunks_exact(8).enumerate() {
        let p = Point2 {
            x: number(grid, "minX") + (i % width) as f64 * number(grid, "spacing"),
            z: number(grid, "minZ") + (i / width) as f64 * number(grid, "spacing"),
        };
        max_error = max_error.max((height(p) - f64::from_le_bytes(b.try_into().unwrap())).abs());
    }
    assert!(
        max_error < 1e-8,
        "actual Rust/JS elevation disagreement {max_error}"
    );
    for p in &route.points {
        assert_eq!(p.height, height(p.position));
        assert!(p.height > 0.);
    }
    // Independently probe returned segment footprints at quarter-unit spacing.
    let half_width =
        (route.width.surface / 2. + route.width.shoulder).max(route.width.material / 2.);
    let cross_steps = (half_width * 2. / 0.25).ceil() as usize;
    for pair in route.points.windows(2) {
        let a = pair[0];
        let b = pair[1];
        let dx = b.position.x - a.position.x;
        let dz = b.position.z - a.position.z;
        let d = dx.hypot(dz);
        assert!(d > 0. && d <= options.sample_spacing + 1e-9);
        assert!((a.height - b.height).abs() <= options.max_grade * d + 1e-9);
        let along_steps = (d / 0.25).ceil() as usize;
        let mut previous = vec![0.; cross_steps + 1];
        for i in 0..=along_steps {
            let mut cross: Option<f64> = None;
            for (j, prior) in previous.iter_mut().enumerate() {
                let offset = -half_width + 2. * half_width * j as f64 / cross_steps as f64;
                let p = Point2 {
                    x: a.position.x + dx * i as f64 / along_steps as f64 - dz / d * offset,
                    z: a.position.z + dz * i as f64 / along_steps as f64 + dx / d * offset,
                };
                let h = sample(&surface, p)
                    .expect("route footprint stays in patch")
                    .height;
                assert!(h > number(&m, "seaLevel"), "dry footprint");
                if i > 0 {
                    assert!(
                        (h - *prior).abs() <= options.max_grade * d / along_steps as f64 + 1e-9
                    );
                }
                if let Some(c) = cross {
                    assert!(
                        (h - c).abs()
                            <= options.max_grade * 2. * half_width / cross_steps as f64 + 1e-9
                    );
                }
                *prior = h;
                cross = Some(h);
            }
        }
    }
    let encode = |points: &[layerprocgen::path::PathPoint]| {
        points
            .iter()
            .map(|p| json!([p.position.x, p.height, p.position.z]))
            .collect::<Vec<_>>()
    };
    let body_points = route
        .points
        .iter()
        .map(|p| direction(p.position).map(|v| v * (radius + p.height)))
        .collect::<Vec<_>>();
    let result = json!({"schema":1,"sampler":"rfl_world::SurfaceTerrain::elevation",
        "coordinateSystem":"local-gnomonic-x-height-z","snapshotSha256":m["snapshotSha256"],
        "siteIds":[m["sites"][0]["id"],m["sites"][1]["id"]],"raw":encode(&route.raw),
        "points":encode(&route.points),"bodyLocalPoints":body_points,
        "width":{"surface":route.width.surface,"shoulder":route.width.shoulder,"material":route.width.material},
        "options":{"gridSpacing":options.grid_spacing,"corridor":options.corridor,
            "sampleSpacing":options.sample_spacing,"maxGrade":options.max_grade,
            "maxExpansions":options.max_expansions,"maxSamples":options.max_samples},
        "expanded":route.expanded,"sampled":route.sampled,"maxRustJsHeightError":max_error});
    fs::write(root.join("route.json"), serde_json::to_vec_pretty(&result)?)?;
    println!(
        "PASS actual RFL Rust sampler; {} raw nodes, {} smoothed points, {} unique samples, Rust/JS max error {}",
        route.raw.len(),
        route.points.len(),
        route.sampled,
        max_error
    );
    Ok(())
}
