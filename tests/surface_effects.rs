use layerprocgen::cultivation::{PatchBounds, Road};
use layerprocgen::geogrid::*;
use layerprocgen::location::{Location, LocationCell, LocationConnection, LocationConnections};
use layerprocgen::path::{Path, PathPoint, PathWidth, Point2, TerrainSample};
use std::rc::Rc;
fn p(x: f64, z: f64) -> Point2 {
    Point2 { x, z }
}
fn base(p: Point2) -> Option<TerrainSample> {
    Some(TerrainSample {
        height: 20. + p.x * 0.1 - p.z * 0.05,
        cost: 0.,
        passable: true,
    })
}
fn road(a: &str, b: &str, points: &[(f64, f64, f64)]) -> Rc<Road> {
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
            width: PathWidth {
                surface: 2.,
                shoulder: 1.,
                material: 4.,
            },
            expanded: 0,
            sampled: 0,
        },
        bounds: PatchBounds::new(p(-20., -20.), p(20., 20.)).unwrap(),
    })
}
#[test]
fn indexed_owned_effects_match_grid_kernel_and_outlive_the_source() {
    let effects = {
        let pads = vec![SurfacePad {
            id: "site".into(),
            center: p(-2., 3.),
            height: 8.,
            width: PathWidth {
                surface: 2.,
                shoulder: 2.,
                material: 3.,
            },
        }];
        let geo = GeoGridLayer::new(0.25, pads, vec![], base).unwrap();
        let roads = vec![
            road("a", "b", &[(-5., 0., 2.), (0., 1., 4.), (5., 0., 6.)]),
            road("c", "d", &[(0., -5., 6.), (0., 5., 2.)]),
        ];
        let composition = geo.with_roads(roads).unwrap();
        let effects = composition.surface_effects().unwrap();
        for z in -40..=40 {
            for x in -40..=40 {
                let point = p(x as f64 * 0.25, z as f64 * 0.25);
                let expected = composition.sample_vertex(GridPoint { x, z }).unwrap();
                let actual = effects.sample(point, base(point).unwrap().height).unwrap();
                assert_eq!(actual.height, expected.height);
                assert_eq!(actual.location_coverage, expected.location_coverage);
                assert_eq!(actual.road_coverage, expected.road_coverage);
            }
        }
        effects
    };
    assert_eq!(effects.sample(p(100., 100.), 123.5).unwrap().height, 123.5);
    assert!(effects.elevation_bound() >= 6.);
    assert!(effects.bounds().is_some());
    assert!(effects.sample(p(f64::NAN, 0.), 0.).is_err());
    assert!(effects.sample(p(0., 0.), f64::INFINITY).is_err());
}
#[test]
fn scalar_effect_export_explicitly_rejects_protected_geometry() {
    let footprint = PatchBounds::new(p(0., 0.), p(1., 1.)).unwrap();
    for protection in [
        SurfaceProtection::Opening {
            id: "opening".into(),
            footprint,
        },
        SurfaceProtection::Reserved {
            id: "hull".into(),
            footprint,
        },
        SurfaceProtection::BuriedVolume {
            id: "room".into(),
            footprint,
            floor: -10.,
            roof: 0.,
            cover: 8.,
        },
    ] {
        let geo = GeoGridLayer::new(1., vec![], vec![protection], base).unwrap();
        let composition = geo.with_roads(Vec::new()).unwrap();
        assert!(matches!(
            composition.surface_effects(),
            Err(GeoGridError::ProtectedEffectsRequireGrid)
        ));
    }
}
#[test]
fn explicit_grid_origin_connects_finite_patch_without_moving_sites() {
    let sites = vec![
        Location {
            id: "a".into(),
            position: p(-5., -5.),
        },
        Location {
            id: "b".into(),
            position: p(5., 5.),
        },
        Location {
            id: "c".into(),
            position: p(6., 6.),
        },
    ];
    let before = sites.clone();
    assert_eq!(
        LocationConnections::build(&sites, 20.)
            .unwrap()
            .connections
            .len(),
        1
    );
    let graph = LocationConnections::build_with_origin(&sites, 20., p(-10., -10.)).unwrap();
    assert_eq!(graph.connections.len(), 2);
    assert_eq!(sites, before);
    assert!(
        graph
            .connections
            .iter()
            .all(|e| e.owner == LocationCell { x: 0, z: 0 })
    );
    let reversed: Vec<_> = sites.into_iter().rev().collect();
    assert_eq!(
        LocationConnections::build_with_origin(&reversed, 20., p(-10., -10.)).unwrap(),
        graph
    );
    assert!(LocationConnections::build_with_origin(&reversed, 20., p(f64::NAN, 0.)).is_err());
}

#[test]
fn indexed_and_grid_effects_agree_near_large_segment_endpoints() {
    let x = f64::from_bits(250_000_000_000_f64.to_bits() + 1);
    let geo = GeoGridLayer::new(x, vec![], vec![], |_| {
        Some(TerrainSample {
            height: 0.,
            cost: 0.,
            passable: true,
        })
    })
    .unwrap();
    let mut line = road(
        "a",
        "b",
        &[
            (-1e12, 0., 0.),
            (250_000_000_000., 0., 0.),
            (250_000_000_001., 0., 1e12),
        ],
    );
    Rc::make_mut(&mut line).path.width = PathWidth {
        surface: 1e-6,
        shoulder: 0.,
        material: 0.,
    };
    let composition = geo.with_roads(vec![line]).unwrap();
    let indexed = composition
        .surface_effects()
        .unwrap()
        .sample(p(x, 0.), 0.)
        .unwrap();
    let grid = composition.sample_vertex(GridPoint { x: 1, z: 0 }).unwrap();
    assert_eq!(indexed.height, grid.height);
}
