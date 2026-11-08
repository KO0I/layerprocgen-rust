// Copyright (c) 2024 Rune Skovbo Johansen
// SPDX-License-Identifier: MPL-2.0
use layerprocgen::common::{Point, Point3};
use layerprocgen::save_state::{
    CallbackToken, LiveState, SaveState, StateBindings, StateObject, StateValue,
};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "layerprocgen-save-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn all_typed_values_roundtrip() {
    let mut s = SaveState::new();
    s.set_bool_values(-7, vec![true, false, true]);
    s.set_int_values(4, vec![i32::MIN, 0, i32::MAX]);
    s.set_point_values(12, vec![Point::new(1, -2)]);
    s.set_point3_values(-19, vec![Point3::new(5, 6, 7)]);
    let decoded = SaveState::from_bytes(&s.to_bytes().unwrap()).unwrap();
    assert_eq!(decoded, s);
    assert!(decoded.int_values(999).is_none());
}
#[test]
fn encoding_matches_cpp_golden_bytes() {
    let mut s = SaveState::new();
    s.set_bool_values(-2, vec![true, false]);
    let golden: &[u8] = &[
        b'L', b'P', b'C', b'G', b'S', b'A', b'V', b'E', 1, 0, 16, 0, 42, 0, 0, 0, 1, 0, 0, 0, 1, 0,
        0, 0, 254, 255, 255, 255, 2, 0, 0, 0, 1, 0, 2, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0,
        4, 0, 0, 0, 0, 0, 0, 0,
    ];
    assert_eq!(s.to_bytes().unwrap(), golden);
    assert_eq!(SaveState::from_bytes(golden).unwrap(), s);
}
#[test]
fn encoding_sorts_signed_keys_independent_of_insertion() {
    let mut a = SaveState::new();
    a.set_int_values(9, vec![90]);
    a.set_int_values(-2, vec![-20]);
    let mut b = SaveState::new();
    b.set_int_values(-2, vec![-20]);
    b.set_int_values(9, vec![90]);
    let bytes = a.to_bytes().unwrap();
    assert_eq!(bytes, b.to_bytes().unwrap());
    assert_eq!(&bytes[32..36], &(-2i32).to_le_bytes());
    assert_eq!(&bytes[44..48], &9i32.to_le_bytes());
}
#[test]
fn every_truncated_prefix_is_rejected() {
    let mut s = SaveState::new();
    s.set_point3_values(1, vec![Point3::new(2, 3, 4)]);
    let bytes = s.to_bytes().unwrap();
    for length in 0..bytes.len() {
        assert!(
            SaveState::from_bytes(&bytes[..length]).is_err(),
            "accepted {length}"
        );
    }
}
#[test]
fn invalid_header_sections_reserved_fields_and_bools_are_rejected() {
    let mut s = SaveState::new();
    s.set_bool_values(1, vec![true]);
    s.set_int_values(2, vec![3]);
    let original = s.to_bytes().unwrap();
    for (offset, value) in [
        (0, 255),
        (8, 2),
        (10, 15),
        (12, 255),
        (16, 9),
        (17, 1),
        (32, 2),
    ] {
        let mut bytes = original.clone();
        bytes[offset] = value;
        assert!(
            SaveState::from_bytes(&bytes).is_err(),
            "accepted invalid byte {offset}"
        );
    }
    let mut trailing = original;
    trailing.push(0);
    assert!(SaveState::from_bytes(&trailing).is_err());
}
#[test]
fn duplicate_keys_and_unreasonable_counts_are_rejected() {
    let mut s = SaveState::new();
    s.set_bool_values(1, vec![]);
    s.set_bool_values(2, vec![]);
    let mut duplicate = s.to_bytes().unwrap();
    let key: [u8; 4] = duplicate[24..28].try_into().unwrap();
    duplicate[32..36].copy_from_slice(&key);
    assert!(SaveState::from_bytes(&duplicate).is_err());
    let mut huge = SaveState::new().to_bytes().unwrap();
    huge[20..24].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(SaveState::from_bytes(&huge).is_err());
    let mut huge = s.to_bytes().unwrap();
    huge[28..32].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(SaveState::from_bytes(&huge).is_err());
}
#[test]
fn file_replacement_cleans_temporary_files() {
    let dir = Temp::new();
    let path = dir.0.join("state.bin");
    let mut s = SaveState::new();
    s.set_point_values(77, vec![Point::new(-1, 8)]);
    s.save_file(&path).unwrap();
    assert_eq!(SaveState::load_file(&path).unwrap(), s);
    let mut replacement = SaveState::new();
    replacement.set_int_values(-5, vec![6, 7]);
    replacement.save_file(&path).unwrap();
    assert_eq!(SaveState::load_file(&path).unwrap(), replacement);
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 1);
    assert!(SaveState::load_file(dir.0.join("missing.bin")).is_err());
}
#[test]
fn state_values_restore_defaults() {
    let value = StateValue::new(Point::new(-2, 5));
    value.set(Point::new(8, 9));
    assert_eq!(value.default_value(), Point::new(-2, 5));
    value.restore_default();
    assert_eq!(value.value(), Point::new(-2, 5));
}
#[test]
fn globals_are_independent_for_each_type() {
    let live = LiveState::default();
    assert!(!live.get_global::<bool>(7));
    assert_eq!(live.get_global::<i32>(7), 0);
    assert_eq!(live.get_global::<Point>(7), Point::default());
    assert_eq!(live.get_global::<Point3>(7), Point3::default());
    live.set_global(7, true);
    live.set_global(7, -12i32);
    live.set_global(7, Point::new(3, 4));
    live.set_global(7, Point3::new(5, 6, 7));
    assert!(live.get_global::<bool>(7));
    assert_eq!(live.get_global::<i32>(7), -12);
    assert_eq!(live.get_global::<Point>(7), Point::new(3, 4));
    assert_eq!(live.get_global::<Point3>(7), Point3::new(5, 6, 7));
}
#[test]
fn bound_objects_load_reload_and_reset_all_types() {
    let mut s = SaveState::new();
    s.set_bool_values(41, vec![false]);
    s.set_int_values(41, vec![12]);
    s.set_point_values(41, vec![Point::new(8, 9)]);
    s.set_point3_values(41, vec![Point3::new(1, 2, 3)]);
    let live = LiveState::new(s);
    let enabled = StateValue::new(true);
    let count = StateValue::new(-1i32);
    let cell = StateValue::new(Point::new(-2, -3));
    let voxel = StateValue::new(Point3::new(-4, -5, -6));
    let mut bind = StateBindings::new();
    bind.bind_bool(&enabled);
    bind.bind_int(&count);
    bind.bind_point(&cell);
    bind.bind_point3(&voxel);
    let _object = StateObject::new(&live, 41, bind);
    assert!(!enabled.value());
    assert_eq!(count.value(), 12);
    assert_eq!(cell.value(), Point::new(8, 9));
    assert_eq!(voxel.value(), Point3::new(1, 2, 3));
    let mut next = SaveState::new();
    next.set_int_values(41, vec![27]);
    live.reload(next);
    assert_eq!(count.value(), 27);
    assert!(enabled.value());
    live.reset();
    assert_eq!(count.value(), -1);
    assert_eq!(cell.value(), Point::new(-2, -3));
    assert_eq!(voxel.value(), Point3::new(-4, -5, -6));
}
#[test]
fn length_mismatch_updates_only_common_prefix() {
    let mut s = SaveState::new();
    s.set_int_values(1, vec![4]);
    let live = LiveState::new(s);
    let a = StateValue::new(-1i32);
    let b = StateValue::new(-2i32);
    b.set(99);
    let mut bind = StateBindings::new();
    bind.bind_int(&a);
    bind.bind_int(&b);
    let _object = StateObject::new(&live, 1, bind);
    assert_eq!(a.value(), 4);
    assert_eq!(b.value(), 99);
}
#[test]
fn live_save_waits_for_first_reset_or_reload() {
    let live = LiveState::default();
    let calls = Rc::new(RefCell::new(0));
    let out = calls.clone();
    let _token = live.register_callbacks(Some(Rc::new(move || *out.borrow_mut() += 1)), None);
    live.save_live();
    assert_eq!(*calls.borrow(), 0);
    live.reload(SaveState::new());
    live.save_live();
    assert_eq!(*calls.borrow(), 1);
}
#[test]
fn live_save_stores_current_bound_values() {
    let live = LiveState::default();
    let enabled = StateValue::new(false);
    let count = StateValue::new(3i32);
    let mut bind = StateBindings::new();
    bind.bind_bool(&enabled);
    bind.bind_int(&count);
    let _object = StateObject::new(&live, 12, bind);
    live.reset();
    enabled.set(true);
    count.set(81);
    live.save_live();
    let s = live.stored();
    assert_eq!(s.bool_values(12), Some(&[true][..]));
    assert_eq!(s.int_values(12), Some(&[81][..]));
}
#[test]
fn unregister_stops_notifications_and_recycle_saves() {
    let live = LiveState::default();
    let value = StateValue::new(1i32);
    let mut bind = StateBindings::new();
    bind.bind_int(&value);
    let mut object = StateObject::new(&live, 5, bind);
    object.unregister();
    value.set(77);
    live.reset();
    assert_eq!(value.value(), 77);
    assert!(live.stored().int_values(5).is_none());
    let recycled = StateValue::new(3i32);
    let mut bind = StateBindings::new();
    bind.bind_int(&recycled);
    let mut object = StateObject::new(&live, -8, bind);
    recycled.set(44);
    object.recycle();
    assert!(!object.registered());
    assert_eq!(live.stored().int_values(-8), Some(&[44][..]));
    recycled.set(91);
    live.reset();
    assert_eq!(recycled.value(), 91);
}
#[test]
fn callback_can_remove_later_registration() {
    let live = LiveState::default();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let later: Rc<RefCell<Option<CallbackToken>>> = Rc::new(RefCell::new(None));
    let out = calls.clone();
    let target = later.clone();
    let _first = live.register_callbacks(
        None,
        Some(Rc::new(move || {
            out.borrow_mut().push(1);
            if let Some(token) = target.borrow_mut().as_mut() {
                token.reset();
            }
        })),
    );
    let out = calls.clone();
    *later.borrow_mut() =
        Some(live.register_callbacks(None, Some(Rc::new(move || out.borrow_mut().push(2)))));
    live.reset();
    live.reset();
    assert_eq!(*calls.borrow(), vec![1, 1]);
}
#[test]
fn callback_can_unregister_itself() {
    let live = LiveState::default();
    let calls = Rc::new(RefCell::new(0));
    let token: Rc<RefCell<Option<CallbackToken>>> = Rc::new(RefCell::new(None));
    let out = calls.clone();
    let target = token.clone();
    *token.borrow_mut() = Some(live.register_callbacks(
        None,
        Some(Rc::new(move || {
            *out.borrow_mut() += 1;
            if let Some(token) = target.borrow_mut().as_mut() {
                token.reset();
            }
        })),
    ));
    live.reset();
    live.reset();
    assert_eq!(*calls.borrow(), 1);
}
#[test]
fn dropped_bound_values_make_callbacks_inert() {
    let live = LiveState::default();
    let value = StateValue::new(2i32);
    let mut bind = StateBindings::new();
    bind.bind_int(&value);
    let _object = StateObject::new(&live, 5, bind);
    drop(value);
    live.reset();
    live.save_live();
    assert!(live.stored().int_values(5).is_none());
}
#[test]
fn either_live_state_destruction_order_is_safe() {
    let value = StateValue::new(2i32);
    let mut object = {
        let live = LiveState::default();
        let mut bind = StateBindings::new();
        bind.bind_int(&value);
        StateObject::new(&live, 5, bind)
    };
    assert!(!object.registered());
    object.save();
    object.load();
    object.recycle();
    drop(object);
    let live = LiveState::default();
    {
        let mut bind = StateBindings::new();
        bind.bind_int(&value);
        let _object = StateObject::new(&live, 6, bind);
    }
    live.reset();
}
