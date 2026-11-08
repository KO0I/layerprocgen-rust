use layerprocgen::common::{GridBounds, Point};
use layerprocgen::core::{Layer, LayerConfig};
use layerprocgen::executor::{Executor, InlineExecutor, ManagedExecutor, ShutdownPolicy};
use layerprocgen::floating_origin::FloatingOrigin;
use std::sync::{Arc, mpsc};
use std::time::Duration;
fn config() -> LayerConfig {
    LayerConfig {
        chunk_size: Point::new(4, 4),
        levels: 1,
        grid_width: 1,
        grid_height: 1,
        max_overlap: 1,
        max_users: usize::MAX,
    }
}
#[test]
fn chunk_read_can_release_its_request() {
    let layer = Layer::new(config(), |_, _| Ok(42), |_, _, _| {}).unwrap();
    let request = layer
        .request(GridBounds::new(Point::new(0, 0), Point::new(4, 4)), 0)
        .unwrap();
    let handle = layer.try_get_chunk(Point::new(0, 0), 0).unwrap();
    assert_eq!(
        handle.with(move |value| {
            drop(request);
            *value
        }),
        Some(42)
    );
    assert_eq!(layer.active_chunk_count(), 0);
    assert!(!handle.is_valid());
}
#[test]
fn origin_callback_capture_can_drop_another_registration() {
    let origin = FloatingOrigin::new();
    let first = origin.register_handler(|_, _| {});
    let second = origin.register_handler(move |_, _| {
        let _ = &first;
    });
    drop(second);
}
#[test]
fn large_request_hits_grid_limit_without_collecting_rectangle() {
    let layer = Layer::new(config(), |_, _| Ok(()), |_, _, _| {}).unwrap();
    assert!(
        layer
            .request(
                GridBounds::new(
                    Point::new(-1_000_000, -1_000_000),
                    Point::new(1_000_000, 1_000_000)
                ),
                0
            )
            .is_err()
    );
    assert_eq!(layer.active_chunk_count(), 0);
    let empty = layer
        .request(GridBounds::new(Point::new(1, 0), Point::new(1, 4)), 0)
        .unwrap();
    assert_eq!(layer.active_chunk_count(), 0);
    drop(empty);
}
#[test]
fn nested_inline_can_shutdown_outer_executor() {
    let a = Arc::new(InlineExecutor::new());
    let owner = a.clone();
    let b = InlineExecutor::new();
    assert!(a.execute(Box::new(move |_| {
        assert!(b.execute(Box::new(move |_| owner.shutdown(ShutdownPolicy::Drain))));
    })));
}
#[test]
fn inline_callback_can_shutdown_outer_managed_executor() {
    let a = Arc::new(ManagedExecutor::new(1).unwrap());
    let owner = a.clone();
    let (done, received) = mpsc::channel();
    assert!(a.execute(Box::new(move |_| {
        let b = InlineExecutor::new();
        assert!(b.execute(Box::new(move |_| owner.shutdown(ShutdownPolicy::Drain))));
        done.send(()).unwrap();
    })));
    received.recv_timeout(Duration::from_secs(3)).unwrap();
    a.shutdown(ShutdownPolicy::Drain);
}
