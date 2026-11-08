// Copyright (c) 2024 Rune Skovbo Johansen
// SPDX-License-Identifier: MPL-2.0
//! Renderer-independent port of the C++ simple layers sample.
use crate::common::{GridBounds, Point, RandomHash};
use crate::core::{
    ActionQueue, DataLayer, Layer, LayerConfig, LayerDependency, LayerError, LayerRequest,
};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
const POINTS_SIZE: Point = Point::new(256, 256);
const SPAWNING_SIZE: Point = Point::new(300, 300);
fn config(chunk_size: Point) -> LayerConfig {
    LayerConfig {
        chunk_size,
        levels: 1,
        grid_width: 64,
        grid_height: 64,
        max_overlap: 3,
        max_users: usize::MAX,
    }
}
fn chunk_bounds(index: Point, size: Point) -> GridBounds {
    let min = Point::new(index.x.wrapping_mul(size.x), index.y.wrapping_mul(size.y));
    GridBounds::new(min, min + size)
}
fn generate_points(index: Point) -> Vec<Point> {
    let bounds = chunk_bounds(index, POINTS_SIZE);
    let random = RandomHash::new(1234);
    let coordinate = |minimum: i32, extent: i32, salt: u32| {
        minimum.wrapping_add(
            (random.hash_values(&[index.x as u32, index.y as u32, salt]) % extent as u32) as i32,
        )
    };
    (0..3)
        .map(|i| {
            Point::new(
                coordinate(bounds.min.x, POINTS_SIZE.x, i * 2),
                coordinate(bounds.min.y, POINTS_SIZE.y, i * 2 + 1),
            )
        })
        .collect()
}
pub struct PointsChunk {
    pub points: Vec<Point>,
}
pub struct PointsLayer {
    layer: Layer<PointsChunk>,
}
impl PointsLayer {
    pub fn new() -> Result<Self, LayerError> {
        Ok(Self {
            layer: Layer::new(
                config(POINTS_SIZE),
                |index, _| {
                    Ok(PointsChunk {
                        points: generate_points(index),
                    })
                },
                |_, _, _| {},
            )?,
        })
    }
    pub fn request(&self, bounds: GridBounds, level: usize) -> Result<LayerRequest, LayerError> {
        self.layer.request(bounds, level)
    }
    pub fn points_in_bounds(&self, bounds: GridBounds) -> Vec<Point> {
        let mut result = Vec::new();
        let indices = bounds.divided(POINTS_SIZE);
        for x in indices.min.x..indices.max.x {
            for y in indices.min.y..indices.max.y {
                if let Some(chunk) = self.layer.try_get_chunk(Point::new(x, y), 0) {
                    chunk.with(|chunk| {
                        result.extend(
                            chunk
                                .points
                                .iter()
                                .copied()
                                .filter(|point| bounds.contains(*point)),
                        )
                    });
                }
            }
        }
        result
    }
    pub fn active_chunk_count(&self) -> usize {
        self.layer.active_chunk_count()
    }
    pub fn shutdown(&self) {
        self.layer.shutdown();
    }
}
pub type ObjectId = u64;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SceneEvent {
    Created(ObjectId, Point),
    Destroyed(ObjectId),
}
#[derive(Default)]
pub struct InMemoryScene {
    next_id: ObjectId,
    objects: BTreeMap<ObjectId, Point>,
    events: Vec<SceneEvent>,
    created: usize,
    destroyed: usize,
}
impl InMemoryScene {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn create_cube(&mut self, position: Point) -> ObjectId {
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("scene object ID exhausted");
        let id = self.next_id;
        self.objects.insert(id, position);
        self.events.push(SceneEvent::Created(id, position));
        self.created += 1;
        id
    }
    pub fn destroy_object(&mut self, id: ObjectId) {
        if self.objects.remove(&id).is_some() {
            self.events.push(SceneEvent::Destroyed(id));
            self.destroyed += 1;
        }
    }
    pub fn objects(&self) -> Vec<(ObjectId, Point)> {
        self.objects
            .iter()
            .map(|(&id, &point)| (id, point))
            .collect()
    }
    pub fn events(&self) -> &[SceneEvent] {
        &self.events
    }
    pub fn take_events(&mut self) -> Vec<SceneEvent> {
        std::mem::take(&mut self.events)
    }
}
struct SpawnGroup {
    objects: Vec<ObjectId>,
}
pub struct PointsSpawningLayer {
    layer: Layer<Rc<RefCell<SpawnGroup>>>,
    _provider: Rc<dyn DataLayer>,
}
impl PointsSpawningLayer {
    pub fn new(
        points: Rc<PointsLayer>,
        scene: Rc<RefCell<InMemoryScene>>,
        queue: ActionQueue,
    ) -> Result<Self, LayerError> {
        let create_points = points.clone();
        let create_scene = scene.clone();
        let create_queue = queue.clone();
        let destroy_scene = scene;
        let destroy_queue = queue;
        let layer = Layer::new(
            config(SPAWNING_SIZE),
            move |index, _| {
                let group = Rc::new(RefCell::new(SpawnGroup {
                    objects: Vec::new(),
                }));
                for point in create_points.points_in_bounds(chunk_bounds(index, SPAWNING_SIZE)) {
                    let group = group.clone();
                    let scene = create_scene.clone();
                    create_queue.enqueue(move || {
                        let id = scene.borrow_mut().create_cube(point);
                        group.borrow_mut().objects.push(id);
                    });
                }
                Ok(group)
            },
            move |_, _, group: &Rc<RefCell<SpawnGroup>>| {
                let group = group.clone();
                let scene = destroy_scene.clone();
                destroy_queue.enqueue(move || {
                    for id in group.borrow_mut().objects.drain(..) {
                        scene.borrow_mut().destroy_object(id);
                    }
                });
            },
        )?;
        let provider: Rc<dyn DataLayer> = Rc::new(points.layer.clone());
        layer.add_dependency(
            0,
            provider.clone(),
            LayerDependency::new(Point::new(0, 0), 0)?,
        )?;
        Ok(Self {
            layer,
            _provider: provider,
        })
    }
    pub fn request(&self, bounds: GridBounds, level: usize) -> Result<LayerRequest, LayerError> {
        self.layer.request(bounds, level)
    }
    pub fn active_chunk_count(&self) -> usize {
        self.layer.active_chunk_count()
    }
    pub fn shutdown(&self) {
        self.layer.shutdown();
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SceneStats {
    pub created: usize,
    pub destroyed: usize,
    pub pending: usize,
}
pub struct SampleWorld {
    points: Rc<PointsLayer>,
    spawning: PointsSpawningLayer,
    scene: Rc<RefCell<InMemoryScene>>,
    queue: ActionQueue,
    request: Option<LayerRequest>,
}
impl SampleWorld {
    pub fn new() -> Result<Self, LayerError> {
        let points = Rc::new(PointsLayer::new()?);
        let scene = Rc::new(RefCell::new(InMemoryScene::new()));
        let queue = ActionQueue::default();
        let spawning = PointsSpawningLayer::new(points.clone(), scene.clone(), queue.clone())?;
        Ok(Self {
            points,
            spawning,
            scene,
            queue,
            request: None,
        })
    }
    pub fn move_to(&mut self, bounds: GridBounds) -> Result<(), LayerError> {
        let request = self.spawning.request(bounds, 0)?;
        self.request = Some(request);
        Ok(())
    }
    pub fn process(&self, budget: usize) -> usize {
        let count = self.queue.process(budget);
        self.scene.borrow_mut().events.clear();
        count
    }
    pub fn objects(&self) -> Vec<(ObjectId, Point)> {
        self.scene.borrow().objects()
    }
    pub fn stats(&self) -> SceneStats {
        let scene = self.scene.borrow();
        SceneStats {
            created: scene.created,
            destroyed: scene.destroyed,
            pending: self.queue.queue_count(),
        }
    }
    pub fn shutdown(&mut self) {
        self.spawning.shutdown();
        self.request.take();
        self.points.shutdown();
    }
}
impl Drop for SampleWorld {
    fn drop(&mut self) {
        self.shutdown();
        self.process(usize::MAX);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn bounds(x: i32, y: i32, mx: i32, my: i32) -> GridBounds {
        GridBounds::new(Point::new(x, y), Point::new(mx, my))
    }
    fn positions(world: &SampleWorld) -> Vec<Point> {
        let mut p: Vec<_> = world.objects().into_iter().map(|(_, p)| p).collect();
        p.sort();
        p
    }
    #[test]
    fn point_chunks_match_cpp_vectors() {
        let layer = PointsLayer::new().unwrap();
        let positive = bounds(0, 0, 256, 256);
        let negative = bounds(-256, -256, 0, 0);
        let _a = layer.request(positive, 0).unwrap();
        let _b = layer.request(negative, 0).unwrap();
        let mut actual = layer.points_in_bounds(positive);
        actual.sort();
        assert_eq!(
            actual,
            vec![
                Point::new(92, 167),
                Point::new(130, 196),
                Point::new(247, 2)
            ]
        );
        let mut actual = layer.points_in_bounds(negative);
        actual.sort();
        assert_eq!(
            actual,
            vec![
                Point::new(-253, -152),
                Point::new(-244, -68),
                Point::new(-224, -151)
            ]
        );
    }
    #[test]
    fn retained_overlap_and_movement() {
        let mut world = SampleWorld::new().unwrap();
        let negative = bounds(-300, -300, 0, 0);
        world.move_to(negative).unwrap();
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
        let before = world.objects();
        world.move_to(negative).unwrap();
        assert_eq!(world.process(1000), 0);
        assert_eq!(world.objects(), before);
        world.move_to(bounds(0, 0, 300, 300)).unwrap();
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
    }
    #[test]
    fn release_before_processing_keeps_fifo_order() {
        let points = Rc::new(PointsLayer::new().unwrap());
        let scene = Rc::new(RefCell::new(InMemoryScene::new()));
        let queue = ActionQueue::default();
        let spawning = PointsSpawningLayer::new(points, scene.clone(), queue.clone()).unwrap();
        let request = spawning.request(bounds(-300, -300, 0, 0), 0).unwrap();
        drop(request);
        assert_eq!(queue.process(1000), 5);
        let scene = scene.borrow();
        let ids: Vec<_> = scene
            .events()
            .iter()
            .map(|e| match e {
                SceneEvent::Created(id, _) => *id as i64,
                SceneEvent::Destroyed(id) => -(*id as i64),
            })
            .collect();
        assert_eq!(ids, vec![1, 2, 3, 4, -1, -2, -3, -4]);
        assert!(scene.objects().is_empty());
    }
}
