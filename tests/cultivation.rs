use layerprocgen::cultivation::*;
use layerprocgen::location::{Location, LocationConnections};
use layerprocgen::path::{PathOptions, PathWidth, Point2, TerrainSample};
use std::cell::Cell;
use std::rc::Rc;
fn bounds(x: f64, z: f64, w: f64, h: f64) -> PatchBounds {
    PatchBounds::new(Point2 { x, z }, Point2 { x: x + w, z: z + h }).unwrap()
}
fn sites() -> Vec<Location> {
    [("a", 0.), ("b", 5.), ("c", 10.)]
        .into_iter()
        .map(|(id, x)| Location {
            id: id.into(),
            position: Point2 { x, z: 0. },
        })
        .collect()
}
fn options() -> PathOptions {
    PathOptions {
        corridor: 0.,
        grid_spacing: 1.,
        width: PathWidth {
            surface: 0.2,
            shoulder: 0.1,
            material: 4.,
        },
        ..Default::default()
    }
}
fn flat(_: Point2) -> Option<TerrainSample> {
    Some(TerrainSample {
        height: 2.,
        cost: 0.,
        passable: true,
    })
}
#[test]
fn multiple_roads_share_demand_and_retire_on_last_patch_drop() {
    let sites = sites();
    let graph = LocationConnections::build(&sites, 360.).unwrap();
    let calls = Rc::new(Cell::new(0));
    let counter = calls.clone();
    let mut layer = CultivationLayer::new(&sites, &graph, options(), move |p| {
        counter.set(counter.get() + 1);
        flat(p)
    })
    .unwrap();
    let area = bounds(-3., -3., 16., 6.);
    let all = layer.retain_patch(area).unwrap();
    let roads = layer.roads_overlapping(area).unwrap();
    assert_eq!(roads.len(), 2);
    let previous = calls.get();
    let neighbor = layer.retain_patch(bounds(0., -1., 1., 2.)).unwrap();
    assert_eq!(calls.get(), previous);
    assert!(Rc::ptr_eq(
        &roads[0],
        &layer.roads_overlapping(area).unwrap()[0]
    ));
    drop(all);
    assert_eq!(layer.retained_road_count(), 1);
    drop(neighbor);
    assert_eq!(layer.retained_road_count(), 0);
    assert_eq!(roads[0].path.points[0].height, 2.);
    let reloaded = layer.retain_patch(area).unwrap();
    assert_eq!(*roads[0], *layer.roads_overlapping(area).unwrap()[0]);
    assert!(calls.get() > previous);
    drop(reloaded);
}
#[test]
fn overlap_includes_material_width_and_other_owner_cells() {
    let sites = vec![
        Location {
            id: "a".into(),
            position: Point2 { x: 9., z: 0. },
        },
        Location {
            id: "b".into(),
            position: Point2 { x: 11., z: 0. },
        },
    ];
    let graph = LocationConnections::build(&sites, 10.).unwrap();
    let mut layer = CultivationLayer::new(&sites, &graph, options(), flat).unwrap();
    let patch = layer.retain_patch(bounds(8., 1.5, 1., 0.4)).unwrap();
    let roads = layer.roads_overlapping(bounds(8., 1.5, 1., 0.4)).unwrap();
    assert_eq!(roads.len(), 1);
    assert_eq!(roads[0].connection.owner.x, 1);
    assert!(
        layer
            .roads_overlapping(bounds(8., 2.1, 1., 1.))
            .unwrap()
            .is_empty()
    );
    drop(patch);
    assert_eq!(layer.retained_road_count(), 0);
}
#[test]
fn failed_acquisition_publishes_no_partial_roads_and_preserves_existing() {
    let sites = sites();
    let graph = LocationConnections::build(&sites, 360.).unwrap();
    let mut layer = CultivationLayer::new(&sites, &graph, options(), |p| {
        if p.x > 7. { None } else { flat(p) }
    })
    .unwrap();
    let area = bounds(-3., -3., 16., 6.);
    assert!(matches!(
        layer.retain_patch(area),
        Err(CultivationError::Planning { .. })
    ));
    assert_eq!(layer.retained_road_count(), 0);
    let first = layer.retain_patch(bounds(0., -1., 1., 2.)).unwrap();
    let road = layer.roads_overlapping(area).unwrap()[0].clone();
    assert!(layer.retain_patch(area).is_err());
    assert_eq!(layer.retained_road_count(), 1);
    assert!(Rc::ptr_eq(
        &road,
        &layer.roads_overlapping(area).unwrap()[0]
    ));
    drop(first);
}
#[test]
fn distant_patch_does_no_work_and_lease_can_outlive_layer() {
    let sites = sites();
    let graph = LocationConnections::build(&sites, 360.).unwrap();
    let mut layer = CultivationLayer::new(&sites, &graph, options(), |_| {
        panic!("distant patch sampled terrain")
    })
    .unwrap();
    let patch = layer.retain_patch(bounds(100., 100., 1., 1.)).unwrap();
    assert_eq!(layer.retained_road_count(), 0);
    drop(layer);
    drop(patch);
}
#[test]
fn malformed_graph_bounds_and_options_are_rejected() {
    let sites = sites();
    let mut graph = LocationConnections::build(&sites, 360.).unwrap();
    graph.connections[0].a = "missing".into();
    graph.connections[0].b = "z".into();
    assert!(matches!(
        CultivationLayer::new(&sites, &graph, options(), flat),
        Err(CultivationError::MissingLocation(_))
    ));
    let graph = LocationConnections::build(&sites, 360.).unwrap();
    let mut layer = CultivationLayer::new(&sites, &graph, options(), flat).unwrap();
    assert!(
        layer
            .retain_patch(PatchBounds {
                min: Point2 { x: f64::NAN, z: 0. },
                max: Point2 { x: 1., z: 1. }
            })
            .is_err()
    );
    let mut bad = options();
    bad.corridor = -1.;
    assert!(CultivationLayer::new(&sites, &graph, bad, flat).is_err());
}
