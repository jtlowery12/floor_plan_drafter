//! Lengths and areas. Everything in the plan is stored in inches; this module
//! turns that into what people read and type (feet-and-inches or metric).

use serde::{Deserialize, Serialize};

pub const INCH_PER_M: f32 = 39.370_08;
pub const SQIN_PER_SQFT: f32 = 144.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub enum Units {
    #[default]
    Imperial,
    Metric,
}

impl Units {
    pub fn label(self) -> &'static str {
        match self {
            Units::Imperial => "Feet & inches",
            Units::Metric => "Metric",
        }
    }

    pub fn area_unit(self) -> &'static str {
        match self {
            Units::Imperial => "sq ft",
            Units::Metric => "m²",
        }
    }

    /// Grid spacing (inches) for the fine and the heavy lines at a given zoom
    /// level (screen pixels per inch), chosen so lines stay readable.
    pub fn grid_steps(self, px_per_inch: f32) -> (f32, f32) {
        match self {
            Units::Imperial => {
                // Candidate fine steps in inches: 1", 6", 1', 2', 5', 10', 25'.
                let steps = [1.0, 6.0, 12.0, 24.0, 60.0, 120.0, 300.0, 600.0];
                let fine = steps
                    .iter()
                    .copied()
                    .find(|s| s * px_per_inch >= 11.0)
                    .unwrap_or(600.0);
                let heavy = match fine as i32 {
                    1 => 12.0,
                    6 => 12.0,
                    12 => 60.0,
                    24 => 120.0,
                    60 => 300.0,
                    120 => 600.0,
                    _ => fine * 5.0,
                };
                (fine, heavy)
            }
            Units::Metric => {
                let m = INCH_PER_M;
                let steps = [0.025, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0];
                let fine = steps
                    .iter()
                    .copied()
                    .find(|s| s * m * px_per_inch >= 11.0)
                    .unwrap_or(10.0);
                let heavy = if fine < 0.1 { 0.1 } else if fine < 1.0 { 1.0 } else { fine * 5.0 };
                (fine * m, heavy * m)
            }
        }
    }

    /// The snap increments (inches) offered in the toolbar.
    pub fn snap_options(self) -> &'static [(f32, &'static str)] {
        match self {
            Units::Imperial => &[(1.0, "1\""), (3.0, "3\""), (6.0, "6\""), (12.0, "1'")],
            Units::Metric => &[(1.968_5, "5 cm"), (3.937, "10 cm"), (9.842_5, "25 cm"), (19.685, "50 cm")],
        }
    }

    pub fn default_snap(self) -> f32 {
        match self {
            Units::Imperial => 6.0,
            Units::Metric => 3.937,
        }
    }

    /// A length such as `12' 6"` or `3.81 m`.
    pub fn len(self, inches: f32) -> String {
        match self {
            Units::Imperial => fmt_feet_inches(inches),
            Units::Metric => fmt_metric(inches),
        }
    }

    /// An area such as `1,200 sq ft` or `111.5 m²`.
    pub fn area(self, sq_inches: f32) -> String {
        match self {
            Units::Imperial => format!("{} sq ft", group(sq_inches / SQIN_PER_SQFT)),
            Units::Metric => {
                let m2 = sq_inches / (INCH_PER_M * INCH_PER_M);
                format!("{} m²", group_dec(m2, 1))
            }
        }
    }

    /// Plain number of area units (sq ft or m²) for an area in square inches.
    pub fn area_value(self, sq_inches: f32) -> f32 {
        match self {
            Units::Imperial => sq_inches / SQIN_PER_SQFT,
            Units::Metric => sq_inches / (INCH_PER_M * INCH_PER_M),
        }
    }

    /// Square inches for a number of area units.
    pub fn area_to_sq_inches(self, value: f32) -> f32 {
        match self {
            Units::Imperial => value * SQIN_PER_SQFT,
            Units::Metric => value * INCH_PER_M * INCH_PER_M,
        }
    }

    /// Parse something a person typed as a length into inches.
    pub fn parse_len(self, text: &str) -> Option<f32> {
        match self {
            Units::Imperial => parse_imperial(text),
            Units::Metric => parse_metric(text),
        }
    }
}

fn group(v: f32) -> String {
    group_dec(v, 0)
}

