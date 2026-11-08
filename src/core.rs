// Copyright (c) 2024 Rune Skovbo Johansen
// SPDX-License-Identifier: MPL-2.0
//! Synchronous retained chunk layers and scheduling utilities.
use crate::common::{GridBounds, Point, floor_div};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fmt;
use std::rc::{Rc, Weak};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerError(pub String);
impl fmt::Display for LayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for LayerError {}
fn error(message: &str) -> LayerError {
    LayerError(message.into())
}
#[derive(Debug, Clone, Copy)]
pub struct LayerConfig {
    pub chunk_size: Point,
    pub levels: usize,
    pub grid_width: i32,
    pub grid_height: i32,
    pub max_overlap: usize,
    pub max_users: usize,
}
#[derive(Debug, Clone, Copy)]
pub struct LayerDependency {
    padding: Point,
    level: usize,
}
impl LayerDependency {
    pub fn new(padding: Point, level: usize) -> Result<Self, LayerError> {
        if padding.x < 0 || padding.y < 0 {
            return Err(error("dependency padding cannot be negative"));
        }
        Ok(Self { padding, level })
    }
    pub fn required_bounds(&self, bounds: GridBounds) -> GridBounds {
        GridBounds::new(bounds.min - self.padding, bounds.max + self.padding)
    }
}
pub struct LayerRequest {
    release: Option<Box<dyn FnOnce()>>,
}
impl LayerRequest {
    pub fn new(release: impl FnOnce() + 'static) -> Self {
        Self {
            release: Some(Box::new(release)),
        }
    }
    pub fn release(&mut self) {
        if let Some(release) = self.release.take() {
            release();
        }
    }
    pub fn is_active(&self) -> bool {
        self.release.is_some()
    }
}
impl Drop for LayerRequest {
    fn drop(&mut self) {
        self.release();
    }
}
pub trait DataLayer {
    fn request(&self, bounds: GridBounds, level: usize) -> Result<LayerRequest, LayerError>;
    fn is_ready(&self, bounds: GridBounds, level: usize) -> bool;
    fn shutdown(&self);
}
type Key = (i32, i32);
fn key(point: Point) -> Key {
    (point.x, point.y)
}
type LevelCreate<T> = dyn Fn(Point, usize) -> Result<T, LayerError>;
type LevelDestroy<T> = dyn Fn(Point, usize, &T);
struct RetainedProvider {
    _request: LayerRequest,
    _owner: Option<Rc<dyn DataLayer>>,
}
enum LevelState<T> {
    Vacant,
    Building,
    Ready(Rc<T>),
}
impl<T> LevelState<T> {
    fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }
    fn is_vacant(&self) -> bool {
        matches!(self, Self::Vacant)
    }
    fn as_ref(&self) -> Option<&Rc<T>> {
        if let Self::Ready(value) = self {
            Some(value)
        } else {
            None
        }
    }
    fn take(&mut self) -> Option<Rc<T>> {
        if let Self::Ready(value) = std::mem::replace(self, Self::Vacant) {
            Some(value)
        } else {
            None
        }
    }
}

