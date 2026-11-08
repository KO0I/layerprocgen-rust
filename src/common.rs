// Copyright (c) 2024 Rune Skovbo Johansen
// SPDX-License-Identifier: MPL-2.0

use std::ops::{Add, AddAssign, Div, Index, Mul, Neg, Sub, SubAssign};

pub const fn positive_mod(value: i32, divisor: i32) -> i32 {
    assert!(divisor > 0, "divisor must be positive");
    value.rem_euclid(divisor)
}

pub const fn floor_div(value: i32, divisor: i32) -> i32 {
    assert!(divisor > 0, "divisor must be positive");
    value.div_euclid(divisor)
}

pub const fn ceil_div(value: i32, divisor: i32) -> i32 {
    assert!(divisor > 0, "divisor must be positive");
    value / divisor + if value % divisor > 0 { 1 } else { 0 }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
    pub const fn squared_magnitude(self) -> u64 {
        (self.x as i64 * self.x as i64) as u64 + (self.y as i64 * self.y as i64) as u64
    }
    pub fn magnitude(self) -> f64 {
        (self.squared_magnitude() as f64).sqrt()
    }
    pub const fn rotate(self, quarters: i32) -> Self {
        match quarters.rem_euclid(4) {
            1 => Self::new(self.y.wrapping_neg(), self.x),
            2 => Self::new(self.x.wrapping_neg(), self.y.wrapping_neg()),
            3 => Self::new(self.y, self.x.wrapping_neg()),
            _ => self,
        }
    }
}

