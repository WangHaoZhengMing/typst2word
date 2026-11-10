//! Physical typographic point unit (`Pt`, $1\text{ pt} = 1/72\text{ inch}$).
//!
//! Typst's native layout coordinate unit. Target-specific document units (such as
//! WordprocessingML `Dxa`, `WLine`, `HalfPt`, `EighthPt`, and DrawingML `Emu`) live in
//! `word-construct::units`.

use std::fmt;
use std::ops::{Add, Div, Mul, Sub};

use serde::{Deserialize, Serialize};

/// Physical typographic size or distance in points (`pt`, $1/72\text{ inch}$).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default, Serialize, Deserialize)]
pub struct Pt(pub f64);

impl Pt {
    #[inline]
    pub const fn new(val: f64) -> Self {
        Self(val)
    }

    #[inline]
    pub const fn raw(self) -> f64 {
        self.0
    }

    /// Clamp point value to `[min, max]`.
    #[inline]
    pub fn clamp(self, min: f64, max: f64) -> Self {
        Self(self.0.clamp(min, max))
    }

    /// Maximum of `self` and `other`.
    #[inline]
    pub fn max(self, other: f64) -> Self {
        Self(self.0.max(other))
    }

    /// Minimum of `self` and `other`.
    #[inline]
    pub fn min(self, other: f64) -> Self {
        Self(self.0.min(other))
    }
}

impl fmt::Display for Pt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Add for Pt {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for Pt {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Mul<f64> for Pt {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: f64) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl Div<f64> for Pt {
    type Output = Self;
    #[inline]
    fn div(self, rhs: f64) -> Self::Output {
        Self(self.0 / rhs)
    }
}
