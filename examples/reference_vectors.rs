use layerprocgen::common::{Fixed8, GridBounds, Point, Point3, RandomHash};
use layerprocgen::samples::SampleWorld;
use layerprocgen::save_state::SaveState;

fn main() {
    for seed in [0, 1, 1234, u32::MAX] {
        for value in [0, 1, 9, (-9i32) as u32, u32::MAX] {
            println!("hash {seed} {value} {}", RandomHash::new(seed).hash(value));
        }
    }
    for x in -4i32..=4 {
        for y in -4i32..=4 {
            for salt in 0..6 {
                println!(
                    "multi {x} {y} {salt} {}",
                    RandomHash::new(1234).hash_values(&[x as u32, y as u32, salt])
                );
            }
        }
    }
    for number in [-12.75, -1.1, -0.5, -0.001, 0.0, 0.1, 0.5, 1.1, 100.999] {
        let value = Fixed8::from_f64(number).unwrap();
        println!(
            "fixed {} {} {} {}",
            value.raw(),
            value.floor_to_int(),
            value.ceil_to_int(),
            value.round_to_int()
        );
    }
    let bounds = GridBounds::new(Point::new(-1024, -1024), Point::new(1280, 1280));
    let mut world = SampleWorld::new().unwrap();
    world.move_to(bounds).unwrap();
    world.process(usize::MAX);
    let mut points: Vec<_> = world
        .objects()
        .into_iter()
        .map(|(_, p)| p)
        .filter(|&p| bounds.contains(p))
        .collect();
    points.sort();
    for point in points {
        println!("point {} {}", point.x, point.y);
    }
    world.shutdown();
    world.process(usize::MAX);
    let mut save = SaveState::default();
    save.set_bool_values(-9, vec![true, false, true]);
    save.set_bool_values(12, vec![]);
    save.set_int_values(-1, vec![i32::MIN, 0, i32::MAX]);
    save.set_point_values(7, vec![Point::new(-300, 256), Point::new(92, 167)]);
    save.set_point3_values(0, vec![Point3::new(-1, 2, -3)]);
    print!("save ");
    for byte in save.to_bytes().unwrap() {
        print!("{byte:02x}");
    }
    println!();
}
