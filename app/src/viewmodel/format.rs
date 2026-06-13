//! Locale number formatting (NFR-X2): decimal comma vs point with matching thousands
//! separator, configured in-app, independent of the OS locale. Pure string transform over the
//! canonical decimal form (`-?digits[.digits]`, the money-as-strings shape later stories pass
//! in) — no arithmetic, no float, Cardinal-Rule safe. Story 2.4's grid reuses this helper.

use serde::{Deserialize, Serialize};

/// True minus sign (U+2212) used for display — visually distinct from the ASCII hyphen.
pub const MINUS_SIGN: char = '\u{2212}';

/// Narrow no-break space (U+202F), the French thousands separator.
const NARROW_NBSP: char = '\u{202F}';

/// The two shipped presets: `Comma` → `1 234,56` (narrow no-break space + decimal comma),
/// `Point` → `1,234.56` (comma thousands + decimal point).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NumberFormat {
    #[default]
    Comma,
    Point,
}

impl NumberFormat {
    /// Stable identifier used by the UI callbacks and the config file.
    pub fn as_str(self) -> &'static str {
        match self {
            NumberFormat::Comma => "comma",
            NumberFormat::Point => "point",
        }
    }

    /// Parse a UI-callback identifier; `None` for anything unknown (caller falls back).
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "comma" => Some(NumberFormat::Comma),
            "point" => Some(NumberFormat::Point),
            _ => None,
        }
    }

    fn decimal_separator(self) -> char {
        match self {
            NumberFormat::Comma => ',',
            NumberFormat::Point => '.',
        }
    }

    fn thousands_separator(self) -> char {
        match self {
            NumberFormat::Comma => NARROW_NBSP,
            NumberFormat::Point => ',',
        }
    }
}

/// Format a canonical decimal string (`-?digits[.digits]`) for display. A string that is not
/// in canonical form is returned unchanged — this is a display transform, it never invents or
/// repairs a value.
pub fn format_amount(canonical: &str, format: NumberFormat) -> String {
    let (negative, unsigned) = match canonical.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, canonical),
    };
    let (integer, fraction) = match unsigned.split_once('.') {
        Some((int, frac)) => (int, Some(frac)),
        None => (unsigned, None),
    };
    let canonical_shape = !integer.is_empty()
        && integer.bytes().all(|b| b.is_ascii_digit())
        && fraction.is_none_or(|f| !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()));
    if !canonical_shape {
        return canonical.to_string();
    }

    let mut out = String::with_capacity(canonical.len() + integer.len() / 3 + 2);
    if negative {
        out.push(MINUS_SIGN);
    }
    let first_group = match integer.len() % 3 {
        0 => 3,
        n => n,
    };
    for (i, digit) in integer.chars().enumerate() {
        if i != 0 && (i + 3 - first_group) % 3 == 0 {
            out.push(format.thousands_separator());
        }
        out.push(digit);
    }
    if let Some(fraction) = fraction {
        out.push(format.decimal_separator());
        out.push_str(fraction);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comma_preset_groups_with_narrow_nbsp_and_decimal_comma() {
        assert_eq!(
            format_amount("1234567.89", NumberFormat::Comma),
            "1\u{202F}234\u{202F}567,89"
        );
    }

    #[test]
    fn point_preset_groups_with_comma_and_decimal_point() {
        assert_eq!(
            format_amount("1234567.89", NumberFormat::Point),
            "1,234,567.89"
        );
    }

    #[test]
    fn negative_amounts_use_the_true_minus_sign() {
        assert_eq!(
            format_amount("-1234.5", NumberFormat::Comma),
            "\u{2212}1\u{202F}234,5"
        );
        assert_eq!(
            format_amount("-1234.5", NumberFormat::Point),
            "\u{2212}1,234.5"
        );
    }

    #[test]
    fn short_integers_get_no_thousands_separator() {
        assert_eq!(format_amount("0.5", NumberFormat::Comma), "0,5");
        assert_eq!(format_amount("999", NumberFormat::Point), "999");
        assert_eq!(format_amount("-12", NumberFormat::Comma), "\u{2212}12");
    }

    #[test]
    fn grouping_boundaries_are_exact() {
        assert_eq!(format_amount("1000", NumberFormat::Point), "1,000");
        assert_eq!(format_amount("100000", NumberFormat::Point), "100,000");
        assert_eq!(format_amount("1000000", NumberFormat::Point), "1,000,000");
    }

    #[test]
    fn non_canonical_input_passes_through_unchanged() {
        for weird in ["", "-", "1.2.3", "12a", "1,5", ".5", "1.", "--2"] {
            assert_eq!(format_amount(weird, NumberFormat::Comma), weird);
        }
    }

    #[test]
    fn identifiers_round_trip() {
        for format in [NumberFormat::Comma, NumberFormat::Point] {
            assert_eq!(NumberFormat::parse(format.as_str()), Some(format));
        }
        assert_eq!(NumberFormat::parse("space"), None);
    }
}
