//! Locale number formatting (NFR-X2): decimal comma vs point with matching thousands
//! separator, configured in-app, independent of the OS locale. Pure string transform over the
//! canonical decimal form (`-?digits[.digits]`, the money-as-strings shape later stories pass
//! in) — no arithmetic, no float, Cardinal-Rule safe. Story 2.4's grid reuses this helper.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use steadyinvest_contract::Money;
use steadyinvest_core::rounding::{DisplayField, round_for_display};

/// True minus sign (U+2212) used for display — visually distinct from the ASCII hyphen.
pub const MINUS_SIGN: char = '\u{2212}';

/// The French thousands separator. Typography wants the NARROW no-break space (U+202F), but the
/// UI's default font has no glyph for it and drew « 1540 » in a status band while the numeric
/// font drew « 1 540 » beside it (walk finding, 2026-09-24) — so the plain no-break space
/// (U+00A0), which every font carries, is emitted; the paste parser accepts both.
const NBSP: char = '\u{00A0}';
/// The typographic narrow variant (U+202F): never emitted, always accepted on parse (pasted
/// columns from CH/EU sources carry it).
const TYPOGRAPHIC_NARROW_NBSP: char = '\u{202F}';

/// The two shipped presets: `Comma` → `1 234,56` (no-break space + decimal comma),
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

    /// The `report` crate's mirror of this format, for the study PDF (G1 I) — `report` stays
    /// independent of `app`, so it carries its own two-variant enum.
    pub fn report_style(self) -> steadyinvest_report::NumberStyle {
        match self {
            NumberFormat::Comma => steadyinvest_report::NumberStyle::Comma,
            NumberFormat::Point => steadyinvest_report::NumberStyle::Point,
        }
    }

    fn decimal_separator(self) -> char {
        match self {
            NumberFormat::Comma => ',',
            NumberFormat::Point => '.',
        }
    }

    pub(crate) fn thousands_separator(self) -> char {
        match self {
            NumberFormat::Comma => NBSP,
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

/// Round an exact engine [`Decimal`] for display — the named half-up mode + the field's decimal
/// scale **both come from [`core::rounding`]** — then group it under the active locale preset. The
/// app never invents a rounding mode or a scale (Cardinal Rule / architecture §rounding): it reads
/// `core`'s and only *presents*. Story 2.6 uses this for every engine result string (§2–§5).
pub fn format_scaled(value: Decimal, field: DisplayField, format: NumberFormat) -> String {
    // `round_for_display` returns a `Decimal` carrying exactly `field.scale()` decimals, so the
    // canonical string already shows the fixed places (e.g. price → "141.00"); `format_amount`
    // only re-groups it for the locale. No arithmetic here.
    format_amount(&round_for_display(value, field).to_string(), format)
}

/// Parse a **user-entered** number under the user's number format into an exact [`Decimal`], or
/// `None` when it is blank, not a number, or **ambiguous** — never `0`, never a guess. The ONE
/// reading rule of every numeric field the user types into (study cells, positions, ledger, FX,
/// Réglages — G1 I, #237):
///
/// - Surrounding whitespace is trimmed; one leading sign, ASCII `-` or the display minus `\u{2212}`.
/// - The format's own decimal mark (`,` under [`NumberFormat::Comma`], `.` under
///   [`NumberFormat::Point`]) is the decimal mark; at most one.
/// - Grouping lives in the integer part only, and must be well-formed — a first group of 1–3 digits
///   then groups of exactly 3, one kind of separator throughout. The space family (ASCII space, the
///   no-break space the app emits, the typographic narrow one) groups under both formats; under
///   `Point` the comma groups too. « 7 5 » is refused, never read as 75.
/// - The OTHER decimal mark is read as the decimal mark when it cannot be a grouping:
///   - under `Point`, a comma that forms a well-formed grouping IS grouping (« 1,234 » = 1234, the
///     format says so); a single comma that cannot group (« 10,5 », « 1,2345 ») is the decimal mark;
///   - under `Comma`, the point is foreign to the format: a single point is the decimal mark
///     (« 10.5 », « 0.925 »), EXCEPT when it could be a foreign thousands point — 1 to 3 digits not
///     starting with 0, then exactly 3 digits (« 1.234 », « 12.500 »): that is ambiguous and refused.
///     Two points (« 1.234.567 ») or a point beside the comma (« 1.234,5 ») are refused.
/// - Both sides of a decimal mark carry digits (« ,5 » and « 5, » are refused).
///
/// Pure string→`Decimal` — **no arithmetic** (Cardinal Rule); `Decimal::from_str_exact` enforces
/// exactness (no float, no silent rounding, no scientific notation).
pub fn parse_decimal(input: &str, format: NumberFormat) -> Option<Decimal> {
    let trimmed = input.trim();
    let (negative, body) = match trimmed.strip_prefix(['-', MINUS_SIGN]) {
        Some(rest) => (true, rest),
        None => (false, trimmed),
    };
    let is_space = |c: char| matches!(c, ' ' | NBSP | TYPOGRAPHIC_NARROW_NBSP);
    if body.is_empty()
        || !body
            .chars()
            .all(|c| c.is_ascii_digit() || c == ',' || c == '.' || is_space(c))
    {
        return None;
    }
    let decimal = format.decimal_separator();
    let other = match format {
        NumberFormat::Comma => '.',
        NumberFormat::Point => ',',
    };
    let split_at = |mark: char| body.split_once(mark).map(|(i, f)| (i, Some(f)));
    let (integer, fraction) = match (body.matches(decimal).count(), format) {
        (0, NumberFormat::Point) => {
            if grouped_digits(body, format).is_some() {
                (body, None)
            } else if body.matches(other).count() == 1 {
                split_at(other)?
            } else {
                return None;
            }
        }
        (0, NumberFormat::Comma) => match body.matches(other).count() {
            0 => (body, None),
            1 => {
                let (int, frac) = body.split_once(other)?;
                let could_group = (1..=3).contains(&int.len())
                    && int.bytes().all(|b| b.is_ascii_digit())
                    && !int.starts_with('0')
                    && frac.len() == 3
                    && frac.bytes().all(|b| b.is_ascii_digit());
                if could_group {
                    return None;
                }
                (int, Some(frac))
            }
            _ => return None,
        },
        (1, NumberFormat::Comma) if body.contains(other) => return None,
        (1, _) => split_at(decimal)?,
        _ => return None,
    };
    let mut canonical = String::with_capacity(body.len() + 1);
    if negative {
        canonical.push('-');
    }
    canonical.push_str(&grouped_digits(integer, format)?);
    if let Some(fraction) = fraction {
        if fraction.is_empty() || !fraction.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        canonical.push('.');
        canonical.push_str(fraction);
    }
    Decimal::from_str_exact(&canonical).ok()
}

/// The digits of a user-typed INTEGER part, its grouping removed — or `None` when it is empty,
/// carries a non-digit, or groups badly (first group 1–3 digits, then exactly 3; one separator
/// kind). The space family groups under both formats; the comma under `Point` only.
fn grouped_digits(integer: &str, format: NumberFormat) -> Option<String> {
    let is_space = |c: char| matches!(c, ' ' | NBSP | TYPOGRAPHIC_NARROW_NBSP);
    let has_space = integer.chars().any(is_space);
    let has_comma = format == NumberFormat::Point && integer.contains(',');
    if has_space && has_comma {
        return None;
    }
    let groups: Vec<&str> = if has_space {
        integer.split(is_space).collect()
    } else if has_comma {
        integer.split(',').collect()
    } else {
        vec![integer]
    };
    let digits = |g: &str| !g.is_empty() && g.bytes().all(|b| b.is_ascii_digit());
    let (first, rest) = groups.split_first()?;
    let well_formed = digits(first)
        && (rest.is_empty() || first.len() <= 3)
        && rest.iter().all(|g| g.len() == 3 && digits(g));
    well_formed.then(|| groups.concat())
}

/// The [`parse_decimal`] reading as the canonical decimal spelling the journal stores (trailing
/// zeros dropped), or `None` under the same refusals. For the Rust-side rails that validate a
/// canonical string (Réglages).
pub fn canonical_input(input: &str, format: NumberFormat) -> Option<String> {
    parse_decimal(input, format).map(|d| d.normalize().to_string())
}

/// Parse a **user-entered** amount under the active locale preset into an exact [`Money`], or
/// `None` for blank / ambiguous / non-numeric input — **never `0`** (the prior project's
/// blank-coercion bug). The production INVERSE of [`format_amount`] (Story 2.4); the reading rule
/// is [`parse_decimal`]'s.
pub fn parse_amount(input: &str, format: NumberFormat) -> Option<Money> {
    parse_decimal(input, format).map(Money::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comma_preset_groups_with_narrow_nbsp_and_decimal_comma() {
        assert_eq!(
            format_amount("1234567.89", NumberFormat::Comma),
            "1\u{00A0}234\u{00A0}567,89"
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
            "\u{2212}1\u{00A0}234,5"
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

    // ── Story 2.4: the production entry parser (inverse of `format_amount`) ──

    fn money(s: &str) -> Money {
        Money::from(rust_decimal::Decimal::from_str_exact(s).unwrap())
    }

    #[test]
    fn comma_preset_parses_nbsp_group_and_decimal_comma() {
        // The CH/EU case the Spike-A paste explicitly deferred: "1 234,56" (narrow NBSP + comma).
        assert_eq!(
            parse_amount("1\u{202F}234,56", NumberFormat::Comma),
            Some(money("1234.56"))
        );
        // The plain no-break space the app itself emits round-trips too.
        assert_eq!(
            parse_amount("1\u{00A0}234,56", NumberFormat::Comma),
            Some(money("1234.56"))
        );
        // A user who types a plain ASCII space for the group separator is tolerated.
        assert_eq!(
            parse_amount("1 234,56", NumberFormat::Comma),
            Some(money("1234.56"))
        );
        // The bare decimal-comma case.
        assert_eq!(parse_amount("1,5", NumberFormat::Comma), Some(money("1.5")));
    }

    #[test]
    fn point_preset_parses_comma_group_and_decimal_point() {
        assert_eq!(
            parse_amount("1,234.56", NumberFormat::Point),
            Some(money("1234.56"))
        );
        assert_eq!(
            parse_amount("1234.56", NumberFormat::Point),
            Some(money("1234.56"))
        );
    }

    #[test]
    fn both_minus_signs_parse() {
        assert_eq!(
            parse_amount("\u{2212}12,5", NumberFormat::Comma),
            Some(money("-12.5"))
        );
        assert_eq!(
            parse_amount("-12.5", NumberFormat::Point),
            Some(money("-12.5"))
        );
    }

    #[test]
    fn blank_and_ambiguous_and_non_numeric_are_none_never_zero() {
        for (input, format) in [
            ("", NumberFormat::Comma),
            ("   ", NumberFormat::Comma),
            ("1.2.3", NumberFormat::Point),
            ("12a", NumberFormat::Comma),
            ("--2", NumberFormat::Point),
            ("\u{2212}", NumberFormat::Comma),
            ("abc", NumberFormat::Point),
        ] {
            assert_eq!(
                parse_amount(input, format),
                None,
                "{input:?} must map to None, never 0"
            );
        }
    }

    #[test]
    fn format_scaled_reads_core_scale_and_groups_for_the_locale() {
        // Price field → 2 decimals, half-up from core::rounding; grouped under the preset. The app
        // applies neither the scale nor the rounding itself — both come from `core`.
        assert_eq!(
            format_scaled(
                Decimal::new(1234567, 3),
                DisplayField::Price,
                NumberFormat::Comma
            ),
            "1\u{00A0}234,57", // 1234.567 → half-up 2dp → 1234.57
        );
        // Ratio field → 1 decimal; 3.05 → 3.1 (half-up, not banker's).
        assert_eq!(
            format_scaled(
                Decimal::new(305, 2),
                DisplayField::Ratio,
                NumberFormat::Point
            ),
            "3.1"
        );
        // Large monetary → 0 decimals.
        assert_eq!(
            format_scaled(
                Decimal::new(12345, 1),
                DisplayField::LargeMonetary,
                NumberFormat::Point
            ),
            "1,235"
        );
    }

    // ── G1 I (#237): the one reading rule of every numeric user input ──

    fn dec(s: &str) -> Decimal {
        Decimal::from_str_exact(s).unwrap()
    }

    #[test]
    fn comma_format_reads_its_own_spelling_and_an_unambiguous_point() {
        let f = NumberFormat::Comma;
        assert_eq!(parse_decimal("10,5", f), Some(dec("10.5")));
        assert_eq!(parse_decimal(" 1 234,5 ", f), Some(dec("1234.5")));
        assert_eq!(parse_decimal("1\u{00A0}234,5", f), Some(dec("1234.5")));
        assert_eq!(
            parse_decimal("1\u{202F}234\u{202F}567", f),
            Some(dec("1234567"))
        );
        assert_eq!(parse_decimal("1,234", f), Some(dec("1.234")));
        // The other mark, where it cannot be a thousands point.
        assert_eq!(parse_decimal("10.5", f), Some(dec("10.5")));
        assert_eq!(parse_decimal("0.925", f), Some(dec("0.925")));
        assert_eq!(parse_decimal("1.2345", f), Some(dec("1.2345")));
        assert_eq!(parse_decimal("1234.567", f), Some(dec("1234.567")));
        assert_eq!(parse_decimal("1 234.5", f), Some(dec("1234.5")));
        assert_eq!(parse_decimal("\u{2212}12,5", f), Some(dec("-12.5")));
    }

    #[test]
    fn point_format_reads_its_own_spelling_and_an_unambiguous_comma() {
        let f = NumberFormat::Point;
        assert_eq!(parse_decimal("10.5", f), Some(dec("10.5")));
        assert_eq!(parse_decimal("1,234.5", f), Some(dec("1234.5")));
        assert_eq!(parse_decimal("1,234,567", f), Some(dec("1234567")));
        assert_eq!(parse_decimal("1 234.5", f), Some(dec("1234.5")));
        // A comma that groups well IS grouping under this format — never 1.234.
        assert_eq!(parse_decimal("1,234", f), Some(dec("1234")));
        // A comma that cannot group is the decimal mark.
        assert_eq!(parse_decimal("10,5", f), Some(dec("10.5")));
        assert_eq!(parse_decimal("1,2345", f), Some(dec("1.2345")));
        assert_eq!(parse_decimal("1 234,5", f), Some(dec("1234.5")));
        assert_eq!(parse_decimal("-0,5", f), Some(dec("-0.5")));
    }

    #[test]
    fn ambiguous_or_malformed_input_is_refused_never_guessed() {
        for (input, format) in [
            // A foreign thousands point under the comma format.
            ("1.234", NumberFormat::Comma),
            ("12.500", NumberFormat::Comma),
            ("1.234.567", NumberFormat::Comma),
            ("1.234,5", NumberFormat::Comma),
            // Two decimal marks, badly formed groups.
            ("1,2,3", NumberFormat::Comma),
            ("1.2.3", NumberFormat::Point),
            ("12,34,567", NumberFormat::Point),
            ("1,234 567", NumberFormat::Point),
            ("7 5", NumberFormat::Comma),
            ("1234 567", NumberFormat::Comma),
            ("1 234,5,6", NumberFormat::Comma),
            ("1,234.5,6", NumberFormat::Point),
            // Bare marks, blank, signs, letters, exponents.
            (",5", NumberFormat::Comma),
            ("5,", NumberFormat::Comma),
            (".5", NumberFormat::Point),
            ("", NumberFormat::Point),
            ("  ", NumberFormat::Comma),
            ("-", NumberFormat::Comma),
            ("--2", NumberFormat::Point),
            ("12 %", NumberFormat::Comma),
            ("1e5", NumberFormat::Point),
            ("dix", NumberFormat::Comma),
        ] {
            assert_eq!(
                parse_decimal(input, format),
                None,
                "{input:?} under {format:?}"
            );
        }
    }

    #[test]
    fn canonical_input_stores_the_reading_without_trailing_zeros() {
        assert_eq!(
            canonical_input("12,50", NumberFormat::Comma).as_deref(),
            Some("12.5")
        );
        assert_eq!(
            canonical_input("1,000", NumberFormat::Point).as_deref(),
            Some("1000")
        );
        assert_eq!(canonical_input("1.000", NumberFormat::Comma), None);
    }

    #[test]
    fn round_trip_format_then_parse_is_value_stable() {
        for format in [NumberFormat::Comma, NumberFormat::Point] {
            for canonical in ["0", "0.5", "141.50", "-1234.5", "1234567.89", "200"] {
                let shown = format_amount(canonical, format);
                assert_eq!(
                    parse_amount(&shown, format),
                    Some(money(canonical)),
                    "parse(format({canonical:?})) must recover the value under {format:?}"
                );
            }
        }
    }
}
