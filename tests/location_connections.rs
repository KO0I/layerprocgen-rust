use layerprocgen::location::{Location, LocationConnections};
use layerprocgen::path::{PathOptions, Point2, TerrainSample, plan_path};

#[test]
fn existing_location_ids_connect_to_the_terrain_planner() {
    let sites = [
        Location {
            id: "0:21:21:0:0".into(),
            position: Point2 { x: 1., z: 1. },
        },
        Location {
            id: "0:18:22:0:4".into(),
            position: Point2 { x: 6., z: 1. },
        },
        Location {
            id: "third-existing-site".into(),
            position: Point2 { x: 6., z: 7. },
        },
    ];
    let graph = LocationConnections::build(&sites, 360.).unwrap();
    assert_eq!(graph.connections.len(), 2);
    for edge in &graph.connections {
        let start = sites
            .iter()
            .find(|site| site.id == edge.a)
            .unwrap()
            .position;
        let goal = sites
            .iter()
            .find(|site| site.id == edge.b)
            .unwrap()
            .position;
        let route = plan_path(
            |_| {
                Some(TerrainSample {
                    height: 3.,
                    cost: 0.,
                    passable: true,
                })
            },
            start,
            goal,
            PathOptions::default(),
        )
        .unwrap();
        assert_eq!(route.points.first().unwrap().position, start);
        assert_eq!(route.points.last().unwrap().position, goal);
    }
}