impl Index<usize> for Point {
    type Output = i32;
    fn index(&self, index: usize) -> &i32 {
        match index {
            0 => &self.x,
            1 => &self.y,
            _ => panic!("Point index out of range"),
        }
    }
}
impl Add for Point {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self::new(self.x.wrapping_add(other.x), self.y.wrapping_add(other.y))
    }
}
impl Sub for Point {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self::new(self.x.wrapping_sub(other.x), self.y.wrapping_sub(other.y))
    }
}
impl Mul<i32> for Point {
    type Output = Self;
    fn mul(self, factor: i32) -> Self {
        Self::new(self.x.wrapping_mul(factor), self.y.wrapping_mul(factor))
    }
}
impl Div<i32> for Point {
    type Output = Self;
    fn div(self, divisor: i32) -> Self {
        Self::new(
            self.x
                .checked_div(divisor)
                .expect("Point division overflow or zero divisor"),
            self.y
                .checked_div(divisor)
                .expect("Point division overflow or zero divisor"),
        )
    }
}
impl Neg for Point {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(self.x.wrapping_neg(), self.y.wrapping_neg())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Point3 {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}
impl Point3 {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct GridBounds {
    pub min: Point,
    pub max: Point,
}
impl GridBounds {
    pub const fn new(min: Point, max: Point) -> Self {
        Self { min, max }
    }
    pub const fn is_empty(self) -> bool {
        self.max.x <= self.min.x || self.max.y <= self.min.y
    }
    pub fn size(self) -> Point {
        self.max - self.min
    }
    pub const fn contains(self, point: Point) -> bool {
        self.min.x <= point.x
            && self.min.y <= point.y
            && self.max.x > point.x
            && self.max.y > point.y
    }
    pub const fn contains_bounds(self, other: Self) -> bool {
        self.min.x <= other.min.x
            && self.min.y <= other.min.y
            && self.max.x >= other.max.x
            && self.max.y >= other.max.y
    }
    pub const fn overlaps(self, other: Self) -> bool {
        self.min.x <= other.max.x
            && self.min.y <= other.max.y
            && self.max.x >= other.min.x
            && self.max.y >= other.min.y
    }
    pub const fn divided(self, scale: Point) -> Self {
        Self::new(
            Point::new(
                floor_div(self.min.x, scale.x),
                floor_div(self.min.y, scale.y),
            ),
            Point::new(ceil_div(self.max.x, scale.x), ceil_div(self.max.y, scale.y)),
        )
    }
    pub fn intersection(self, other: Self) -> Self {
        Self::new(
            Point::new(self.min.x.max(other.min.x), self.min.y.max(other.min.y)),
            Point::new(self.max.x.min(other.max.x), self.max.y.min(other.max.y)),
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Fixed8(i32);

impl Fixed8 {
    pub const RESOLUTION: i32 = 256;
    pub const fn from_raw(raw: i32) -> Self {
        Self(raw)
    }
    pub const fn from_int(whole: i32) -> Self {
        Self(whole.wrapping_mul(256))
    }
    pub fn from_f64(value: f64) -> Result<Self, &'static str> {
        let raw = (value * 256.0).floor();
        if !raw.is_finite() || raw < f64::from(i32::MIN) || raw > f64::from(i32::MAX) {
            return Err("Fixed8 value out of range");
        }
        Ok(Self(raw as i32))
    }
    pub const fn raw(self) -> i32 {
        self.0
    }
    pub const fn floor_to_int(self) -> i32 {
        self.0.div_euclid(256)
    }
    pub const fn ceil_to_int(self) -> i32 {
        self.0 / 256 + if self.0 % 256 > 0 { 1 } else { 0 }
    }
    pub const fn round_to_int(self) -> i32 {
        ((self.0 as i64 + 128).div_euclid(256)) as i32
    }
    pub fn to_f64(self) -> f64 {
        f64::from(self.0) / 256.0
    }
    pub fn lerp(a: Self, b: Self, t: Self) -> Self {
        Self(a.0.wrapping_add(((i64::from(b.0.wrapping_sub(a.0)) * i64::from(t.0)) / 256) as i32))
    }
}
impl Add for Fixed8 {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self(self.0.wrapping_add(b.0))
    }
}
impl Sub for Fixed8 {
    type Output = Self;
    fn sub(self, b: Self) -> Self {
        Self(self.0.wrapping_sub(b.0))
    }
}
impl AddAssign for Fixed8 {
    fn add_assign(&mut self, b: Self) {
        *self = *self + b;
    }
}
impl SubAssign for Fixed8 {
    fn sub_assign(&mut self, b: Self) {
        *self = *self - b;
    }
}
impl Neg for Fixed8 {
    type Output = Self;
    fn neg(self) -> Self {
        Self(self.0.wrapping_neg())
    }
}
impl Mul<i32> for Fixed8 {
    type Output = Self;
    fn mul(self, b: i32) -> Self {
        Self(self.0.wrapping_mul(b))
    }
}
impl Div<i32> for Fixed8 {
    type Output = Self;
    fn div(self, b: i32) -> Self {
        Self(floor_div(self.0, b))
    }
}
impl Mul for Fixed8 {
    type Output = Self;
    fn mul(self, b: Self) -> Self {
        Self(((i64::from(self.0) * i64::from(b.0)) / 256) as i32)
    }
}
impl Div for Fixed8 {
    type Output = Self;
    fn div(self, b: Self) -> Self {
        let numerator = i64::from(self.0) * 256;
        let divisor = i64::from(b.0);
        assert_ne!(divisor, 0, "Fixed8 divisor cannot be zero");
        let q = numerator / divisor;
        let r = numerator % divisor;
        Self((q - i64::from(r != 0 && ((r < 0) != (divisor < 0)))) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RandomHash {
    seed: u32,
}
impl RandomHash {
    pub const fn new(seed: u32) -> Self {
        Self { seed }
    }
    pub const fn hash(self, value: u32) -> u32 {
        Self::finish(Self::round(
            self.seed.wrapping_add(374761393).wrapping_add(4),
            value,
        ))
    }
    pub fn hash_values(self, values: &[u32]) -> u32 {
        let mut acc = self
            .seed
            .wrapping_add(374761393)
            .wrapping_add((values.len() as u32).wrapping_mul(4));
        for &value in values {
            acc = Self::round(acc, value);
        }
        Self::finish(acc)
    }
    pub fn range(self, minimum: i32, maximum: i32, value: u32) -> i32 {
        assert!(maximum > minimum, "maximum must exceed minimum");
        (i64::from(minimum)
            + i64::from(self.hash(value) % ((i64::from(maximum) - i64::from(minimum)) as u32)))
            as i32
    }
    const fn round(value: u32, input: u32) -> u32 {
        value
            .wrapping_add(input.wrapping_mul(3266489917))
            .rotate_left(17)
            .wrapping_mul(668265263)
    }
    const fn finish(mut value: u32) -> u32 {
        value ^= value >> 15;
        value = value.wrapping_mul(2246822519);
        value ^= value >> 13;
        value = value.wrapping_mul(3266489917);
        value ^ (value >> 16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negative_coordinates_and_extremes() {
        assert_eq!(
            (
                floor_div(-257, 256),
                ceil_div(-257, 256),
                positive_mod(-257, 256)
            ),
            (-2, -1, 255)
        );
        assert_eq!(ceil_div(i32::MAX, 1), i32::MAX);
        assert_eq!(
            Point::new(i32::MIN, i32::MIN).squared_magnitude(),
            1u64 << 63
        );
        assert_eq!(
            Point::new(i32::MAX, 0) + Point::new(1, 0),
            Point::new(i32::MIN, 0)
        );
        assert_eq!(Point::new(2, 3).rotate(-1), Point::new(3, -2));
    }
    #[test]
    fn bounds_are_half_open_with_inclusive_rectangle_contact() {
        let b = GridBounds::new(Point::new(-257, -256), Point::new(0, 1));
        assert!(b.contains(b.min));
        assert!(!b.contains(b.max));
        assert_eq!(
            b.divided(Point::new(256, 256)),
            GridBounds::new(Point::new(-2, -1), Point::new(0, 1))
        );
        assert!(b.overlaps(GridBounds::new(Point::new(0, 0), Point::new(1, 1))));
    }
    #[test]
    fn hashes_match_cpp_vectors() {
        assert_eq!(RandomHash::new(0).hash(0), 0x08d6d969);
        assert_eq!(RandomHash::new(1).hash(1), 0xea951e9a);
        assert_eq!(RandomHash::new(1234).hash((-9i32) as u32), 0x409497f3);
        assert_eq!(RandomHash::new(u32::MAX).hash(u32::MAX), 0x20e3fb57);
        for value in [0, 1, 42, u32::MAX] {
            assert_eq!(
                RandomHash::new(7).hash(value),
                RandomHash::new(7).hash_values(&[value])
            );
        }
        let value = RandomHash::new(1).range(i32::MIN, i32::MAX, 0);
        assert!(value < i32::MAX);
    }
    #[test]
    fn fixed_values_round_and_divide_like_source() {
        assert_eq!(Fixed8::from_f64(-1.1).unwrap().raw(), -282);
        let f = Fixed8::from_raw(-129);
        assert_eq!(
            (f.floor_to_int(), f.ceil_to_int(), f.round_to_int()),
            (-1, 0, -1)
        );
        assert_eq!((Fixed8::from_int(-1) / Fixed8::from_int(3)).raw(), -86);
        assert_eq!((Fixed8::from_int(1) / Fixed8::from_int(-3)).raw(), -86);
        assert_eq!((Fixed8::from_raw(-1) * Fixed8::from_raw(1)).raw(), 0);
        assert_eq!(Fixed8::from_raw(i32::MAX).round_to_int(), 8388608);
        assert!(Fixed8::from_f64(f64::NAN).is_err());
        assert!(Fixed8::from_f64(f64::INFINITY).is_err());
    }
}
