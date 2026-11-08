// Copyright (c) 2024 Rune Skovbo Johansen
// SPDX-License-Identifier: MPL-2.0
use crate::common::{Point, Point3};
use std::cell::{Cell, Ref, RefCell};
use std::collections::BTreeMap;
use std::fmt;
use std::rc::{Rc, Weak};
const MAGIC: &[u8; 8] = b"LPCGSAVE";
const MAX_FILE_SIZE: usize = 64 * 1024 * 1024;
const MAX_PAYLOAD_SIZE: usize = MAX_FILE_SIZE - 16;
const MAX_ENTRIES: usize = 1_000_000;
const MAX_VALUES: usize = 16_000_000;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    InvalidMagic,
    UnsupportedVersion,
    MalformedData,
    TruncatedData,
    SizeLimitExceeded,
    IoError,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
}
impl Error {
    fn new(code: ErrorCode, message: &str) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
fn malformed(message: &str) -> Error {
    Error::new(ErrorCode::MalformedData, message)
}
fn limit(message: &str) -> Error {
    Error::new(ErrorCode::SizeLimitExceeded, message)
}
#[cfg(not(target_arch = "wasm32"))]
fn io_error(message: &str) -> Error {
    Error::new(ErrorCode::IoError, message)
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SaveState {
    bools: BTreeMap<i32, Vec<bool>>,
    ints: BTreeMap<i32, Vec<i32>>,
    points: BTreeMap<i32, Vec<Point>>,
    points3: BTreeMap<i32, Vec<Point3>>,
}
impl SaveState {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set_bool_values(&mut self, key: i32, values: Vec<bool>) {
        self.bools.insert(key, values);
    }
    pub fn set_int_values(&mut self, key: i32, values: Vec<i32>) {
        self.ints.insert(key, values);
    }
    pub fn set_point_values(&mut self, key: i32, values: Vec<Point>) {
        self.points.insert(key, values);
    }
    pub fn set_point3_values(&mut self, key: i32, values: Vec<Point3>) {
        self.points3.insert(key, values);
    }
    pub fn bool_values(&self, key: i32) -> Option<&[bool]> {
        self.bools.get(&key).map(Vec::as_slice)
    }
    pub fn int_values(&self, key: i32) -> Option<&[i32]> {
        self.ints.get(&key).map(Vec::as_slice)
    }
    pub fn point_values(&self, key: i32) -> Option<&[Point]> {
        self.points.get(&key).map(Vec::as_slice)
    }
    pub fn point3_values(&self, key: i32) -> Option<&[Point3]> {
        self.points3.get(&key).map(Vec::as_slice)
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut payload = Vec::new();
        append_section(&mut payload, 1, &self.bools, 1, |out, v| {
            out.push(u8::from(*v))
        })?;
        append_section(&mut payload, 2, &self.ints, 4, |out, v| {
            out.extend(v.to_le_bytes())
        })?;
        append_section(&mut payload, 3, &self.points, 8, |out, v| {
            out.extend(v.x.to_le_bytes());
            out.extend(v.y.to_le_bytes());
        })?;
        append_section(&mut payload, 4, &self.points3, 12, |out, v| {
            out.extend(v.x.to_le_bytes());
            out.extend(v.y.to_le_bytes());
            out.extend(v.z.to_le_bytes());
        })?;
        let mut bytes = Vec::with_capacity(16 + payload.len());
        bytes.extend(MAGIC);
        bytes.extend(1u16.to_le_bytes());
        bytes.extend(16u16.to_le_bytes());
        bytes.extend((payload.len() as u32).to_le_bytes());
        bytes.extend(payload);
        Ok(bytes)
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_FILE_SIZE {
            return Err(limit("save-state file exceeds size limit"));
        }
        let mut reader = Reader { bytes, pos: 0 };
        if reader.take(8)? != MAGIC {
            return Err(Error::new(
                ErrorCode::InvalidMagic,
                "save-state magic does not match",
            ));
        }
        let version = reader.u16()?;
        let header = reader.u16()?;
        let payload = reader.u32()? as usize;
        if version != 1 {
            return Err(Error::new(
                ErrorCode::UnsupportedVersion,
                "unsupported save-state version",
            ));
        }
        if header != 16 {
            return Err(malformed("invalid save-state header size"));
        }
        if payload != bytes.len() - 16 {
            return Err(malformed("declared payload size does not match input size"));
        }
        let bools = read_section(&mut reader, 1, 1, |r| match r.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(malformed("bool value must be zero or one")),
        })?;
        let ints = read_section(&mut reader, 2, 4, |r| r.i32())?;
        let points = read_section(&mut reader, 3, 8, |r| {
            Ok(Point {
                x: r.i32()?,
                y: r.i32()?,
            })
        })?;
        let points3 = read_section(&mut reader, 4, 12, |r| {
            Ok(Point3 {
                x: r.i32()?,
                y: r.i32()?,
                z: r.i32()?,
            })
        })?;
        if reader.pos != bytes.len() {
            return Err(malformed("save-state payload has trailing data"));
        }
        Ok(Self {
            bools,
            ints,
            points,
            points3,
        })
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save_file(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        use std::io::Write;
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        struct Temporary(std::path::PathBuf);
        impl Drop for Temporary {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let bytes = self.to_bytes()?;
        let path = path.as_ref();
        let mut created = None;
        for _ in 0..128 {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let mut name = path.as_os_str().to_os_string();
            name.push(format!(
                ".tmp.{:x}.{:x}.{:x}",
                std::process::id(),
                timestamp,
                id
            ));
            let temp = std::path::PathBuf::from(name);
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&temp) {
                Ok(file) => {
                    created = Some((Temporary(temp), file));
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err(io_error("could not create temporary save-state file")),
            }
        }
        let (temporary, mut file) = created
            .ok_or_else(|| io_error("could not create a unique temporary save-state file"))?;
        file.write_all(&bytes)
            .map_err(|_| io_error("could not write temporary save-state file"))?;
        file.flush()
            .map_err(|_| io_error("could not write temporary save-state file"))?;
        file.sync_all()
            .map_err(|_| io_error("could not sync temporary save-state file"))?;
        drop(file);
        replace_file(&temporary.0, path)
            .map_err(|_| io_error("could not atomically replace save-state file"))?;
        Ok(())
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_file(path: impl AsRef<std::path::Path>) -> Result<Self> {
        use std::io::Read;
        let mut file =
            std::fs::File::open(path).map_err(|_| io_error("could not open save-state file"))?;
        let size = file
            .metadata()
            .map_err(|_| io_error("could not determine save-state file size"))?
            .len();
        if size > MAX_FILE_SIZE as u64 {
            return Err(limit("save-state file exceeds size limit"));
        }
        let mut bytes = Vec::with_capacity(size as usize);
        file.by_ref()
            .take(MAX_FILE_SIZE as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| io_error("could not read save-state file"))?;
        Self::from_bytes(&bytes)
    }
}
#[cfg(all(not(target_arch = "wasm32"), not(windows)))]
fn replace_file(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    std::fs::rename(from, to)
}
#[cfg(windows)]
fn replace_file(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
    }
    let from: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0x1 | 0x8) } == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}
fn append_section<T>(
    out: &mut Vec<u8>,
    tag: u8,
    entries: &BTreeMap<i32, Vec<T>>,
    width: usize,
    write: impl Fn(&mut Vec<u8>, &T),
) -> Result<()> {
    if entries.len() > MAX_ENTRIES {
        return Err(limit("too many keyed entries"));
    }
    if out.len() > MAX_PAYLOAD_SIZE - 8 {
        return Err(limit("encoded save state exceeds size limit"));
    }
    out.extend([tag, 0, 0, 0]);
    out.extend((entries.len() as u32).to_le_bytes());
    for (key, values) in entries {
        if values.len() > MAX_VALUES {
            return Err(limit("too many values in keyed entry"));
        }
        if out.len() > MAX_PAYLOAD_SIZE - 8
            || values.len() > (MAX_PAYLOAD_SIZE - out.len() - 8) / width
        {
            return Err(limit("encoded save state exceeds size limit"));
        }
        out.extend(key.to_le_bytes());
        out.extend((values.len() as u32).to_le_bytes());
        for value in values {
            write(out, value);
        }
    }
    Ok(())
}
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if n > self.bytes.len() - self.pos {
            return Err(Error::new(
                ErrorCode::TruncatedData,
                "save-state data is truncated",
            ));
        }
        let result = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        Ok(result)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
}
fn read_section<T>(
    r: &mut Reader<'_>,
    tag: u8,
    width: usize,
    mut read: impl FnMut(&mut Reader<'_>) -> Result<T>,
) -> Result<BTreeMap<i32, Vec<T>>> {
    let section_header = r.take(4)?;
    let count = r.u32()? as usize;
    if section_header != [tag, 0, 0, 0] {
        return Err(malformed("invalid save-state section header"));
    }
    if count > MAX_ENTRIES {
        return Err(limit("too many keyed entries"));
    }
    let mut entries = BTreeMap::new();
    for _ in 0..count {
        let key = r.i32()?;
        let count = r.u32()? as usize;
        if count > MAX_VALUES {
            return Err(limit("too many values in keyed entry"));
        }
        if count > (r.bytes.len() - r.pos) / width {
            return Err(Error::new(
                ErrorCode::TruncatedData,
                "save-state value array is truncated",
            ));
        }
        if entries.contains_key(&key) {
            return Err(malformed("duplicate key in save-state section"));
        }
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(read(r)?);
        }
        entries.insert(key, values);
    }
    Ok(entries)
}
struct ValueData<T> {
    value: T,
    default: T,
}
/// Clones share the same live value. Bindings keep weak references to it.
#[derive(Clone)]
pub struct StateValue<T>(Rc<RefCell<ValueData<T>>>);
impl<T: Clone> StateValue<T> {
    pub fn new(default: T) -> Self {
        Self(Rc::new(RefCell::new(ValueData {
            value: default.clone(),
            default,
        })))
    }
    pub fn value(&self) -> T {
        self.0.borrow().value.clone()
    }
    pub fn set(&self, value: T) {
        self.0.borrow_mut().value = value;
    }
    pub fn default_value(&self) -> T {
        self.0.borrow().default.clone()
    }
    pub fn restore_default(&self) {
        let mut value = self.0.borrow_mut();
        value.value = value.default.clone();
    }
}
impl<T: Clone + Default> Default for StateValue<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}
#[derive(Clone, Default)]
pub struct StateBindings {
    bools: Vec<Weak<RefCell<ValueData<bool>>>>,
    ints: Vec<Weak<RefCell<ValueData<i32>>>>,
    points: Vec<Weak<RefCell<ValueData<Point>>>>,
    points3: Vec<Weak<RefCell<ValueData<Point3>>>>,
}
impl StateBindings {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn bind_bool(&mut self, value: &StateValue<bool>) {
        self.bools.push(Rc::downgrade(&value.0));
    }
    pub fn bind_int(&mut self, value: &StateValue<i32>) {
        self.ints.push(Rc::downgrade(&value.0));
    }
    pub fn bind_point(&mut self, value: &StateValue<Point>) {
        self.points.push(Rc::downgrade(&value.0));
    }
    pub fn bind_point3(&mut self, value: &StateValue<Point3>) {
        self.points3.push(Rc::downgrade(&value.0));
    }
    fn alive(&self) -> bool {
        self.bools.iter().all(|v| v.strong_count() > 0)
            && self.ints.iter().all(|v| v.strong_count() > 0)
            && self.points.iter().all(|v| v.strong_count() > 0)
            && self.points3.iter().all(|v| v.strong_count() > 0)
    }
}
fn restore_values<T: Clone>(stored: Option<&[T]>, bindings: &[Weak<RefCell<ValueData<T>>>]) {
    for (index, binding) in bindings.iter().enumerate() {
        if let Some(binding) = binding.upgrade() {
            let mut data = binding.borrow_mut();
            if let Some(values) = stored {
                if let Some(value) = values.get(index) {
                    data.value = value.clone();
                }
            } else {
                data.value = data.default.clone();
            }
        }
    }
}
fn collect_values<T: Clone>(bindings: &[Weak<RefCell<ValueData<T>>>]) -> Vec<T> {
    bindings
        .iter()
        .filter_map(|v| v.upgrade().map(|v| v.borrow().value.clone()))
        .collect()
}
fn restore_bindings(state: &SaveState, key: i32, bindings: &StateBindings) {
    if !bindings.alive() {
        return;
    }
    restore_values(state.bool_values(key), &bindings.bools);
    restore_values(state.int_values(key), &bindings.ints);
    restore_values(state.point_values(key), &bindings.points);
    restore_values(state.point3_values(key), &bindings.points3);
}
fn store_bindings(state: &mut SaveState, key: i32, bindings: &StateBindings) {
    if !bindings.alive() {
        return;
    }
    if !bindings.bools.is_empty() {
        state.set_bool_values(key, collect_values(&bindings.bools));
    }
    if !bindings.ints.is_empty() {
        state.set_int_values(key, collect_values(&bindings.ints));
    }
    if !bindings.points.is_empty() {
        state.set_point_values(key, collect_values(&bindings.points));
    }
    if !bindings.points3.is_empty() {
        state.set_point3_values(key, collect_values(&bindings.points3));
    }
}
pub type Callback = Rc<dyn Fn()>;
struct CallbackEntry {
    active: Cell<bool>,
    save: RefCell<Option<Callback>>,
    load: RefCell<Option<Callback>>,
}
struct LiveControl {
    stored: RefCell<SaveState>,
    entries: RefCell<Vec<Rc<CallbackEntry>>>,
    ready: Cell<bool>,
}
pub struct CallbackToken {
    control: Weak<LiveControl>,
    entry: Weak<CallbackEntry>,
}
impl CallbackToken {
    pub fn reset(&mut self) {
        if let Some(entry) = self.entry.upgrade() {
            entry.active.set(false);
            let save = entry.save.borrow_mut().take();
            let load = entry.load.borrow_mut().take();
            drop((save, load));
        }
        if let Some(control) = self.control.upgrade() {
            control
                .entries
                .borrow_mut()
                .retain(|entry| entry.active.get());
        }
        self.entry = Weak::new();
        self.control = Weak::new();
    }
    pub fn registered(&self) -> bool {
        self.control.strong_count() > 0
            && self.entry.upgrade().is_some_and(|entry| entry.active.get())
    }
}
impl Drop for CallbackToken {
    fn drop(&mut self) {
        self.reset();
    }
}
pub struct LiveState {
    control: Rc<LiveControl>,
}
impl Default for LiveState {
    fn default() -> Self {
        Self::new(SaveState::default())
    }
}
impl LiveState {
    pub fn new(stored: SaveState) -> Self {
        Self {
            control: Rc::new(LiveControl {
                stored: RefCell::new(stored),
                entries: RefCell::new(Vec::new()),
                ready: Cell::new(false),
            }),
        }
    }
    pub fn register_callbacks(
        &self,
        save: Option<Callback>,
        load: Option<Callback>,
    ) -> CallbackToken {
        let entry = Rc::new(CallbackEntry {
            active: Cell::new(true),
            save: RefCell::new(save),
            load: RefCell::new(load),
        });
        self.control.entries.borrow_mut().push(entry.clone());
        CallbackToken {
            control: Rc::downgrade(&self.control),
            entry: Rc::downgrade(&entry),
        }
    }
    pub fn restore(&self, key: i32, bindings: &StateBindings) {
        restore_bindings(&self.control.stored.borrow(), key, bindings);
    }
    pub fn store(&self, key: i32, bindings: &StateBindings) {
        store_bindings(&mut self.control.stored.borrow_mut(), key, bindings);
    }
    pub fn get_global<T: GlobalValue>(&self, key: i32) -> T {
        T::get(&self.control.stored.borrow(), key)
    }
    pub fn set_global<T: GlobalValue>(&self, key: i32, value: T) {
        T::set(&mut self.control.stored.borrow_mut(), key, value);
    }
    pub fn stored(&self) -> Ref<'_, SaveState> {
        self.control.stored.borrow()
    }
    fn notify(control: &Rc<LiveControl>, save: bool) {
        let snapshot = control.entries.borrow().clone();
        for entry in snapshot {
            if entry.active.get() {
                let callback = if save {
                    entry.save.borrow().clone()
                } else {
                    entry.load.borrow().clone()
                };
                if let Some(callback) = callback {
                    callback();
                }
            }
        }
    }
    pub fn save_live(&self) {
        if self.control.ready.get() {
            Self::notify(&self.control, true);
        }
    }
    pub fn reload(&self, stored: SaveState) {
        let control = self.control.clone();
        *control.stored.borrow_mut() = stored;
        Self::notify(&control, false);
        control.ready.set(true);
    }
    pub fn reset(&self) {
        let control = self.control.clone();
        control.stored.borrow_mut().clear();
        Self::notify(&control, false);
        control.ready.set(true);
    }
}
pub trait GlobalValue: Sized {
    fn get(state: &SaveState, key: i32) -> Self;
    fn set(state: &mut SaveState, key: i32, value: Self);
}
macro_rules! global_value {
    ($type:ty,$get:ident,$set:ident,$default:expr) => {
        impl GlobalValue for $type {
            fn get(state: &SaveState, key: i32) -> Self {
                state
                    .$get(key)
                    .and_then(|v| v.first())
                    .copied()
                    .unwrap_or($default)
            }
            fn set(state: &mut SaveState, key: i32, value: Self) {
                state.$set(key, vec![value]);
            }
        }
    };
}
global_value!(bool, bool_values, set_bool_values, false);
global_value!(i32, int_values, set_int_values, 0);
global_value!(Point, point_values, set_point_values, Point { x: 0, y: 0 });
global_value!(
    Point3,
    point3_values,
    set_point3_values,
    Point3 { x: 0, y: 0, z: 0 }
);
struct ObjectControl {
    live: Weak<LiveControl>,
    key: i32,
    bindings: StateBindings,
}
impl ObjectControl {
    fn save(&self) {
        if let Some(live) = self.live.upgrade() {
            store_bindings(&mut live.stored.borrow_mut(), self.key, &self.bindings);
        }
    }
    fn load(&self) {
        if let Some(live) = self.live.upgrade() {
            restore_bindings(&live.stored.borrow(), self.key, &self.bindings);
        }
    }
}
pub struct StateObject {
    control: Rc<ObjectControl>,
    callbacks: CallbackToken,
}
impl StateObject {
    pub fn new(live: &LiveState, key: i32, bindings: StateBindings) -> Self {
        let control = Rc::new(ObjectControl {
            live: Rc::downgrade(&live.control),
            key,
            bindings,
        });
        control.load();
        let save = Rc::downgrade(&control);
        let load = Rc::downgrade(&control);
        let callbacks = live.register_callbacks(
            Some(Rc::new(move || {
                if let Some(control) = save.upgrade() {
                    control.save();
                }
            })),
            Some(Rc::new(move || {
                if let Some(control) = load.upgrade() {
                    control.load();
                }
            })),
        );
        Self { control, callbacks }
    }
    pub fn save(&self) {
        self.control.save();
    }
    pub fn load(&self) {
        self.control.load();
    }
    pub fn unregister(&mut self) {
        self.callbacks.reset();
    }
    pub fn recycle(&mut self) {
        if self.registered() {
            self.save();
            self.unregister();
        }
    }
    pub fn registered(&self) -> bool {
        self.callbacks.registered()
    }
}
