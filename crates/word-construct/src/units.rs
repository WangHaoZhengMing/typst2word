//! Exact unit conversions and strongly-typed dimensions (Newtypes) for WordprocessingML (OOXML).
//!
//! WordprocessingML uses several distinct measurement units across elements:
//! - **Points ([`Pt`])**: $1\text{ pt} = 1/72\text{ inch}$. Typst's native layout coordinate unit.
//! - **Twips ([`Dxa`])**: $1/20\text{ pt}$ ($1/1440\text{ inch}$). Used for page sizes, margins,
//!   paragraph spacing (`w:before`, `w:after`), indentation (`w:ind`), and table widths (`w:tcW`, `w:gridCol`).
//! - **240ths of a single line ([`WLine`])**: Used for `<w:spacing w:line="..." w:lineRule="auto"/>`,
//!   where `240` equals `1.0x` single-line spacing as defined by the font's `OS/2` metrics.
//! - **Half-points ([`HalfPt`])**: $1/2\text{ pt}$. Used for font sizes (`<w:sz w:val="..."/>`).
//! - **Eighth-points ([`EighthPt`])**: $1/8\text{ pt}$. Used for border thicknesses (`<w:top w:sz="..."/>`).
//! - **English Metric Units ([`Emu`])**: $1\text{ pt} = 12700\text{ EMU}$ ($1\text{ inch} = 914400\text{ EMU}$).
//!   Used for DrawingML (`<wp:extent>`, `<a:ext>`, `<a:off>`).

use std::fmt;
use std::ops::{Add, Div, Mul, Sub};

use serde::{Deserialize, Serialize};

// ============================================================================
// 1. Declarative Macro for Strongly-Typed Physical Units (Newtypes)
// ============================================================================

macro_rules! define_scalar_unit {
    (
        $(#[$meta:meta])*
        pub struct $name:ident(pub $inner:ty);
    ) => {
        $(#[$meta])*
        #[repr(transparent)]
        #[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default, Serialize, Deserialize)]
        pub struct $name(pub $inner);

        impl $name {
            #[inline]
            pub const fn new(val: $inner) -> Self {
                Self(val)
            }

            #[inline]
            pub const fn raw(self) -> $inner {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl Add for $name {
            type Output = Self;
            #[inline]
            fn add(self, rhs: Self) -> Self::Output {
                Self(self.0 + rhs.0)
            }
        }

        impl Sub for $name {
            type Output = Self;
            #[inline]
            fn sub(self, rhs: Self) -> Self::Output {
                Self(self.0 - rhs.0)
            }
        }
    };
}

define_scalar_unit! {
    /// Physical typographic size or distance in points (`pt`, $1/72\text{ inch}$).
    pub struct Pt(pub f64);
}

define_scalar_unit! {
    /// WordprocessingML twips (`dxa`, $1/20\text{ pt}$).
    pub struct Dxa(pub i64);
}

define_scalar_unit! {
    /// WordprocessingML `<w:spacing w:line="W" w:lineRule="auto"/>` unit ($1/240$ of a single line).
    pub struct WLine(pub i64);
}

define_scalar_unit! {
    /// WordprocessingML `<w:sz w:val="..."/>` font size unit ($1/2\text{ pt}$).
    pub struct HalfPt(pub u32);
}

define_scalar_unit! {
    /// WordprocessingML `<w:bdr w:sz="..."/>` border width unit ($1/8\text{ pt}$).
    pub struct EighthPt(pub u32);
}

define_scalar_unit! {
    /// DrawingML English Metric Unit (`EMU`, $1\text{ pt} = 12700\text{ EMU}$).
    pub struct Emu(pub i64);
}

impl Eq for Dxa {}
impl Ord for Dxa {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl Eq for WLine {}
impl Ord for WLine {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl Eq for HalfPt {}
impl Ord for HalfPt {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl Eq for EighthPt {}
impl Ord for EighthPt {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl Eq for Emu {}
impl Ord for Emu {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
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

impl Pt {
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

    /// Convert points to twips ([`Dxa`]).
    #[inline]
    pub fn to_dxa(self) -> Dxa {
        Dxa::from(self)
    }

    /// Convert points to DrawingML [`Emu`].
    #[inline]
    pub fn to_emu(self) -> Emu {
        Emu::from(self)
    }

    /// Convert points to Word [`HalfPt`] (`w:sz`).
    #[inline]
    pub fn to_half_pt(self) -> HalfPt {
        HalfPt::from(self)
    }

    /// Convert points to Word [`EighthPt`] (border `w:sz`).
    #[inline]
    pub fn to_eighth_pt(self) -> EighthPt {
        EighthPt::from(self)
    }
}

impl From<Pt> for Dxa {
    #[inline]
    fn from(pt: Pt) -> Self {
        match pt.0.is_finite() {
            true => Self((pt.0 * 20.0).round() as i64),
            false => Self(0),
        }
    }
}

impl From<Dxa> for Pt {
    #[inline]
    fn from(dxa: Dxa) -> Self {
        Self(dxa.0 as f64 / 20.0)
    }
}

impl From<Pt> for Emu {
    #[inline]
    fn from(pt: Pt) -> Self {
        match pt.0.is_finite() {
            true => Self((pt.0 * 12700.0).round() as i64),
            false => Self(0),
        }
    }
}

impl From<Pt> for HalfPt {
    #[inline]
    fn from(pt: Pt) -> Self {
        match pt.0 {
            v if v.is_finite() && v > 0.0 => Self((v * 2.0).round().max(2.0) as u32),
            _ => Self(22),
        }
    }
}

impl From<Pt> for EighthPt {
    #[inline]
    fn from(pt: Pt) -> Self {
        match pt.0 {
            v if v.is_finite() && v > 0.0 => Self(((v * 8.0).round() as u32).clamp(2, 96)),
            _ => Self(2),
        }
    }
}

// ============================================================================
// 2. Ergonomic Conversion Functions (Backed by Newtypes)
// ============================================================================

/// Convert points (`pt`) to twips (`dxa`, $1\text{ pt} = 20\text{ dxa}$).
#[inline]
pub fn pt_to_dxa(pt: f64) -> i64 {
    Dxa::from(Pt(pt)).0
}

/// Convert twips (`dxa`) back to points (`pt`).
#[inline]
pub fn dxa_to_pt(dxa: i64) -> f64 {
    Pt::from(Dxa(dxa)).0
}

/// Convert points (`pt`) to DrawingML English Metric Units (`EMU`, $1\text{ pt} = 12700\text{ EMU}$).
#[inline]
pub fn pt_to_emu(pt: f64) -> i64 {
    Emu::from(Pt(pt)).0
}

/// Convert points (`pt`) to Word half-points ($1\text{ pt} = 2\text{ half-pt}$) for `<w:sz>`.
#[inline]
pub fn pt_to_half_pt(pt: f64) -> u32 {
    HalfPt::from(Pt(pt)).0
}

/// Convert points (`pt`) to Word eighth-points ($1\text{ pt} = 8\text{ eighth-pt}$) for border `w:sz`.
#[inline]
pub fn pt_to_eighth_pt(pt: f64) -> u32 {
    EighthPt::from(Pt(pt)).0
}