fn group_dec(v: f32, decimals: usize) -> String {
    let s = format!("{:.*}", decimals, v.abs());
    let (int, frac) = match s.split_once('.') {
        Some((i, f)) => (i.to_string(), Some(f.to_string())),
        None => (s, None),
    };
    let mut out = String::new();
    for (n, c) in int.chars().rev().enumerate() {
        if n > 0 && n % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    let mut out: String = out.chars().rev().collect();
    if let Some(f) = frac {
        out.push('.');
        out.push_str(&f);
    }
    if v < 0.0 && out.chars().any(|c| c != '0' && c.is_ascii_digit()) {
        out.insert(0, '-');
    }
    out
}

fn fmt_feet_inches(inches: f32) -> String {
    let neg = inches < -0.01;
    // Round to the nearest half inch.
    let total = ((inches.abs() * 2.0).round() / 2.0).max(0.0);
    let feet = (total / 12.0).floor() as i64;
    let rem = total - feet as f32 * 12.0;
    let whole = rem.floor() as i64;
    let half = rem - whole as f32 >= 0.25;
    let inch = if half {
        format!("{whole}½\"")
    } else {
        format!("{whole}\"")
    };
    let body = match (feet, whole == 0 && !half) {
        (0, _) => inch,
        (f, true) => format!("{f}'"),
        (f, false) => format!("{f}' {inch}"),
    };
    if neg { format!("-{body}") } else { body }
}

fn fmt_metric(inches: f32) -> String {
    let m = inches / INCH_PER_M;
    if m.abs() < 1.0 {
        format!("{:.0} cm", m * 100.0)
    } else {
        format!("{m:.2} m")
    }
}

fn parse_metric(text: &str) -> Option<f32> {
    let t = text.trim().to_lowercase().replace(',', ".");
    if t.is_empty() {
        return None;
    }
    let (num, scale) = if let Some(n) = t.strip_suffix("cm") {
        (n, 0.01)
    } else if let Some(n) = t.strip_suffix("mm") {
        (n, 0.001)
    } else if let Some(n) = t.strip_suffix('m') {
        (n, 1.0)
    } else {
        (t.as_str(), 1.0)
    };
    let v: f32 = num.trim().parse().ok()?;
    (v.is_finite() && v >= 0.0).then_some(v * scale * INCH_PER_M)
}

/// Accepts `12' 6"`, `12'6`, `12ft 6in`, `12.5'`, `150"`, `150in`, `12.5` (feet),
/// `12 6` (feet and inches).
fn parse_imperial(text: &str) -> Option<f32> {
    let t = text
        .trim()
        .to_lowercase()
        .replace("feet", "'")
        .replace("foot", "'")
        .replace("ft", "'")
        .replace("inches", "\"")
        .replace("inch", "\"")
        .replace("in", "\"")
        .replace('’', "'")
        .replace('”', "\"")
        .replace("''", "\"");
    if t.is_empty() {
        return None;
    }
    let num = |s: &str| -> Option<f32> {
        let s = s.trim();
        if s.is_empty() {
            return Some(0.0);
        }
        // Allow "1/2" and "6 1/2".
        let mut total = 0.0;
        for part in s.split_whitespace() {
            if let Some((a, b)) = part.split_once('/') {
                let (a, b): (f32, f32) = (a.parse().ok()?, b.parse().ok()?);
                if b == 0.0 {
                    return None;
                }
                total += a / b;
            } else {
                total += part.parse::<f32>().ok()?;
            }
        }
        total.is_finite().then_some(total)
    };
    let v = if let Some((feet, rest)) = t.split_once('\'') {
        let inches = rest.trim().trim_end_matches('"');
        num(feet)? * 12.0 + num(inches)?
    } else if let Some(inches) = t.strip_suffix('"') {
        num(inches)?
    } else {
        // Bare number: "12.5" is feet; "12 6" is 12 feet 6 inches.
        let parts: Vec<&str> = t.split_whitespace().collect();
        match parts.as_slice() {
            [f] => num(f)? * 12.0,
            [f, i] if !i.contains('/') => num(f)? * 12.0 + num(i)?,
            _ => return None,
        }
    };
    (v >= 0.0).then_some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_feet_and_inches() {
        let u = Units::Imperial;
        assert_eq!(u.len(150.0), "12' 6\"");
        assert_eq!(u.len(144.0), "12'");
        assert_eq!(u.len(30.0), "2' 6\"");
        assert_eq!(u.len(9.0), "9\"");
        assert_eq!(u.len(0.0), "0\"");
        assert_eq!(u.len(150.5), "12' 6½\"");
        assert_eq!(u.len(119.8), "10'");
    }

    #[test]
    fn formats_areas() {
        assert_eq!(Units::Imperial.area(1200.0 * 144.0), "1,200 sq ft");
        assert_eq!(Units::Imperial.area(144.0 * 168.4), "168 sq ft");
        assert_eq!(Units::Metric.area(INCH_PER_M * INCH_PER_M * 111.5), "111.5 m²");
    }

    #[test]
    fn formats_metric() {
        assert_eq!(Units::Metric.len(INCH_PER_M * 3.81), "3.81 m");
        assert_eq!(Units::Metric.len(INCH_PER_M * 0.5), "50 cm");
    }

    #[test]
    fn parses_imperial_forms() {
        let p = |s| Units::Imperial.parse_len(s);
        assert_eq!(p("12' 6\""), Some(150.0));
        assert_eq!(p("12'6"), Some(150.0));
        assert_eq!(p("12ft 6in"), Some(150.0));
        assert_eq!(p("12.5'"), Some(150.0));
        assert_eq!(p("150\""), Some(150.0));
        assert_eq!(p("150 in"), Some(150.0));
        assert_eq!(p("12.5"), Some(150.0));
        assert_eq!(p("12 6"), Some(150.0));
        assert_eq!(p("12' 6 1/2\""), Some(150.5));
        assert_eq!(p("10'"), Some(120.0));
        assert_eq!(p(""), None);
        assert_eq!(p("abc"), None);
        assert_eq!(p("-4"), None);
        assert_eq!(p("1/0\""), None);
    }

    #[test]
    fn parses_metric_forms() {
        let p = |s| Units::Metric.parse_len(s);
        assert!((p("3.5").unwrap() - 3.5 * INCH_PER_M).abs() < 0.01);
        assert!((p("3,5 m").unwrap() - 3.5 * INCH_PER_M).abs() < 0.01);
        assert!((p("350cm").unwrap() - 3.5 * INCH_PER_M).abs() < 0.01);
        assert_eq!(p("x"), None);
        assert_eq!(p("-1"), None);
    }

    #[test]
    fn area_round_trip() {
        for u in [Units::Imperial, Units::Metric] {
            let v = 1850.0;
            assert!((u.area_value(u.area_to_sq_inches(v)) - v).abs() < 0.01);
        }
    }

    #[test]
    fn grid_gets_coarser_when_zoomed_out() {
        let (near, _) = Units::Imperial.grid_steps(4.0);
        let (far, _) = Units::Imperial.grid_steps(0.2);
        assert!(far > near);
    }
}
