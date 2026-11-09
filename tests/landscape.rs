use layerprocgen::cultivation::{PatchBounds, Road};
use layerprocgen::geogrid::*;
use layerprocgen::landscape::*;
use layerprocgen::location::{LocationCell, LocationConnection};
use layerprocgen::path::{Path, PathPoint, PathWidth, Point2, TerrainSample};
use std::rc::Rc;
fn p(x: f64, z: f64) -> Point2 {
    Point2 { x, z }
}
fn gp(x: i64, z: i64) -> GridPoint {
    GridPoint { x, z }
}
fn request(x: i64, z: i64, w: u32, h: u32) -> LandscapeRequest {
    LandscapeRequest {
        region: GridRequest {
            min: gp(x, z),
            cells_x: w,
            cells_z: h,
        },
        seed: 123,
    }
}
fn flat(_: Point2) -> Option<TerrainSample> {
    Some(TerrainSample {
        height: 20.,
        cost: 0.,
        passable: true,
    })
}
fn width(surface: f64, shoulder: f64, material: f64) -> PathWidth {
    PathWidth {
        surface,
        shoulder,
        material,
    }
}
fn bounds(x: f64, z: f64, w: f64, h: f64) -> PatchBounds {
    PatchBounds::new(p(x, z), p(x + w, z + h)).unwrap()
}
fn road(points: &[(f64, f64, f64)], width: PathWidth) -> Rc<Road> {
    let points: Vec<_> = points
        .iter()
        .map(|&(x, z, height)| PathPoint {
            position: p(x, z),
            height,
        })
        .collect();
    Rc::new(Road {
        connection: LocationConnection {
            a: "a".into(),
            b: "b".into(),
            owner: LocationCell { x: 0, z: 0 },
        },
        path: Path {
            raw: points.clone(),
            points,
            width,
            expanded: 0,
            sampled: 0,
        },
        bounds: bounds(-100., -100., 200., 200.),
    })
}
#[test]
fn flat_climate_surface_has_unit_up_normal_and_source_density() {
    let geo = GeoGridLayer::new(1., vec![], vec![], flat).unwrap();
    let req = request(-2, -2, 4, 4);
    let result = req
        .build(geo.grid(req.source_request().unwrap()).unwrap(), |_| 1.)
        .unwrap();
    for v in result.vertices() {
        assert_eq!(v.normal, [0., 1., 0.]);
        assert_eq!(v.height, 20.);
        assert_eq!(v.material.climate(), 1.);
        assert_eq!(v.material.cliff(), 0.);
        assert_eq!(v.material.path(), 0.);
    }
    assert!(
        result
            .cells()
            .iter()
            .all(|c| c.vegetation_count == 10 && !c.road_excluded)
    );
    assert_eq!(result.request(), req.region);
    assert_eq!(result.spacing(), 1.);
    assert!(result.vertex(gp(3, 0)).is_none());
    assert!(result.cell(gp(2, 0)).is_none());
}
#[test]
fn physical_normals_and_cliff_path_suppression_match_the_height_field() {
    for y in [0.6501_f64, 0.6499] {
        let slope = (1. / (y * y) - 1.).sqrt();
        let geo = GeoGridLayer::new(1., vec![], vec![], move |p: Point2| {
            Some(TerrainSample {
                height: 20. + slope * p.x,
                cost: 0.,
                passable: true,
            })
        })
        .unwrap();
        let r = road(
            &[(-5., 0., 20. - slope * 5.), (5., 0., 20. + slope * 5.)],
            width(2., 1., 2.),
        );
        let view = geo.with_roads(vec![r]).unwrap();
        let req = request(-1, -1, 2, 2);
        let out = req
            .build(view.grid(req.source_request().unwrap()).unwrap(), |_| 1.)
            .unwrap();
        let v = out.vertex(gp(0, 0)).unwrap();
        assert!((v.normal[1] - y).abs() < 1e-12);
        assert!(v.normal[0] < 0.);
        if y < 0.65 {
            assert_eq!(v.material.cliff(), 1.);
            assert_eq!(v.material.path(), 0.);
        } else {
            assert_eq!(v.material.cliff(), 0.);
            assert_eq!(v.material.path(), 1.);
        }
        assert_eq!(
            v.material.climate() + v.material.cliff() + v.material.path(),
            1.
        );
        assert!(out.cells().iter().all(|c| c.vegetation_count == 0));
    }
}
#[test]
fn split_fractional_patches_match_one_combined_landscape() {
    let geo = GeoGridLayer::new(
        0.3,
        vec![],
        vec![
            SurfaceProtection::Opening {
                id: "border-opening".into(),
                footprint: bounds(-0.04, 1.12, 0.08, 0.04),
            },
            SurfaceProtection::BuriedVolume {
                id: "under-grid".into(),
                footprint: bounds(-1.55, -0.05, 0.1, 0.1),
                floor: -10.,
                roof: 0.,
                cover: 8.,
            },
        ],
        |p: Point2| {
            Some(TerrainSample {
                height: 20. + p.x * p.x * 0.02 + p.z * p.z * 0.01,
                cost: 0.,
                passable: true,
            })
        },
    )
    .unwrap();
    let r = road(&[(-2., -2., 20.), (2., 2., 20.)], width(0.4, 0.3, 0.6));
    let mut crossing = road(&[(-2., 2., 20.), (2., -2., 20.)], width(0.4, 0.3, 0.6));
    let connection = &mut Rc::get_mut(&mut crossing).unwrap().connection;
    connection.a = "c".into();
    connection.b = "d".into();
    let view = geo.with_roads(vec![r, crossing]).unwrap();
    let plant = |p: GridPoint| 0.5 + (p.x + p.z).rem_euclid(5) as f64 * 0.05;
    let build = |req: LandscapeRequest| {
        req.build(view.grid(req.source_request().unwrap()).unwrap(), plant)
            .unwrap()
    };
    let right = build(request(0, -4, 8, 8));
    let left = build(request(-8, -4, 8, 8));
    let combined = build(request(-8, -4, 16, 8));
    for z in -4..=4 {
        assert_eq!(left.vertex(gp(0, z)), right.vertex(gp(0, z)));
    }
    for z in -4..=4 {
        for x in -8..=8 {
            let piece = if x < 0 { &left } else { &right };
            assert_eq!(piece.vertex(gp(x, z)), combined.vertex(gp(x, z)));
        }
    }
    for z in -4..4 {
        for x in -8..8 {
            let piece = if x < 0 { &left } else { &right };
            assert_eq!(piece.cell(gp(x, z)), combined.cell(gp(x, z)));
        }
    }
    assert!((combined.vertex(gp(0, 0)).unwrap().material.path() - 1.).abs() < 1e-12);
    assert_eq!(
        left.cell(gp(-1, 3)).unwrap().surface,
        SurfacePresence::Opening
    );
    assert_eq!(
        right.cell(gp(0, 3)).unwrap().surface,
        SurfacePresence::Opening
    );
    assert_eq!(right.cell(gp(0, 3)).unwrap().vegetation_count, 0);
    assert_eq!(build(request(-8, -4, 8, 8)), left);
}
#[test]
fn thin_road_and_point_road_exclude_cells_even_with_zero_vertex_masks() {
    let geo = GeoGridLayer::new(1., vec![], vec![], flat).unwrap();
    let req = request(0, 0, 2, 2);
    for points in [
        vec![(0.4, 0.5, 20.), (0.6, 0.5, 20.)],
        vec![(0.5, 0.5, 20.)],
    ] {
        let view = geo
            .with_roads(vec![road(&points, width(0.1, 0., 0.1))])
            .unwrap();
        let halo = view.grid(req.source_request().unwrap()).unwrap();
        for point in [gp(0, 0), gp(1, 0), gp(0, 1), gp(1, 1)] {
            assert_eq!(halo.vertex(point).unwrap().road_coverage, 0.);
        }
        let out = req.build(halo, |_| 1.).unwrap();
        assert!(out.cell(gp(0, 0)).unwrap().road_excluded);
        assert_eq!(out.cell(gp(0, 0)).unwrap().vegetation_count, 0);
        assert_eq!(out.cell(gp(1, 1)).unwrap().vegetation_count, 10);
    }
}
#[test]
fn climate_suitability_controls_density_without_changing_materials() {
    let geo = GeoGridLayer::new(1., vec![], vec![], flat).unwrap();
    let req = request(0, 0, 1, 1);
    let wet = req
        .build(geo.grid(req.source_request().unwrap()).unwrap(), |_| 1.)
        .unwrap();
    let dry = req
        .build(geo.grid(req.source_request().unwrap()).unwrap(), |_| 0.)
        .unwrap();
    for (a, b) in wet.vertices().iter().zip(dry.vertices()) {
        assert_eq!(a.material, b.material);
        assert_eq!(a.normal, b.normal);
        assert_eq!(a.height, b.height);
    }
    assert_eq!(dry.cells()[0].vegetation_count, 0);
    assert_eq!(wet.cells()[0].vegetation_count, 10);
    let cutoff = req
        .build(geo.grid(req.source_request().unwrap()).unwrap(), |_| 0.4)
        .unwrap();
    assert_eq!(cutoff.cells()[0].vegetation_count, 0);
    let mixed = req
        .build(geo.grid(req.source_request().unwrap()).unwrap(), |p| {
            if p == gp(0, 0) { 0. } else { 1. }
        })
        .unwrap();
    assert_eq!(mixed.cells()[0].vegetation_count, 10);
    let water_edge = GeoGridLayer::new(1., vec![], vec![], |point: Point2| {
        Some(TerrainSample {
            height: 20.,
            cost: 0.,
            passable: point.x >= 1.,
        })
    })
    .unwrap();
    let request = request(0, 0, 2, 1);
    let edge = request
        .build(
            water_edge.grid(request.source_request().unwrap()).unwrap(),
            |_| 1.,
        )
        .unwrap();
    assert_eq!(edge.cell(gp(0, 0)).unwrap().vegetation_count, 0);
    assert_eq!(edge.cell(gp(1, 0)).unwrap().vegetation_count, 10);
}
#[test]
fn global_cell_and_seed_high_bits_affect_density_and_reload_is_repeatable() {
    let geo = GeoGridLayer::new(1., vec![], vec![], flat).unwrap();
    let counts = |req: LandscapeRequest| {
        req.build(geo.grid(req.source_request().unwrap()).unwrap(), |_| 0.65)
            .unwrap()
            .cells()
            .iter()
            .map(|c| c.vegetation_count)
            .collect::<Vec<_>>()
    };
    let req = request(-8, -4, 16, 8);
    let initial = counts(req);
    assert!(initial.contains(&6) && initial.contains(&7));
    assert_eq!(counts(req), initial);
    let mut seed = req;
    seed.seed += 1_u64 << 32;
    assert_ne!(counts(seed), initial);
    let mut shifted = req;
    shifted.region.min.x += 1_i64 << 32;
    assert_ne!(counts(shifted), initial);
    let mut shifted_z = req;
    shifted_z.region.min.z += 1_i64 << 32;
    assert_ne!(counts(shifted_z), initial);
    let mut positive = req;
    positive.region.min.x = -positive.region.min.x;
    assert_ne!(counts(positive), initial);
}
#[test]
fn underground_cover_and_opening_topology_survive_landscape_derivation() {
    let buried = SurfaceProtection::BuriedVolume {
        id: "room".into(),
        footprint: bounds(-10., -10., 20., 20.),
        floor: -20.,
        roof: 4.,
        cover: 8.,
    };
    let pad = SurfacePad {
        id: "cut".into(),
        center: p(0., 0.),
        height: -10.,
        width: width(30., 1., 0.),
    };
    let geo = GeoGridLayer::new(1., vec![pad], vec![buried], flat).unwrap();
    let req = request(-2, -2, 4, 4);
    let halo = geo.grid(req.source_request().unwrap()).unwrap();
    assert!(halo.vertex(gp(0, 0)).unwrap().cover_limited());
    let out = req.build(halo, |_| 1.).unwrap();
    assert!(out.vertices().iter().all(|v| v.height == 12.));
    assert!(
        out.cells()
            .iter()
            .all(|c| c.surface == SurfacePresence::Solid && c.vegetation_count == 10)
    );
    let protections = vec![
        SurfaceProtection::Opening {
            id: "shaft".into(),
            footprint: bounds(0.4, 0.4, 0.1, 0.1),
        },
        SurfaceProtection::Reserved {
            id: "special-hull".into(),
            footprint: bounds(3.4, 0.4, 0.1, 0.1),
        },
    ];
    let geo = GeoGridLayer::new(1., vec![], protections, flat).unwrap();
    let req = request(-2, -2, 8, 4);
    let out = req
        .build(geo.grid(req.source_request().unwrap()).unwrap(), |_| 1.)
        .unwrap();
    assert_eq!(
        out.cell(gp(0, 0)).unwrap().surface,
        SurfacePresence::Opening
    );
    assert_eq!(out.cell(gp(0, 0)).unwrap().vegetation_count, 0);
    assert_eq!(
        out.cell(gp(3, 0)).unwrap().surface,
        SurfacePresence::Reserved
    );
    assert_eq!(out.cell(gp(3, 0)).unwrap().vegetation_count, 0);
    assert!(out.vertices().iter().all(|v| v.height == 20.));
}
#[test]
fn malformed_halos_climate_values_and_requests_fail_before_output() {
    let geo = GeoGridLayer::new(1., vec![], vec![], flat).unwrap();
    let req = request(0, 0, 1, 1);
    assert!(matches!(
        req.build(geo.grid(req.region).unwrap(), |_| 1.),
        Err(LandscapeError::InvalidHalo)
    ));
    let fresh = || geo.grid(req.source_request().unwrap()).unwrap();
    for value in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        assert!(matches!(
            req.build(fresh(), |_| value),
            Err(LandscapeError::InvalidPlantability { .. })
        ));
    }
    assert!(request(i64::MIN, 0, 1, 1).source_request().is_err());
    assert!(request(i64::MAX, 0, 1, 1).source_request().is_err());
    assert!(matches!(
        request(0, 0, u32::MAX, 1).source_request(),
        Err(LandscapeError::Budget)
    ));
    assert!(request(0, 0, 0, 1).source_request().is_err());
    assert!(matches!(
        request(0, 0, 10000, 10000).source_request(),
        Err(LandscapeError::Budget)
    ));
}

