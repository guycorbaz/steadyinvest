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

/// What a user-typed number reads as under the user's number format (G1 I review): a value, a
/// blank field, a text that is no number, or an **ambiguous** one — a number in some spelling, but
/// not unambiguously in the user's (« 1.085 » under the comma format could be 1,085 or 1085). The
/// callers tell them apart: blank may mean « clear » or « default », a non-number and an ambiguous
/// number are refused each with its own named reason — never a guess, never 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumberReading {
    Value(Decimal),
    Blank,
    NotANumber,
    Ambiguous,
}

impl NumberReading {
    /// The value, when there is one.
    pub fn value(self) -> Option<Decimal> {
        match self {
            NumberReading::Value(d) => Some(d),
            _ => None,
        }
    }
}

/// One way of spelling a number: its decimal mark, and whether the comma or the point may group.
/// The space family and the Swiss apostrophes group in every spelling.
#[derive(Clone, Copy)]
struct Spelling {
    decimal: char,
    comma_groups: bool,
    point_groups: bool,
}

/// The comma format's own spelling: « 1 234,5 », « 1'234,5 ».
const COMMA_OWN: Spelling = Spelling {
    decimal: ',',
    comma_groups: false,
    point_groups: false,
};
/// The point format's own spelling: « 1,234.5 », « 1 234.5 », « 1'234.5 ».
const POINT_OWN: Spelling = Spelling {
    decimal: '.',
    comma_groups: true,
    point_groups: false,
};
/// A decimal point without any comma — the one foreign spelling the comma format also reads,
/// when it is unambiguous (« 10.5 », « 0.925 »).
const POINT_DECIMAL_ONLY: Spelling = Spelling {
    decimal: '.',
    comma_groups: false,
    point_groups: false,
};
/// The continental spelling where the point groups (« 1.234,5 », « 1.085 » = 1085): never read,
/// only recognised — a text it reads is ambiguous for the comma format.
const POINT_GROUPS: Spelling = Spelling {
    decimal: ',',
    comma_groups: false,
    point_groups: true,
};

fn is_space(c: char) -> bool {
    matches!(c, ' ' | NBSP | TYPOGRAPHIC_NARROW_NBSP)
}

fn is_apostrophe(c: char) -> bool {
    matches!(c, '\'' | '\u{2019}')
}

/// Read a user-typed number under the user's number format — the ONE reading rule of every
/// numeric field the user types into (study cells and judgment, positions, ledger, FX, Réglages,
/// the quick screen's objective — G1 I, #237):
///
/// - Surrounding whitespace is trimmed (nothing left → [`NumberReading::Blank`]); one leading
///   sign, ASCII `-` or the display minus `\u{2212}`.
/// - Each format reads its OWN marks: `Comma` → decimal « , », grouping by the space family;
///   `Point` → decimal « . », grouping by « , » and the space family. Both also group with the
///   Swiss apostrophe (U+0027, U+2019).
/// - A grouping is well formed or it is none: a first group of 1–3 digits NOT starting with 0,
///   then groups of exactly 3, one separator kind — « 7 5 » and « 0 925 » are not numbers (never
///   75, never 925). Grouping lives in the integer part only; a decimal mark has digits on both
///   sides (« ,5 », « 5, » are not numbers).
/// - Under `Comma`, a single « . » is ALSO read as the decimal mark when it is unambiguous: not
///   when it could be a thousands point (1–3 digits not starting with 0, then exactly 3 digits:
///   « 1.085 », « 12.500 »). « 0.925 », « 10.5 », « 1234.567 » read.
/// - Under `Point`, a « , » is NEVER a decimal mark — it is the user's own grouping character.
/// - A text that is no number in the user's spelling but IS one in another (« 10,5 » or « 0,925 »
///   under `Point`; « 1.085 », « 1.234,5 », « 1,234.5 » under `Comma`) is
///   [`NumberReading::Ambiguous`]; anything else (letters, exponents, « % », stray marks) is
///   [`NumberReading::NotANumber`].
///
/// Pure string→`Decimal` — **no arithmetic** (Cardinal Rule); `Decimal::from_str_exact` enforces
/// exactness (no float, no silent rounding).
pub fn read_number(input: &str, format: NumberFormat) -> NumberReading {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return NumberReading::Blank;
    }
    let (negative, body) = match trimmed.strip_prefix(['-', MINUS_SIGN]) {
        Some(rest) => (true, rest),
        None => (false, trimmed),
    };
    if body.is_empty()
        || !body
            .chars()
            .all(|c| c.is_ascii_digit() || c == ',' || c == '.' || is_space(c) || is_apostrophe(c))
    {
        return NumberReading::NotANumber;
    }
    let reads = |spelling: Spelling| spelled(body, spelling);
    let canonical = match format {
        NumberFormat::Comma => match reads(COMMA_OWN) {
            Some(c) => Some(c),
            None => match reads(POINT_DECIMAL_ONLY) {
                // The point could be a thousands point: ambiguous, never guessed.
                Some(_) if reads(POINT_GROUPS).is_some() => return NumberReading::Ambiguous,
                Some(c) => Some(c),
                None if reads(POINT_OWN).is_some() || reads(POINT_GROUPS).is_some() => {
                    return NumberReading::Ambiguous;
                }
                None => None,
            },
        },
        NumberFormat::Point => match reads(POINT_OWN) {
            Some(c) => Some(c),
            None if reads(COMMA_OWN).is_some() || reads(POINT_GROUPS).is_some() => {
                return NumberReading::Ambiguous;
            }
            None => None,
        },
    };
    let Some(canonical) = canonical else {
        return NumberReading::NotANumber;
    };
    let signed = if negative {
        format!("-{canonical}")
    } else {
        canonical
    };
    Decimal::from_str_exact(&signed).map_or(NumberReading::NotANumber, NumberReading::Value)
}

