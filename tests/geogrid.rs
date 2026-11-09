use layerprocgen::cultivation::{CultivationLayer, PatchBounds, Road};
use layerprocgen::geogrid::*;
use layerprocgen::location::{Location, LocationCell, LocationConnection, LocationConnections};
use layerprocgen::path::{Path, PathOptions, PathPoint, PathWidth, Point2, TerrainSample};
use std::rc::Rc;
fn p(x: f64, z: f64) -> Point2 {
    Point2 { x, z }
}
fn gp(x: i64, z: i64) -> GridPoint {
    GridPoint { x, z }
}
fn base(_: Point2) -> Option<TerrainSample> {
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
fn pad(id: &str, x: f64, z: f64, height: f64) -> SurfacePad {
    SurfacePad {
        id: id.into(),
        center: p(x, z),
        height,
        width: width(4., 2., 4.),
    }
}
fn bounds(x: f64, z: f64, w: f64, h: f64) -> PatchBounds {
    PatchBounds::new(p(x, z), p(x + w, z + h)).unwrap()
}
fn road(a: &str, b: &str, points: &[(f64, f64, f64)], w: PathWidth) -> Rc<Road> {
    let points: Vec<_> = points
        .iter()
        .map(|&(x, z, height)| PathPoint {
            position: p(x, z),
            height,
        })
        .collect();
    Rc::new(Road {
        connection: LocationConnection {
            a: a.into(),
            b: b.into(),
            owner: LocationCell { x: 0, z: 0 },
        },
        path: Path {
            raw: points.clone(),
            points,
            width: w,
            expanded: 0,
            sampled: 0,
        },
        bounds: bounds(-100., -100., 200., 200.),
    })
}
fn request(x: i64, z: i64, w: u32, h: u32) -> GridRequest {
    GridRequest {
        min: gp(x, z),
        cells_x: w,
        cells_z: h,
    }
}
fn roof(id: &str, footprint: PatchBounds, minimum: f64) -> SurfaceProtection {
    SurfaceProtection::BuriedVolume {
        id: id.into(),
        footprint,
        floor: minimum - 20.,
        roof: minimum - 8.,
        cover: 8.,
    }
}
#[test]
fn locations_then_joint_roads_use_weighted_heights_and_maximum_masks() {
    let grid = GeoGridLayer::new(
        1.,
        vec![pad("b", 0., 0., 10.), pad("a", 0., 0., 6.)],
        vec![],
        base,
    )
    .unwrap();
    assert_eq!(grid.sample_vertex(gp(0, 0)).unwrap().height, 8.);
    let a = road("a", "b", &[(-5., 0., 2.), (5., 0., 2.)], width(4., 2., 4.));
    let b = road("c", "d", &[(0., -5., 6.), (0., 5., 6.)], width(4., 2., 4.));
    let view = grid.with_roads(vec![b.clone(), a.clone()]).unwrap();
    let sample = view.sample_vertex(gp(0, 0)).unwrap();
    assert_eq!(sample.height, 4.);
    assert_eq!(sample.road_coverage, 1.);
    assert_eq!(sample.location_coverage, 0.);
    assert_eq!(sample.nearest_location.unwrap().edge_distance, -2.);
    let reversed = grid.with_roads(vec![a, b]).unwrap();
    assert_eq!(
        view.grid(request(-6, -6, 12, 12)).unwrap(),
        reversed.grid(request(-6, -6, 12, 12)).unwrap()
    );
    assert_eq!(
        grid.sample_vertex(gp(0, 0)).unwrap().height,
        8.,
        "final roads do not mutate the planning stage"
    );
}
#[test]
fn influence_matches_reference_equation_and_degenerate_paths_stay_finite() {
    let mut a = pad("a", 0., 0., 4.);
    a.width = width(2., 2., 4.);
    let grid = GeoGridLayer::new(0.5, vec![a], vec![], base).unwrap();
    let sample = grid.sample_vertex(gp(0, 3)).unwrap();
    let influence = 0.75_f64.powi(2) * (3. - 2. * 0.75);
    assert!((sample.height - (20. + (4. - 20.) * influence)).abs() < 1e-12);
    assert_eq!(sample.location_coverage, 1.);
    let duplicate = road("a", "b", &[(0., 0., 7.), (0., 0., 7.)], width(2., 0., 2.));
    let view = grid.with_roads(vec![duplicate]).unwrap();
    assert_eq!(view.sample_vertex(gp(0, 0)).unwrap().height, 7.);
    assert_eq!(view.sample_vertex(gp(20, 20)).unwrap().height, 20.);
    let thin = road(
        "c",
        "d",
        &[(-1., 0., 5.), (1., 0., 5.)],
        width(2., 1e-300, 2.),
    );
    let view = grid.with_roads(vec![thin]).unwrap();
    assert_eq!(view.sample_vertex(gp(0, 0)).unwrap().height, 5.);
}
#[test]
fn shared_global_vertices_match_with_negative_indices_and_fractional_spacing() {
    let grid = GeoGridLayer::new(
        0.3,
        vec![pad("z", 0., 0., 5.), pad("a", 1., 2., 8.)],
        vec![],
        |p: Point2| {
            Some(TerrainSample {
                height: 20. + p.x * 0.1 + p.z * 0.2,
                cost: 0.,
                passable: true,
            })
        },
    )
    .unwrap();
    let a = road(
        "a",
        "b",
        &[(-2., -3., 10.), (4., 3., 11.)],
        width(2., 1., 4.),
    );
    let view = grid.with_roads(vec![a]).unwrap();
    let right = view.grid(request(0, -10, 10, 20)).unwrap();
    let left = view.grid(request(-10, -10, 10, 20)).unwrap();
    for z in -10..=10 {
        assert_eq!(left.vertex(gp(0, z)), right.vertex(gp(0, z)));
    }
    assert_eq!(view.grid(request(-10, -10, 10, 20)).unwrap(), left);
    assert!(left.vertex(gp(-11, 0)).is_none());
    assert!(left.cell(gp(0, 0)).is_none());
}
#[test]
fn covered_footprint_between_vertices_protects_every_touched_cell_corner() {
    let constraints = vec![
        roof("deep", bounds(0.4, 0.4, 0.2, 0.2), 8.),
        roof("high", bounds(0.45, 0.45, 0.1, 0.1), 12.),
    ];
    let grid = GeoGridLayer::new(1., vec![pad("cut", 0.5, 0.5, -40.)], constraints, base).unwrap();
    let patch = grid.grid(request(-1, -1, 3, 3)).unwrap();
    for point in [gp(0, 0), gp(1, 0), gp(0, 1), gp(1, 1)] {
        let s = patch.vertex(point).unwrap();
        assert_eq!(s.height, 12.);
        assert!(s.cover_limited());
        assert!(s.requested_height < 12.);
    }
    for z in 0..=10 {
        for x in 0..=10 {
            assert!(
                grid.terrain_sample(p(x as f64 / 10., z as f64 / 10.))
                    .unwrap()
                    .height
                    >= 12.
            );
        }
    }
}
#[test]
fn preexisting_cover_violation_is_an_error_not_a_climate_repair() {
    let grid = GeoGridLayer::new(
        1.,
        vec![],
        vec![roof("too-high", bounds(0., 0., 1., 1.), 30.)],
        base,
    )
    .unwrap();
    assert!(
        matches!(grid.sample_vertex(gp(0,0)),Err(GeoGridError::InsufficientCover{id,..}) if id=="too-high")
    );
}
#[test]
fn narrow_openings_and_reserved_special_geometry_preserve_base_and_block_roads() {
    let opening = SurfaceProtection::Opening {
        id: "shaft".into(),
        footprint: bounds(0.4, 0.4, 0.1, 0.1),
    };
    let reservation = SurfaceProtection::Reserved {
        id: "hull-extension".into(),
        footprint: bounds(4.4, 0.4, 0.1, 0.1),
    };
    let grid = GeoGridLayer::new(
        1.,
        vec![pad("pad", 0., 0., -50.), pad("other", 5., 0., -60.)],
        vec![opening, reservation],
        base,
    )
    .unwrap();
    let patch = grid.grid(request(-1, -1, 8, 3)).unwrap();
    assert_eq!(patch.cell(gp(0, 0)), Some(SurfacePresence::Opening));
    assert_eq!(patch.cell(gp(4, 0)), Some(SurfacePresence::Reserved));
    let s = patch.vertex(gp(0, 0)).unwrap();
    assert_eq!(s.height, 20.);
    assert_eq!(s.location_coverage, 0.);
    assert!(!grid.terrain_sample(p(0.1, 0.1)).unwrap().passable);
    let r = road("a", "b", &[(-2., 0., 2.), (2., 0., 2.)], width(0.2, 0., 1.));
    assert!(
        matches!(grid.with_roads(vec![r]),Err(GeoGridError::RoadCrossesReservation{protection,..}) if protection=="shaft")
    );
}
#[test]
fn duplicate_roads_do_not_gain_weight_and_conflicting_geometry_fails() {
    let grid = GeoGridLayer::new(1., vec![], vec![], base).unwrap();
    let a = road("a", "b", &[(-5., 0., 2.), (5., 0., 2.)], width(4., 1., 4.));
    let b = road("c", "d", &[(-5., 0., 8.), (5., 0., 8.)], width(4., 1., 4.));
    let one = grid
        .with_roads(vec![a.clone(), b.clone()])
        .unwrap()
        .sample_vertex(gp(0, 0))
        .unwrap();
    let duplicate = grid
        .with_roads(vec![a.clone(), b, a])
        .unwrap()
        .sample_vertex(gp(0, 0))
        .unwrap();
    assert_eq!(one, duplicate);
    let a = road("a", "b", &[(-5., 0., 2.), (5., 0., 2.)], width(4., 1., 4.));
    let changed = road("a", "b", &[(-5., 0., 3.), (5., 0., 3.)], width(4., 1., 4.));
    assert!(matches!(
        grid.with_roads(vec![a, changed]),
        Err(GeoGridError::ConflictingRoad { .. })
    ));
}
#[test]
fn cultivation_patch_owns_complete_neighbor_roads_until_last_unload() {
    let geo = GeoGridLayer::new(1., vec![], vec![], base).unwrap();
    let sites = vec![
        Location {
            id: "a".into(),
            position: p(-5., 0.),
        },
        Location {
            id: "b".into(),
            position: p(0., 0.),
        },
        Location {
            id: "c".into(),
            position: p(5., 0.),
        },
    ];
    let graph = LocationConnections::build(&sites, 360.).unwrap();
    let options = PathOptions {
        corridor: 0.,
        grid_spacing: 1.,
        width: width(2., 1., 4.),
        ..Default::default()
    };
    let mut cultivation =
        CultivationLayer::new(&sites, &graph, options, |p| geo.terrain_sample(p).ok()).unwrap();
    let left = geo
        .compose_patch(&mut cultivation, request(-6, -2, 6, 4))
        .unwrap();
    let right = geo
        .compose_patch(&mut cultivation, request(0, -2, 6, 4))
        .unwrap();
    for z in -2..=2 {
        assert_eq!(left.grid.vertex(gp(0, z)), right.grid.vertex(gp(0, z)));
    }
    assert_eq!(cultivation.retained_road_count(), 2);
    drop(left);
    assert_eq!(cultivation.retained_road_count(), 2);
    let saved = right.grid.clone();
    drop(right);
    assert_eq!(cultivation.retained_road_count(), 0);
    let reloaded = geo
        .compose_patch(&mut cultivation, request(0, -2, 6, 4))
        .unwrap();
    assert_eq!(reloaded.grid, saved);
    drop(reloaded);
    assert_eq!(cultivation.retained_road_count(), 0);
}
#[test]
fn invalid_inputs_and_allocation_budgets_are_checked_at_the_boundary() {
    assert!(GeoGridLayer::new(0., vec![], vec![], base).is_err());
    assert!(
        GeoGridLayer::new(
            1.,
            vec![pad("a", 0., 0., 2.), pad("a", 1., 0., 3.)],
            vec![],
            base
        )
        .is_err()
    );
    let grid = GeoGridLayer::new(1., vec![], vec![], base).unwrap();
    assert!(matches!(
        grid.grid(request(0, 0, u32::MAX, u32::MAX)),
        Err(GeoGridError::Budget)
    ));
    assert!(grid.grid(request(i64::MAX, 0, 1, 1)).is_err());
    assert!(grid.grid(request(0, 0, 0, 1)).is_err());
    let invalid = GeoGridLayer::new(1., vec![], vec![], |_| {
        Some(TerrainSample {
            height: f64::NAN,
            cost: 0.,
            passable: true,
        })
    })
    .unwrap();
    assert!(matches!(
        invalid.sample_vertex(gp(0, 0)),
        Err(GeoGridError::InvalidTerrain(_))
    ));
}

#[test]
fn inclusive_cover_at_fractional_cell_boundary_uses_global_coordinates() {
    let spacing = 0.3;
    let min_x = -24_f64 * spacing;
    let grid = GeoGridLayer::new(
        spacing,
        vec![pad("cut", -7.3, 0.1, -40.)],
        vec![roof("boundary", bounds(min_x, 0.01, 0.01, 0.01), 12.)],
        base,
    )
    .unwrap();
    assert_eq!(grid.sample_vertex(gp(-25, 0)).unwrap().height, 12.);
}

#[test]
fn real_rfl_plate_and_orbital_roofs_survive_surface_cutting() {
    let fixture = include_str!("../fixtures/rfl-structures/volumes.tsv");
    let mut presets = Vec::new();
    for line in fixture.lines().skip(1) {
        let columns: Vec<_> = line.split('\t').collect();
        assert_eq!(columns.len(), 10);
        let values: Vec<f64> = columns[2..]
            .iter()
            .map(|value| value.parse().unwrap())
            .collect();
        let [min_x, min_z, max_x, max_z, floor, roof, cover, base_minimum]: [f64; 8] =
            values.try_into().unwrap();
        let center = p((min_x + max_x) * 0.5, (min_z + max_z) * 0.5);
        let protection = SurfaceProtection::BuriedVolume {
            id: format!("{}/{}", columns[0], columns[1]),
            footprint: bounds(min_x, min_z, max_x - min_x, max_z - min_z),
            floor,
            roof,
            cover,
        };
        let mut cut = pad("surface-cut", center.x, center.z, floor - 100.);
        cut.width = width(1000., 100., 1000.);
        let grid = GeoGridLayer::new(1., vec![cut], vec![protection], move |_| {
            Some(TerrainSample {
                height: base_minimum,
                cost: 0.,
                passable: true,
            })
        })
        .unwrap();
        let point = gp(center.x.round() as i64, center.z.round() as i64);
        let sample = grid.sample_vertex(point).unwrap();
        assert_eq!(sample.height, roof + cover);
        assert!(sample.cover_limited());
        assert!(
            sample.height > floor,
            "an underground floor is not a surface target"
        );
        presets.push(columns[0]);
    }
    assert_eq!(presets, vec!["plate", "orbital"]);
}

#[test]
fn failed_grid_publication_releases_new_roads_and_keeps_previous_demand() {
    let geo = GeoGridLayer::new(
        1.,
        vec![],
        vec![],
        |p: Point2| if p.x == 5. { None } else { base(p) },
    )
    .unwrap();
    let sites = vec![
        Location {
            id: "a".into(),
            position: p(0., 0.),
        },
        Location {
            id: "b".into(),
            position: p(4., 0.),
        },
        Location {
            id: "c".into(),
            position: p(8., 0.),
        },
    ];
    let graph = LocationConnections::build(&sites, 360.).unwrap();
    let mut cultivation = CultivationLayer::new(
        &sites,
        &graph,
        PathOptions {
            corridor: 0.,
            width: width(0.2, 0., 0.2),
            ..Default::default()
        },
        base,
    )
    .unwrap();
    let previous = cultivation.retain_patch(bounds(0., -0.1, 1., 0.2)).unwrap();
    assert_eq!(cultivation.retained_road_count(), 1);
    assert!(matches!(
        geo.compose_patch(&mut cultivation, request(0, 0, 8, 1)),
        Err(GeoGridError::MissingTerrain(_))
    ));
    assert_eq!(cultivation.retained_road_count(), 1);
    drop(previous);
    assert_eq!(cultivation.retained_road_count(), 0);
}

#[test]
fn reservation_work_is_bounded_before_intersection_scanning() {
    let protections = (0..3000)
        .map(|i| SurfaceProtection::Reserved {
            id: format!("reserved-{i}"),
            footprint: bounds(100., 100., 1., 1.),
        })
        .collect();
    let grid = GeoGridLayer::new(1., vec![], protections, base).unwrap();
    let points = vec![(0., 0., 0.); 8192];
    let r = road("a", "b", &points, width(1., 1., 1.));
    assert!(matches!(
        grid.with_roads(vec![r]),
        Err(GeoGridError::Budget)
    ));
}

#[test]
fn unequal_shoulders_preserve_reference_relative_weights() {
    let grid = GeoGridLayer::new(1., vec![], vec![], base).unwrap();
    let a = road("a", "b", &[(-5., 0., 2.), (5., 0., 2.)], width(2., 1., 4.));
    let b = road("c", "d", &[(-5., 0., 6.), (5., 0., 6.)], width(2., 2., 4.));
    let sample = grid
        .with_roads(vec![a, b])
        .unwrap()
        .sample_vertex(gp(0, 0))
        .unwrap();
    let expected = (2_f64.powi(8) * 2. + 1.5_f64.powi(8) * 6.) / (2_f64.powi(8) + 1.5_f64.powi(8));
    assert!((sample.height - expected).abs() < 1e-12);
}

#[test]
fn independent_material_width_can_extend_beyond_grading() {
    let grid = GeoGridLayer::new(1., vec![], vec![], base).unwrap();
    let r = road("a", "b", &[(-5., 0., 2.), (5., 0., 2.)], width(2., 1., 10.));
    let sample = grid
        .with_roads(vec![r])
        .unwrap()
        .sample_vertex(gp(0, 3))
        .unwrap();
    assert_eq!(sample.height, 20.);
    assert_eq!(sample.road_coverage, 1.);
}

#[test]
fn cell_road_footprint_detects_thin_roads_between_vertices() {
    let layer = GeoGridLayer::new(1., vec![], vec![], base).unwrap();
    let thin = road(
        "a",
        "b",
        &[(0.4, 0.5, 20.), (0.6, 0.5, 20.)],
        width(0.1, 0., 0.1),
    );
    let grid = layer
        .with_roads(vec![thin])
        .unwrap()
        .grid(request(0, 0, 2, 1))
        .unwrap();
    for point in [gp(0, 0), gp(1, 0), gp(0, 1), gp(1, 1)] {
        assert_eq!(grid.vertex(point).unwrap().road_coverage, 0.);
    }
    assert_eq!(grid.cell_has_road(gp(0, 0)), Some(true));
    assert_eq!(grid.cell_has_road(gp(1, 0)), Some(false));
    assert_eq!(grid.cell_has_road(gp(2, 0)), None);
    assert_eq!(
        layer
            .grid(request(0, 0, 1, 1))
            .unwrap()
            .cell_has_road(gp(0, 0)),
        Some(false)
    );
}

#[test]
fn point_road_footprint_uses_global_fractional_cells() {
    let layer = GeoGridLayer::new(0.3, vec![], vec![], base).unwrap();
    let point = road("a", "b", &[(-0.15, -0.15, 20.)], width(0.01, 0.01, 0.02));
    let grid = layer
        .with_roads(vec![point])
        .unwrap()
        .grid(request(-1, -1, 2, 2))
        .unwrap();
    assert_eq!(grid.cell_has_road(gp(-1, -1)), Some(true));
    assert_eq!(grid.cell_has_road(gp(0, 0)), Some(false));
}