struct Record<T> {
    index: Point,
    incarnation: u64,
    users: Vec<usize>,
    levels: Vec<LevelState<T>>,
    providers: Vec<Vec<RetainedProvider>>,
    teardown: usize,
    alive: bool,
}
struct Dependency {
    layer: Weak<dyn DataLayer>,
    spec: LayerDependency,
}
struct State<T> {
    records: BTreeMap<Key, Rc<RefCell<Record<T>>>>,
    requests: BTreeMap<u64, Vec<(Key, usize)>>,
    dependencies: Vec<Vec<Dependency>>,
    next_id: u64,
    next_incarnation: u64,
    pool: Vec<Rc<RefCell<Record<T>>>>,
    depth: usize,
    stopping: bool,
    stopped: bool,
    stop_requested: bool,
}
struct Inner<T> {
    config: LayerConfig,
    state: RefCell<State<T>>,
    create: Box<LevelCreate<T>>,
    destroy: Box<LevelDestroy<T>>,
}
pub struct Layer<T: 'static> {
    inner: Rc<Inner<T>>,
}
impl<T> Clone for Layer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}
struct Operation<T: 'static>(Layer<T>);
impl<T> Drop for Operation<T> {
    fn drop(&mut self) {
        let shutdown = {
            let mut state = self.0.inner.state.borrow_mut();
            state.depth -= 1;
            state.depth == 0 && state.stop_requested && !state.stopping && !state.stopped
        };
        if shutdown {
            self.0.perform_shutdown();
        }
    }
}
impl<T: 'static> Layer<T> {
    pub fn new(
        config: LayerConfig,
        create: impl Fn(Point, usize) -> Result<T, LayerError> + 'static,
        destroy: impl Fn(Point, usize, &T) + 'static,
    ) -> Result<Self, LayerError> {
        if config.chunk_size.x <= 0
            || config.chunk_size.y <= 0
            || config.levels == 0
            || config.grid_width <= 0
            || config.grid_height <= 0
            || config.max_overlap == 0
            || config.max_users == 0
        {
            return Err(error("invalid layer configuration"));
        }
        Ok(Self {
            inner: Rc::new(Inner {
                config,
                state: RefCell::new(State {
                    records: BTreeMap::new(),
                    requests: BTreeMap::new(),
                    dependencies: (0..config.levels).map(|_| Vec::new()).collect(),
                    next_id: 0,
                    next_incarnation: 0,
                    pool: Vec::new(),
                    depth: 0,
                    stopping: false,
                    stopped: false,
                    stop_requested: false,
                }),
                create: Box::new(create),
                destroy: Box::new(destroy),
            }),
        })
    }
    fn operation(&self) -> Operation<T> {
        self.inner.state.borrow_mut().depth += 1;
        Operation(self.clone())
    }
    fn validate(&self, level: usize) -> Result<(), LayerError> {
        if level >= self.inner.config.levels {
            Err(error("chunk level is out of range"))
        } else {
            Ok(())
        }
    }
    pub fn world_to_chunk(&self, point: Point) -> Point {
        Point::new(
            floor_div(point.x, self.inner.config.chunk_size.x),
            floor_div(point.y, self.inner.config.chunk_size.y),
        )
    }
    fn indices(&self, bounds: GridBounds) -> impl Iterator<Item = Key> {
        let bounds = if bounds.is_empty() {
            GridBounds::default()
        } else {
            bounds.divided(self.inner.config.chunk_size)
        };
        (bounds.min.x..bounds.max.x)
            .flat_map(move |x| (bounds.min.y..bounds.max.y).map(move |y| (x, y)))
    }
    fn world_bounds(&self, point: Point) -> GridBounds {
        let size = self.inner.config.chunk_size;
        let min = Point::new(point.x.wrapping_mul(size.x), point.y.wrapping_mul(size.y));
        GridBounds::new(min, min + size)
    }
    pub fn add_dependency(
        &self,
        own_level: usize,
        provider: Rc<dyn DataLayer>,
        dependency: LayerDependency,
    ) -> Result<(), LayerError> {
        self.validate(own_level)?;
        self.inner.state.borrow_mut().dependencies[own_level].push(Dependency {
            layer: Rc::downgrade(&provider),
            spec: dependency,
        });
        Ok(())
    }
    pub fn request(&self, bounds: GridBounds, level: usize) -> Result<LayerRequest, LayerError> {
        let _operation = self.operation();
        self.validate(level)?;
        let id = {
            let mut state = self.inner.state.borrow_mut();
            if state.stopped || state.stopping || state.stop_requested {
                return Err(error("layer is shut down"));
            }
            state.next_id = state
                .next_id
                .checked_add(1)
                .ok_or_else(|| error("request id overflow"))?;
            let id = state.next_id;
            state.requests.insert(id, Vec::new());
            id
        };
        for index in self.indices(bounds) {
            if let Err(err) = self.acquire(index, level) {
                self.release_id(id);
                return Err(err);
            }
            self.inner
                .state
                .borrow_mut()
                .requests
                .get_mut(&id)
                .unwrap()
                .push((index, level));
        }
        let weak = Rc::downgrade(&self.inner);
        Ok(LayerRequest::new(move || {
            if let Some(inner) = weak.upgrade() {
                Layer { inner }.release_id(id);
            }
        }))
    }
    fn record(&self, index: Key) -> Result<Rc<RefCell<Record<T>>>, LayerError> {
        let mut state = self.inner.state.borrow_mut();
        if let Some(record) = state.records.get(&index) {
            return Ok(record.clone());
        }
        let config = self.inner.config;
        let overlaps = state
            .records
            .keys()
            .filter(|&&(x, y)| {
                x.rem_euclid(config.grid_width) == index.0.rem_euclid(config.grid_width)
                    && y.rem_euclid(config.grid_height) == index.1.rem_euclid(config.grid_height)
            })
            .count();
        if overlaps >= config.max_overlap {
            return Err(error("rolling grid maximum overlap exceeded"));
        }
        state.next_incarnation = state
            .next_incarnation
            .checked_add(1)
            .ok_or_else(|| error("incarnation overflow"))?;
        let incarnation = state.next_incarnation;
        let record = if let Some(record) = state.pool.pop() {
            {
                let mut value = record.borrow_mut();
                value.index = Point::new(index.0, index.1);
                value.incarnation = incarnation;
                value.users.fill(0);
                value.teardown = 0;
                value.alive = true;
            }
            record
        } else {
            Rc::new(RefCell::new(Record {
                index: Point::new(index.0, index.1),
                incarnation,
                users: vec![0; config.levels],
                levels: (0..config.levels).map(|_| LevelState::Vacant).collect(),
                providers: (0..config.levels).map(|_| Vec::new()).collect(),
                teardown: 0,
                alive: true,
            }))
        };
        state.records.insert(index, record.clone());
        Ok(record)
    }
    fn acquire(&self, index: Key, level: usize) -> Result<(), LayerError> {
        let record = self.record(index)?;
        if record.borrow().users[level] == self.inner.config.max_users {
            self.lower(&record);
            return Err(error("chunk user count overflow"));
        }
        if let Err(err) = self.ensure_level(&record, level) {
            self.lower(&record);
            return Err(err);
        }
        record.borrow_mut().users[level] += 1;
        Ok(())
    }
    fn ensure_level(
        &self,
        record: &Rc<RefCell<Record<T>>>,
        target: usize,
    ) -> Result<(), LayerError> {
        loop {
            let (next, index) = {
                let record = record.borrow();
                (
                    record
                        .levels
                        .iter()
                        .rposition(LevelState::is_ready)
                        .map_or(0, |i| i + 1),
                    record.index,
                )
            };
            if next > target {
                return Ok(());
            }
            {
                let mut record = record.borrow_mut();
                if matches!(record.levels[next], LevelState::Building) {
                    return Err(error("cyclic or reentrant chunk generation"));
                }
                record.levels[next] = LevelState::Building;
            }
            let mut providers = Vec::new();
            let result = (|| {
                if next > 0 {
                    let dependency = LayerDependency::new(Point::new(1, 1), next - 1)?;
                    providers.push(RetainedProvider {
                        _request: self.request(
                            dependency.required_bounds(self.world_bounds(index)),
                            next - 1,
                        )?,
                        _owner: None,
                    });
                }
                let dependencies: Vec<_> = {
                    let state = self.inner.state.borrow();
                    state.dependencies[next]
                        .iter()
                        .map(|dependency| (dependency.layer.clone(), dependency.spec))
                        .collect()
                };
                for (weak, spec) in dependencies {
                    let provider = weak
                        .upgrade()
                        .ok_or_else(|| error("dependency layer expired"))?;
                    providers.push(RetainedProvider {
                        _request: provider
                            .request(spec.required_bounds(self.world_bounds(index)), spec.level)?,
                        _owner: Some(provider),
                    });
                }
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    (self.inner.create)(index, next)
                }))
                .unwrap_or_else(|_| Err(error("chunk creation panicked")))
            })();
            match result {
                Ok(value) => {
                    let mut record = record.borrow_mut();
                    record.levels[next] = LevelState::Ready(Rc::new(value));
                    record.providers[next] = providers;
                }
                Err(err) => {
                    record.borrow_mut().levels[next] = LevelState::Vacant;
                    record.borrow_mut().teardown += 1;
                    drop(providers);
                    record.borrow_mut().teardown -= 1;
                    return Err(err);
                }
            }
        }
    }
    fn release_id(&self, id: u64) {
        let _operation = self.operation();
        let usages = self.inner.state.borrow_mut().requests.remove(&id);
        if let Some(usages) = usages {
            for (index, level) in usages.into_iter().rev() {
                let record = self.inner.state.borrow().records.get(&index).cloned();
                if let Some(record) = record {
                    let lower = {
                        let mut record = record.borrow_mut();
                        if record.users[level] == 0 {
                            false
                        } else {
                            record.users[level] -= 1;
                            record.teardown == 0
                        }
                    };
                    if lower {
                        self.lower(&record);
                    }
                }
            }
        }
    }
    fn lower(&self, record: &Rc<RefCell<Record<T>>>) {
        loop {
            let to_destroy = {
                let mut record = record.borrow_mut();
                if record.teardown > 0
                    || record
                        .levels
                        .iter()
                        .any(|level| matches!(level, LevelState::Building))
                {
                    return;
                }
                let target = record.users.iter().rposition(|&count| count > 0);
                let current = record.levels.iter().rposition(LevelState::is_ready);
                match current {
                    Some(level) if target.is_none_or(|target| level > target) => {
                        let value = record.levels[level].take().unwrap();
                        let providers = std::mem::take(&mut record.providers[level]);
                        record.teardown += 1;
                        Some((record.index, level, value, providers))
                    }
                    _ => None,
                }
            };
            if let Some((index, level, value, providers)) = to_destroy {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    (self.inner.destroy)(index, level, value.as_ref())
                }));
                drop(providers);
                record.borrow_mut().teardown -= 1;
                continue;
            }
            let recycle = {
                let record = record.borrow();
                record.teardown == 0
                    && record.users.iter().all(|&count| count == 0)
                    && record.levels.iter().all(LevelState::is_vacant)
            };
            if recycle {
                let index = key(record.borrow().index);
                let mut state = self.inner.state.borrow_mut();
                if state
                    .records
                    .get(&index)
                    .is_some_and(|existing| Rc::ptr_eq(existing, record))
                {
                    record.borrow_mut().alive = false;
                    state.records.remove(&index);
                    state.pool.push(record.clone());
                }
            }
            return;
        }
    }
    pub fn try_get_chunk(&self, index: Point, minimum_level: usize) -> Option<ChunkHandle<T>> {
        if self.validate(minimum_level).is_err() {
            return None;
        }
        let record = self
            .inner
            .state
            .borrow()
            .records
            .get(&key(index))
            .cloned()?;
        let incarnation = {
            let record = record.borrow();
            record.levels.get(minimum_level)?.as_ref()?;
            record.incarnation
        };
        Some(ChunkHandle {
            record: Rc::downgrade(&record),
            incarnation,
            level: minimum_level,
        })
    }
    pub fn current_level(&self, index: Point) -> Option<usize> {
        self.inner
            .state
            .borrow()
            .records
            .get(&key(index))
            .and_then(|record| {
                record
                    .borrow()
                    .levels
                    .iter()
                    .rposition(LevelState::is_ready)
            })
    }
    pub fn user_count(&self, index: Point, level: usize) -> usize {
        self.inner
            .state
            .borrow()
            .records
            .get(&key(index))
            .and_then(|record| record.borrow().users.get(level).copied())
            .unwrap_or(0)
    }
    pub fn active_chunk_count(&self) -> usize {
        self.inner.state.borrow().records.len()
    }
    pub fn pooled_chunk_count(&self) -> usize {
        self.inner.state.borrow().pool.len()
    }
    pub fn is_ready(&self, bounds: GridBounds, level: usize) -> bool {
        self.validate(level).is_ok()
            && self.indices(bounds).all(|index| {
                self.current_level(Point::new(index.0, index.1))
                    .is_some_and(|current| current >= level)
            })
    }
    pub fn shutdown(&self) {
        let now = {
            let mut state = self.inner.state.borrow_mut();
            if state.stopped || state.stopping {
                return;
            }
            if state.depth > 0 {
                state.stop_requested = true;
                false
            } else {
                true
            }
        };
        if now {
            self.perform_shutdown();
        }
    }
    fn perform_shutdown(&self) {
        {
            let mut state = self.inner.state.borrow_mut();
            if state.stopping || state.stopped {
                return;
            }
            state.stopping = true;
            state.stop_requested = false;
        }
        loop {
            let id = self.inner.state.borrow().requests.keys().next().copied();
            match id {
                Some(id) => self.release_id(id),
                None => break,
            }
        }
        let records: Vec<_> = self
            .inner
            .state
            .borrow()
            .records
            .values()
            .cloned()
            .collect();
        for record in records {
            record.borrow_mut().users.fill(0);
            self.lower(&record);
        }
        let mut state = self.inner.state.borrow_mut();
        state.stopping = false;
        state.stopped = true;
    }
}
impl<T> DataLayer for Layer<T> {
    fn request(&self, bounds: GridBounds, level: usize) -> Result<LayerRequest, LayerError> {
        Layer::request(self, bounds, level)
    }
    fn is_ready(&self, bounds: GridBounds, level: usize) -> bool {
        Layer::is_ready(self, bounds, level)
    }
    fn shutdown(&self) {
        Layer::shutdown(self)
    }
}
impl<T> Drop for Layer<T> {
    fn drop(&mut self) {
        if Rc::strong_count(&self.inner) == 1 {
            self.shutdown();
        }
    }
}
pub struct ChunkHandle<T> {
    record: Weak<RefCell<Record<T>>>,
    incarnation: u64,
    level: usize,
}
impl<T> Clone for ChunkHandle<T> {
    fn clone(&self) -> Self {
        Self {
            record: self.record.clone(),
            incarnation: self.incarnation,
            level: self.level,
        }
    }
}
impl<T> ChunkHandle<T> {
    pub fn with<R>(&self, callback: impl FnOnce(&T) -> R) -> Option<R> {
        let record = self.record.upgrade()?;
        let value = {
            let record = record.borrow();
            if !record.alive || record.incarnation != self.incarnation {
                return None;
            }
            record.levels.get(self.level)?.as_ref()?.clone()
        };
        Some(callback(value.as_ref()))
    }
    pub fn is_valid(&self) -> bool {
        self.with(|_| ()).is_some()
    }
    pub fn incarnation(&self) -> u64 {
        self.incarnation
    }
}
#[derive(Clone, Default)]
pub struct ActionQueue {
    inner: Rc<RefCell<Actions>>,
}
#[derive(Default)]
struct Actions {
    regular: VecDeque<Box<dyn FnOnce()>>,
    fast: VecDeque<Box<dyn FnOnce()>>,
    next: VecDeque<Box<dyn FnOnce()>>,
    processing: bool,
}
impl ActionQueue {
    pub fn enqueue(&self, callback: impl FnOnce() + 'static) {
        self.inner
            .borrow_mut()
            .regular
            .push_back(Box::new(callback));
    }
    pub fn enqueue_fast_track(&self, callback: impl FnOnce() + 'static) {
        self.inner.borrow_mut().fast.push_back(Box::new(callback));
    }
    pub fn enqueue_next_frame(&self, callback: impl FnOnce() + 'static) {
        self.inner.borrow_mut().next.push_back(Box::new(callback));
    }
    pub fn queue_count(&self) -> usize {
        let actions = self.inner.borrow();
        actions.regular.len() + actions.fast.len() + actions.next.len()
    }
    pub fn idle(&self) -> bool {
        self.queue_count() == 0
    }
    pub fn process(&self, budget: usize) -> usize {
        let next_count = {
            let mut actions = self.inner.borrow_mut();
            if actions.processing {
                return 0;
            }
            actions.processing = true;
            actions.next.len()
        };
        struct Guard(Rc<RefCell<Actions>>);
        impl Drop for Guard {
            fn drop(&mut self) {
                self.0.borrow_mut().processing = false;
            }
        }
        let _guard = Guard(self.inner.clone());
        let mut done = 0;
        for _ in 0..next_count {
            let callback = self.inner.borrow_mut().next.pop_front().unwrap();
            callback();
            done += 1;
        }
        for _ in 0..budget {
            let callback = {
                let mut actions = self.inner.borrow_mut();
                actions
                    .fast
                    .pop_front()
                    .or_else(|| actions.regular.pop_front())
            };
            match callback {
                Some(callback) => {
                    callback();
                    done += 1;
                }
                None => break,
            }
        }
        done
    }
}
pub struct RollingGrid<T> {
    width: i32,
    height: i32,
    max_overlap: usize,
    cells: HashMap<Key, Vec<(Key, T)>>,
    size: usize,
}
impl<T> RollingGrid<T> {
    pub fn new(width: i32, height: i32, max_overlap: usize) -> Result<Self, LayerError> {
        if width <= 0 || height <= 0 || max_overlap == 0 {
            return Err(error("invalid rolling grid configuration"));
        }
        Ok(Self {
            width,
            height,
            max_overlap,
            cells: HashMap::new(),
            size: 0,
        })
    }
    fn slot(&self, point: Point) -> Key {
        (
            point.x.rem_euclid(self.width),
            point.y.rem_euclid(self.height),
        )
    }
    pub fn get(&self, point: Point) -> Option<&T> {
        self.cells
            .get(&self.slot(point))?
            .iter()
            .find(|(index, _)| *index == key(point))
            .map(|(_, value)| value)
    }
    pub fn set(&mut self, point: Point, value: T) -> Result<(), LayerError> {
        let slot = self.slot(point);
        let entries = self.cells.entry(slot).or_default();
        if let Some((_, existing)) = entries.iter_mut().find(|(index, _)| *index == key(point)) {
            *existing = value;
            return Ok(());
        }
        if entries.len() == self.max_overlap {
            return Err(error("rolling grid maximum overlap exceeded"));
        }
        entries.push((key(point), value));
        self.size += 1;
        Ok(())
    }
    pub fn erase(&mut self, point: Point) -> bool {
        let slot = self.slot(point);
        if let Some(entries) = self.cells.get_mut(&slot)
            && let Some(index) = entries.iter().position(|(index, _)| *index == key(point))
        {
            entries.swap_remove(index);
            self.size -= 1;
            return true;
        }
        false
    }
    pub fn len(&self) -> usize {
        self.size
    }
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
}
pub trait Poolable: Default {
    fn reset(&mut self);
}
pub struct ObjectPool<T: Poolable> {
    inactive: VecDeque<Box<T>>,
    total: usize,
}
impl<T: Poolable> Default for ObjectPool<T> {
    fn default() -> Self {
        Self {
            inactive: VecDeque::new(),
            total: 0,
        }
    }
}
impl<T: Poolable> ObjectPool<T> {
    pub fn acquire(&mut self) -> Box<T> {
        self.inactive.pop_back().unwrap_or_else(|| {
            self.total += 1;
            Box::default()
        })
    }
    pub fn release(&mut self, mut object: Box<T>) {
        object.reset();
        self.inactive.push_back(object);
    }
    pub fn total_count(&self) -> usize {
        self.total
    }
    pub fn inactive_count(&self) -> usize {
        self.inactive.len()
    }
    pub fn active_count(&self) -> usize {
        self.total - self.inactive.len()
    }
}
#[derive(Default)]
pub struct WorkTracker {
    tracking: bool,
    estimated: HashMap<String, f64>,
    needed: HashMap<String, f64>,
    done: HashMap<String, f64>,
}
impl WorkTracker {
    pub fn start(&mut self) {
        self.estimated.clear();
        self.needed.clear();
        self.done.clear();
        self.tracking = true;
    }
    pub fn stop(&mut self) {
        self.tracking = false;
    }
    pub fn add_estimated(&mut self, key: &str, work: f64) {
        if self.tracking {
            *self.estimated.entry(key.into()).or_default() += work;
        }
    }
    pub fn add_needed(&mut self, key: &str, work: f64) {
        if self.tracking {
            *self.needed.entry(key.into()).or_default() += work;
        }
    }
    pub fn add_done(&mut self, key: &str, work: f64) {
        if self.tracking {
            *self.done.entry(key.into()).or_default() += work;
        }
    }
    pub fn work_is_known(&mut self, key: &str) {
        if self.tracking {
            self.estimated
                .insert(key.into(), *self.needed.get(key).unwrap_or(&0.0));
        }
    }
    pub fn progress(&self) -> f64 {
        let mut needed = 0.0;
        let mut done = 0.0;
        for (key, estimate) in &self.estimated {
            needed += estimate.max(*self.needed.get(key).unwrap_or(&0.0));
            done += self.done.get(key).unwrap_or(&0.0);
        }
        if needed == 0.0 { 0.0 } else { done / needed }
    }
}
