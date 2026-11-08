use layerprocgen::common::{GridBounds, Point};
use layerprocgen::core::{
    ActionQueue, Layer, LayerConfig, LayerDependency, LayerError, LayerRequest, ObjectPool,
    Poolable, RollingGrid, WorkTracker,
};
use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::{Rc, Weak};
fn b(x: i32, y: i32, mx: i32, my: i32) -> GridBounds {
    GridBounds::new(Point::new(x, y), Point::new(mx, my))
}
fn config(levels: usize) -> LayerConfig {
    LayerConfig {
        chunk_size: Point::new(4, 4),
        levels,
        grid_width: 8,
        grid_height: 8,
        max_overlap: 3,
        max_users: usize::MAX,
    }
}
fn unit(c: LayerConfig) -> Layer<()> {
    Layer::new(c, |_, _| Ok(()), |_, _, _| {}).unwrap()
}
#[test]
fn repeated_recursive_requests_have_distinct_ids() {
    let layer = unit(config(2));
    let mut a = layer.request(b(0, 0, 4, 4), 1).unwrap();
    let mut c = layer.request(b(0, 0, 4, 4), 1).unwrap();
    assert_eq!(layer.user_count(Point::new(0, 0), 1), 2);
    for x in -1..=1 {
        for y in -1..=1 {
            assert_eq!(layer.user_count(Point::new(x, y), 0), 1);
        }
    }
    a.release();
    assert_eq!(layer.user_count(Point::new(0, 0), 1), 1);
    c.release();
    assert_eq!(layer.active_chunk_count(), 0);
}
#[test]
fn overlapping_requests_release_independently() {
    let layer = unit(config(1));
    let mut a = layer.request(b(0, 0, 8, 4), 0).unwrap();
    let c = layer.request(b(4, 0, 12, 4), 0).unwrap();
    assert_eq!(layer.user_count(Point::new(1, 0), 0), 2);
    a.release();
    a.release();
    assert_eq!(layer.current_level(Point::new(0, 0)), None);
    assert_eq!(layer.user_count(Point::new(1, 0), 0), 1);
    drop(c);
    assert_eq!(layer.active_chunk_count(), 0);
}
#[test]
fn expired_dependency_rolls_back_prior_provider() {
    let provider = Rc::new(unit(config(1)));
    let expired = Rc::new(unit(config(1)));
    let consumer = unit(config(1));
    let d = LayerDependency::new(Point::new(0, 0), 0).unwrap();
    consumer.add_dependency(0, provider.clone(), d).unwrap();
    consumer.add_dependency(0, expired.clone(), d).unwrap();
    drop(expired);
    assert!(consumer.request(b(0, 0, 4, 4), 0).is_err());
    assert_eq!(consumer.active_chunk_count(), 0);
    assert_eq!(provider.active_chunk_count(), 0);
}
#[test]
fn failed_create_rolls_back_partial_bounds() {
    let provider = Rc::new(unit(config(1)));
    let destroyed = Rc::new(RefCell::new(Vec::new()));
    let events = destroyed.clone();
    let consumer = Layer::new(
        config(1),
        |p, _| {
            if p.x == 1 {
                Err(LayerError("injected failure".into()))
            } else {
                Ok(p)
            }
        },
        move |p, _, value| {
            assert_eq!(p, *value);
            events.borrow_mut().push(p);
        },
    )
    .unwrap();
    consumer
        .add_dependency(
            0,
            provider.clone(),
            LayerDependency::new(Point::new(0, 0), 0).unwrap(),
        )
        .unwrap();
    assert!(consumer.request(b(0, 0, 8, 4), 0).is_err());
    assert_eq!(*destroyed.borrow(), vec![Point::new(0, 0)]);
    assert_eq!(consumer.active_chunk_count(), 0);
    assert_eq!(provider.active_chunk_count(), 0);
}
#[test]
fn acquired_provider_stays_alive_through_destroy() {
    let provider = Rc::new(unit(config(1)));
    let weak = Rc::downgrade(&provider);
    let callback_provider = weak.clone();
    let observed = Rc::new(Cell::new(false));
    let flag = observed.clone();
    let consumer = Layer::new(
        config(1),
        |_, _| Ok(()),
        move |_, _, _| {
            let p = callback_provider
                .upgrade()
                .expect("provider must remain alive");
            assert_eq!(p.user_count(Point::new(0, 0), 0), 1);
            flag.set(true);
        },
    )
    .unwrap();
    consumer
        .add_dependency(
            0,
            provider.clone(),
            LayerDependency::new(Point::new(0, 0), 0).unwrap(),
        )
        .unwrap();
    let request = consumer.request(b(0, 0, 4, 4), 0).unwrap();
    drop(provider);
    assert!(weak.upgrade().is_some());
    drop(request);
    assert!(observed.get());
    assert!(weak.upgrade().is_none());
}
#[test]
fn stale_handle_cannot_observe_reused_record() {
    let n = Rc::new(Cell::new(0));
    let count = n.clone();
    let layer = Layer::new(
        config(1),
        move |_, _| {
            count.set(count.get() + 1);
            Ok(count.get())
        },
        |_, _, _| {},
    )
    .unwrap();
    let a = layer.request(b(0, 0, 4, 4), 0).unwrap();
    let old = layer.try_get_chunk(Point::new(0, 0), 0).unwrap();
    assert_eq!(old.with(|v| *v), Some(1));
    drop(a);
    assert!(!old.is_valid());
    assert_eq!(layer.pooled_chunk_count(), 1);
    let c = layer.request(b(0, 0, 4, 4), 0).unwrap();
    assert_eq!(layer.pooled_chunk_count(), 0);
    assert_eq!(
        layer
            .try_get_chunk(Point::new(0, 0), 0)
            .unwrap()
            .with(|v| *v),
        Some(2)
    );
    assert_eq!(old.with(|v| *v), None);
    drop(c);
}
#[test]
fn invalid_config_and_levels_fail() {
    let mut c = config(1);
    c.chunk_size.x = 0;
    assert!(Layer::new(c, |_, _| Ok(()), |_, _, _| {}).is_err());
    c = config(1);
    c.levels = 0;
    assert!(Layer::new(c, |_, _| Ok(()), |_, _, _| {}).is_err());
    assert!(LayerDependency::new(Point::new(-1, 0), 0).is_err());
    let layer = unit(config(1));
    assert!(layer.request(b(0, 0, 4, 4), 1).is_err());
    assert_eq!(layer.active_chunk_count(), 0);
}
#[test]
fn user_cap_preserves_existing_demand() {
    let mut c = config(1);
    c.max_users = 1;
    let layer = unit(c);
    let request = layer.request(b(0, 0, 4, 4), 0).unwrap();
    assert!(layer.request(b(0, 0, 4, 4), 0).is_err());
    assert_eq!(layer.user_count(Point::new(0, 0), 0), 1);
    drop(request);
    assert_eq!(layer.active_chunk_count(), 0);
}
#[test]
fn grid_cap_failure_rolls_back_new_chunks() {
    let mut c = config(1);
    c.grid_width = 1;
    c.grid_height = 1;
    c.max_overlap = 1;
    let layer = unit(c);
    assert!(layer.request(b(0, 0, 8, 4), 0).is_err());
    assert_eq!(layer.active_chunk_count(), 0);
}
#[test]
fn callbacks_observe_unpublished_levels_and_shutdown_can_reenter() {
    let holder: Rc<RefCell<Option<Weak<Layer<()>>>>> = Rc::new(RefCell::new(None));
    let a = holder.clone();
    let c = holder.clone();
    let observations = Rc::new(RefCell::new(Vec::new()));
    let first = observations.clone();
    let second = observations.clone();
    let layer = Rc::new(
        Layer::new(
            config(1),
            move |p, _| {
                first.borrow_mut().push(
                    a.borrow()
                        .as_ref()
                        .unwrap()
                        .upgrade()
                        .unwrap()
                        .current_level(p),
                );
                Ok(())
            },
            move |p, _, _| {
                let layer = c.borrow().as_ref().unwrap().upgrade().unwrap();
                second.borrow_mut().push(layer.current_level(p));
                layer.shutdown();
            },
        )
        .unwrap(),
    );
    *holder.borrow_mut() = Some(Rc::downgrade(&layer));
    let request = layer.request(b(0, 0, 4, 4), 0).unwrap();
    drop(request);
    assert_eq!(*observations.borrow(), vec![None, None]);
    assert_eq!(layer.active_chunk_count(), 0);
    assert!(layer.request(b(0, 0, 4, 4), 0).is_err());
}
#[test]
fn destroy_can_retain_a_recreated_chunk() {
    let holder: Rc<RefCell<Option<Weak<Layer<()>>>>> = Rc::new(RefCell::new(None));
    let callback = holder.clone();
    let nested: Rc<RefCell<Option<LayerRequest>>> = Rc::new(RefCell::new(None));
    let next = nested.clone();
    let first = Rc::new(Cell::new(true));
    let flag = first.clone();
    let layer = Rc::new(
        Layer::new(
            config(1),
            |_, _| Ok(()),
            move |_, _, _| {
                if flag.replace(false) {
                    let layer = callback.borrow().as_ref().unwrap().upgrade().unwrap();
                    *next.borrow_mut() = Some(layer.request(b(0, 0, 4, 4), 0).unwrap());
                }
            },
        )
        .unwrap(),
    );
    *holder.borrow_mut() = Some(Rc::downgrade(&layer));
    drop(layer.request(b(0, 0, 4, 4), 0).unwrap());
    assert_eq!(layer.user_count(Point::new(0, 0), 0), 1);
    drop(nested.borrow_mut().take());
    assert_eq!(layer.active_chunk_count(), 0);
}
#[test]
fn next_frame_snapshot_precedes_budgeted_fast_and_regular_actions() {
    let queue = ActionQueue::default();
    let events = Rc::new(RefCell::new(Vec::new()));
    let out = events.clone();
    queue.enqueue(move || out.borrow_mut().push(4));
    let out = events.clone();
    queue.enqueue_fast_track(move || out.borrow_mut().push(3));
    let out = events.clone();
    let next = queue.clone();
    queue.enqueue_next_frame(move || {
        out.borrow_mut().push(1);
        let out = out.clone();
        next.enqueue_next_frame(move || out.borrow_mut().push(2));
    });
    assert_eq!(queue.process(1), 2);
    assert_eq!(*events.borrow(), vec![1, 3]);
    assert_eq!(queue.queue_count(), 2);
    assert_eq!(queue.process(1), 2);
    assert_eq!(*events.borrow(), vec![1, 3, 2, 4]);
    assert!(queue.idle());
}
#[test]
fn action_queue_reentry_and_panic_recovery() {
    let q = ActionQueue::default();
    let answer = Rc::new(Cell::new(9));
    let out = answer.clone();
    let nested = q.clone();
    q.enqueue(move || out.set(nested.process(10)));
    assert_eq!(q.process(1), 1);
    assert_eq!(answer.get(), 0);
    q.enqueue(|| panic!("injected failure"));
    let ran = Rc::new(Cell::new(false));
    let flag = ran.clone();
    q.enqueue(move || flag.set(true));
    assert!(catch_unwind(AssertUnwindSafe(|| q.process(2))).is_err());
    assert_eq!(q.process(1), 1);
    assert!(ran.get());
}
#[test]
fn rolling_grid_negative_addresses_and_collision_limit() {
    let mut grid = RollingGrid::new(2, 2, 2).unwrap();
    grid.set(Point::new(-1, -1), 7).unwrap();
    grid.set(Point::new(1, 1), 9).unwrap();
    grid.set(Point::new(-1, -1), 8).unwrap();
    assert_eq!(grid.get(Point::new(-1, -1)), Some(&8));
    assert_eq!(grid.len(), 2);
    assert!(grid.set(Point::new(3, 3), 11).is_err());
    assert!(grid.erase(Point::new(-1, -1)));
    grid.set(Point::new(3, 3), 11).unwrap();
    assert_eq!(grid.get(Point::new(3, 3)), Some(&11));
}
#[test]
fn work_tracker_uses_known_work_and_resets() {
    let mut t = WorkTracker::default();
    t.start();
    assert_eq!(t.progress(), 0.0);
    t.add_estimated("chunk", 10.0);
    t.add_needed("chunk", 5.0);
    t.add_done("chunk", 2.0);
    assert_eq!(t.progress(), 0.2);
    t.work_is_known("chunk");
    assert_eq!(t.progress(), 0.4);
    t.stop();
    t.add_done("chunk", 3.0);
    assert_eq!(t.progress(), 0.4);
    t.start();
    assert_eq!(t.progress(), 0.0);
}
#[derive(Default)]
struct Pooled {
    value: i32,
    resets: usize,
}
impl Poolable for Pooled {
    fn reset(&mut self) {
        self.value = 0;
        self.resets += 1;
    }
}
#[test]
fn object_pool_resets_and_reuses_allocation() {
    let mut pool = ObjectPool::<Pooled>::default();
    let mut first = pool.acquire();
    first.value = 42;
    let address = std::ptr::from_ref(first.as_ref());
    pool.release(first);
    let second = pool.acquire();
    assert_eq!(std::ptr::from_ref(second.as_ref()), address);
    assert_eq!(second.value, 0);
    assert_eq!(second.resets, 1);
    assert_eq!(pool.total_count(), 1);
    assert_eq!(pool.active_count(), 1);
}

#[test]
fn external_dependency_cycle_is_rejected_and_rolled_back() {
    let a = Rc::new(unit(config(1)));
    let c = Rc::new(unit(config(1)));
    let d = LayerDependency::new(Point::new(0, 0), 0).unwrap();
    a.add_dependency(0, c.clone(), d).unwrap();
    c.add_dependency(0, a.clone(), d).unwrap();
    assert!(a.request(b(0, 0, 4, 4), 0).is_err());
    assert_eq!(a.active_chunk_count(), 0);
    assert_eq!(c.active_chunk_count(), 0);
}
#[test]
fn create_panic_releases_partial_demand() {
    let layer = Layer::new(
        config(1),
        |p, _| {
            if p.x == 1 {
                panic!("injected create panic");
            }
            Ok(())
        },
        |_, _, _| {},
    )
    .unwrap();
    assert!(layer.request(b(0, 0, 8, 4), 0).is_err());
    assert_eq!(layer.active_chunk_count(), 0);
}
