use layerprocgen::common::{GridBounds, Point, RandomHash};
use layerprocgen::core::{Layer, LayerConfig};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

#[test]
fn mixed_retained_requests_release_each_created_level_once() {
    let live = Rc::new(RefCell::new(BTreeSet::new()));
    let creates = live.clone();
    let destroys = live.clone();
    let violations = Rc::new(RefCell::new(Vec::new()));
    let failures = violations.clone();
    let config = LayerConfig {
        chunk_size: Point::new(16, 16),
        levels: 2,
        grid_width: 32,
        grid_height: 32,
        max_overlap: 3,
        max_users: usize::MAX,
    };
    let layer = Layer::new(
        config,
        move |index, level| {
            assert!(
                creates.borrow_mut().insert((index, level)),
                "a live level was generated twice"
            );
            Ok((index, level))
        },
        move |index, level, data| {
            if *data != (index, level) {
                failures.borrow_mut().push("destroyed wrong value");
            }
            if !destroys.borrow_mut().remove(&(index, level)) {
                failures.borrow_mut().push("destroyed a level twice");
            }
        },
    )
    .unwrap();
    let random = RandomHash::new(8675309);
    let mut retained: Vec<_> = (0..12).map(|_| None).collect();
    for step in 0..400 {
        let value = random.hash(step);
        let slot = value as usize % retained.len();
        if value & 3 == 0 {
            retained[slot] = None;
            continue;
        }
        let x = (value % 9) as i32 - 4;
        let y = ((value >> 5) % 9) as i32 - 4;
        let bounds = GridBounds::new(
            Point::new(x * 16, y * 16),
            Point::new((x + 2) * 16, (y + 2) * 16),
        );
        let replacement = layer.request(bounds, ((value >> 10) & 1) as usize).unwrap();
        retained[slot] = Some(replacement);
    }
    drop(retained);
    layer.shutdown();
    assert!(violations.borrow().is_empty(), "{:?}", violations.borrow());
    assert!(
        live.borrow().is_empty(),
        "released demand retained generated data"
    );
    assert!(
        layer
            .request(GridBounds::new(Point::new(0, 0), Point::new(16, 16)), 0)
            .is_err()
    );
}