/// The canonical unsigned spelling (`digits[.digits]`) of `body` read in `spelling`, or `None`
/// when it is not a well-formed number there.
fn spelled(body: &str, spelling: Spelling) -> Option<String> {
    let (integer, fraction) = match body.split_once(spelling.decimal) {
        Some((i, f)) => (i, Some(f)),
        None => (body, None),
    };
    let mut canonical = grouped_digits(integer, spelling)?;
    if let Some(fraction) = fraction {
        if fraction.is_empty() || !fraction.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        canonical.push('.');
        canonical.push_str(fraction);
    }
    Some(canonical)
}

/// A character class: one grouping separator kind.
type CharClass = fn(char) -> bool;

/// The digits of a user-typed INTEGER part, its grouping removed — or `None` when it is empty,
/// carries a non-digit, or groups badly (a first group of 1–3 digits not starting with 0, then
/// exactly 3; one separator kind).
fn grouped_digits(integer: &str, spelling: Spelling) -> Option<String> {
    let kinds: [(bool, CharClass); 4] = [
        (true, is_space),
        (true, is_apostrophe),
        (spelling.comma_groups, |c| c == ','),
        (spelling.point_groups, |c| c == '.'),
    ];
    let mut separator: Option<CharClass> = None;
    for (allowed, kind) in kinds {
        if allowed && integer.chars().any(kind) {
            if separator.is_some() {
                return None; // two separator kinds
            }
            separator = Some(kind);
        }
    }
    let groups: Vec<&str> = match separator {
        Some(kind) => integer.split(kind).collect(),
        None => vec![integer],
    };
    let digits = |g: &str| !g.is_empty() && g.bytes().all(|b| b.is_ascii_digit());
    let (first, rest) = groups.split_first()?;
    let well_formed = digits(first)
        && (rest.is_empty() || (first.len() <= 3 && !first.starts_with('0')))
        && rest.iter().all(|g| g.len() == 3 && digits(g));
    well_formed.then(|| groups.concat())
}