#[test]
fn frozen_rfl_native_ground_has_matching_patch_materials_and_original_heights() {
    let bytes = include_bytes!("../fixtures/rfl-route/ground.f64");
    assert_eq!(bytes.len(), 129 * 65 * 8);
    let sample = |point: Point2| {
        let x = point.x / 2. + 64.;
        let z = point.z / 2. + 32.;
        if x < 0. || z < 0. || x > 128. || z > 64. || x.fract() != 0. || z.fract() != 0. {
            return None;
        }
        let offset = (z as usize * 129 + x as usize) * 8;
        let height = f64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
        Some(TerrainSample {
            height,
            cost: 0.,
            passable: height > 0.,
        })
    };
    let geo = GeoGridLayer::new(2., vec![], vec![], sample).unwrap();
    let left_req = request(-63, -31, 63, 62);
    let right_req = request(0, -31, 63, 62);
    let left = left_req
        .build(
            geo.grid(left_req.source_request().unwrap()).unwrap(),
            |_| 0.65,
        )
        .unwrap();
    let right = right_req
        .build(
            geo.grid(right_req.source_request().unwrap()).unwrap(),
            |_| 0.65,
        )
        .unwrap();
    for z in -31..=31 {
        assert_eq!(left.vertex(gp(0, z)), right.vertex(gp(0, z)));
    }
    for grid in [&left, &right] {
        let r = grid.request();
        for z in r.min.z..=r.min.z + i64::from(r.cells_z) {
            for x in r.min.x..=r.min.x + i64::from(r.cells_x) {
                let v = grid.vertex(gp(x, z)).unwrap();
                assert_eq!(
                    v.height,
                    sample(p(x as f64 * 2., z as f64 * 2.)).unwrap().height
                );
                assert!(
                    (v.material.climate() + v.material.cliff() + v.material.path() - 1.).abs()
                        < 1e-12
                );
            }
        }
    }
}
