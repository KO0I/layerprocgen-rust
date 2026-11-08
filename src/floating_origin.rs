// Copyright (c) 2026 Rune Skovbo Johansen
// SPDX-License-Identifier: MPL-2.0
use crate::common::Point3;
use std::cell::RefCell;
use std::rc::{Rc, Weak};
type Callback = Rc<dyn Fn(Point3, Point3)>;
#[derive(Default)]
struct State {
    origin: Point3,
    next_id: u64,
    handlers: Vec<(u64, Callback)>,
}
#[derive(Clone, Default)]
pub struct FloatingOrigin {
    state: Rc<RefCell<State>>,
}
pub struct Registration {
    state: Weak<RefCell<State>>,
    id: Option<u64>,
}
impl Registration {
    pub fn reset(&mut self) {
        if let (Some(state), Some(id)) = (self.state.upgrade(), self.id.take()) {
            let removed = {
                let mut state = state.borrow_mut();
                state
                    .handlers
                    .iter()
                    .position(|(active, _)| *active == id)
                    .map(|index| state.handlers.remove(index))
            };
            drop(removed);
        }
    }
}
impl Drop for Registration {
    fn drop(&mut self) {
        self.reset();
    }
}
impl FloatingOrigin {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register_handler(&self, callback: impl Fn(Point3, Point3) + 'static) -> Registration {
        let mut state = self.state.borrow_mut();
        state.next_id = state
            .next_id
            .checked_add(1)
            .expect("floating origin registration ID exhausted");
        let id = state.next_id;
        state.handlers.push((id, Rc::new(callback)));
        Registration {
            state: Rc::downgrade(&self.state),
            id: Some(id),
        }
    }
    pub fn current(&self) -> Point3 {
        self.state.borrow().origin
    }
    pub fn set_initial(&self, origin: Point3) {
        self.state.borrow_mut().origin = origin;
        self.invoke(origin, origin);
    }
    pub fn set(&self, origin: Point3) {
        let old = {
            let mut state = self.state.borrow_mut();
            if state.origin == origin {
                return;
            }
            let old = state.origin;
            state.origin = origin;
            old
        };
        self.invoke(old, origin);
    }
    fn invoke(&self, old: Point3, new: Point3) {
        let handlers = self.state.borrow().handlers.clone();
        for (id, callback) in handlers.into_iter().rev() {
            if self
                .state
                .borrow()
                .handlers
                .iter()
                .any(|(active, _)| *active == id)
            {
                callback(old, new);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reverse_order_and_removal_during_notification() {
        let origin = FloatingOrigin::new();
        let calls = Rc::new(RefCell::new(Vec::new()));
        let first = {
            let calls = calls.clone();
            origin.register_handler(move |_, _| calls.borrow_mut().push(1))
        };
        let first = Rc::new(RefCell::new(Some(first)));
        let second = {
            let calls = calls.clone();
            let first = first.clone();
            origin.register_handler(move |_, _| {
                calls.borrow_mut().push(2);
                first.borrow_mut().take();
            })
        };
        origin.set(Point3::new(4, 5, 6));
        assert_eq!(*calls.borrow(), vec![2]);
        origin.set(Point3::new(4, 5, 6));
        assert_eq!(*calls.borrow(), vec![2]);
        drop(second);
        origin.set_initial(Point3::new(7, 8, 9));
        assert_eq!(origin.current(), Point3::new(7, 8, 9));
    }
    #[test]
    fn registration_after_origin_drop_is_safe() {
        let token = {
            let origin = FloatingOrigin::new();
            origin.register_handler(|_, _| {})
        };
        drop(token);
    }
}
