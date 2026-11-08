// Copyright (c) 2024 Rune Skovbo Johansen
// SPDX-License-Identifier: MPL-2.0
use layerprocgen::common::{GridBounds, Point};
use layerprocgen::samples::SampleWorld;
fn positions(world: &SampleWorld) -> Vec<Point> {
    let mut p: Vec<_> = world.objects().into_iter().map(|(_, p)| p).collect();
    p.sort();
    p
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut world = SampleWorld::new()?;
    let negative = GridBounds::new(Point::new(-300, -300), Point::new(0, 0));
    world.move_to(negative)?;
    world.process(1000);
    assert_eq!(
        positions(&world),
        vec![
            Point::new(-253, -152),
            Point::new(-244, -68),
            Point::new(-224, -151),
            Point::new(-73, -269)
        ]
    );
    let retained = world.objects();
    world.move_to(negative)?;
    assert_eq!(world.process(1000), 0);
    assert_eq!(world.objects(), retained);
    world.move_to(GridBounds::new(Point::new(0, 0), Point::new(300, 300)))?;
    world.process(1000);
    assert_eq!(
        positions(&world),
        vec![
            Point::new(92, 167),
            Point::new(130, 196),
            Point::new(245, 277),
            Point::new(247, 2)
        ]
    );
    world.shutdown();
    world.process(1000);
    assert!(world.objects().is_empty());
    println!("headless_simple=PASS");
    Ok(())
}