/// The value of a user-typed number, or `None` when it is blank, not a number, or ambiguous
/// ([`read_number`]'s rule).
pub fn parse_decimal(input: &str, format: NumberFormat) -> Option<Decimal> {
    read_number(input, format).value()
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

    fn value(s: &str) -> NumberReading {
        NumberReading::Value(dec(s))
    }

    #[test]
    fn comma_format_reads_its_own_spelling_and_an_unambiguous_point() {
        let f = NumberFormat::Comma;
        for (input, expected) in [
            ("10,5", "10.5"),
            (" 1 234,5 ", "1234.5"),
            ("1\u{00A0}234,5", "1234.5"),
            ("1\u{202F}234\u{202F}567", "1234567"),
            ("1'234,5", "1234.5"),
            ("1\u{2019}234\u{2019}567,25", "1234567.25"),
            ("1,234", "1.234"),
            ("0,925", "0.925"),
            ("12,500", "12.5"),
            ("007", "7"),
            // The other mark, where it cannot be a thousands point.
            ("10.5", "10.5"),
            ("0.925", "0.925"),
            ("1.2345", "1.2345"),
            ("1234.567", "1234.567"),
            ("1 234.5", "1234.5"),
            ("\u{2212}12,5", "-12.5"),
        ] {
            assert_eq!(read_number(input, f), value(expected), "{input:?}");
        }
    }

    #[test]
    fn point_format_reads_its_own_spelling_and_never_a_decimal_comma() {
        let f = NumberFormat::Point;
        for (input, expected) in [
            ("10.5", "10.5"),
            ("1,234.5", "1234.5"),
            ("1,234,567", "1234567"),
            ("1 234.5", "1234.5"),
            ("1'234.5", "1234.5"),
            // The comma is the user's own grouping: « 1,234 » is 1234, never 1.234.
            ("1,234", "1234"),
            ("0.925", "0.925"),
            ("-0.5", "-0.5"),
        ] {
            assert_eq!(read_number(input, f), value(expected), "{input:?}");
        }
    }

    #[test]
    fn a_number_in_another_spelling_is_ambiguous_never_guessed() {
        for (input, format) in [
            // A possible thousands point under the comma format.
            ("1.085", NumberFormat::Comma),
            ("12.500", NumberFormat::Comma),
            ("1.234.567", NumberFormat::Comma),
            ("1.234,5", NumberFormat::Comma),
            ("1,234.5", NumberFormat::Comma),
            // A comma is never a decimal mark under the point format.
            ("10,5", NumberFormat::Point),
            ("0,925", NumberFormat::Point),
            ("1,2345", NumberFormat::Point),
            ("1 234,5", NumberFormat::Point),
            ("1.234,5", NumberFormat::Point),
        ] {
            assert_eq!(
                read_number(input, format),
                NumberReading::Ambiguous,
                "{input:?} under {format:?}"
            );
            assert_eq!(parse_decimal(input, format), None);
        }
    }

    #[test]
    fn a_leading_zero_group_is_never_a_grouping() {
        // The review's HIGH: « 0,925 » under Point and « 0 925 » were read 925 (× 1000).
        assert_eq!(
            read_number("0,925", NumberFormat::Point),
            NumberReading::Ambiguous
        );
        for format in [NumberFormat::Comma, NumberFormat::Point] {
            assert_eq!(read_number("0 925", format), NumberReading::NotANumber);
            assert_eq!(read_number("0'925", format), NumberReading::NotANumber);
            assert_eq!(read_number("01 234", format), NumberReading::NotANumber);
        }
    }

    #[test]
    fn blank_and_non_numbers_are_told_apart_never_zero() {
        for format in [NumberFormat::Comma, NumberFormat::Point] {
            assert_eq!(read_number("", format), NumberReading::Blank);
            assert_eq!(read_number("  ", format), NumberReading::Blank);
        }
        for (input, format) in [
            ("1,2,3", NumberFormat::Comma),
            ("1.2.3", NumberFormat::Point),
            ("12,34,567", NumberFormat::Point),
            ("1,234 567", NumberFormat::Point),
            ("1'234 567", NumberFormat::Comma),
            ("7 5", NumberFormat::Comma),
            ("1234 567", NumberFormat::Comma),
            ("1 234,5,6", NumberFormat::Comma),
            ("1,234.5,6", NumberFormat::Point),
            (",5", NumberFormat::Comma),
            ("5,", NumberFormat::Comma),
            (".5", NumberFormat::Point),
            ("-", NumberFormat::Comma),
            ("--2", NumberFormat::Point),
            ("12 %", NumberFormat::Comma),
            ("1e5", NumberFormat::Point),
            ("dix", NumberFormat::Comma),
        ] {
            assert_eq!(
                read_number(input, format),
                NumberReading::NotANumber,
                "{input:?} under {format:?}"
            );
        }
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
