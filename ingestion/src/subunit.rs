//! Listings quoted in a currency's hundredths (owner decision, Guy 2026-10-01: « les quotations
//! doivent être de vraies valeurs ») — London pence (`GBX`, `GBp`), Johannesburg cents (`ZAc`),
//! Tel Aviv agorot (`ILA`). Their PRICES are converted into the major currency (÷ 100, exact
//! decimal) at ONE place, right after the provider answered and before `normalize` or the app
//! read anything: [`crate::fetch_canonical`] and [`crate::fetch_price`] call this module, nothing
//! else divides.
//!
//! What is a price here, and what is not (EODHD, the only fundamentals provider):
//! - `/eod` bars (each fiscal year's high / low, the latest close) are quoted in the listing's
//!   unit — `General.CurrencyCode` = `GBX` for most London lines: converted.
//! - the statements and `Highlights` are fundamentals, served in the major unit (EODHD's market
//!   capitalisation of a `GBX` line is in pounds; each statement row states its own
//!   `currency_symbol`, read by the adapter): never divided. The per-share EPS, dividend and book
//!   value are derived from the statements, so they are in the statements' currency already.
//!   A statement amount that states no currency of its own carries the listing code as its label
//!   (the adapter's fallback): it is RELABELLED to the major currency, never divided — a figure
//!   served in the major unit under the listing's label (PR #291 review). Left as `GBX`, it would
//!   raise a spurious `currency_mismatch` and « comptes publiés en GBX, cotation en GBP ».
//!   Verified 2026-10-03 on ULVR.LSE (the one real fetch the owner authorised): `GBX` listing,
//!   close 4483.5 pence; `Highlights.EarningsShare` 2.18 in GBP (the served P/E 20.5665 =
//!   44.835 ÷ 2.18); statements in EUR, their own `currency_symbol` — a third currency, left as
//!   served, which takes the mixed-currency warning. Pinned on the trimmed extract
//!   `tests/fixtures/eodhd-*-ULVR-real.json` (`adapters::eodhd` tests).
//! - Twelve Data (`GBp`) serves prices only (no fundamentals on the free tier): converted.

use rust_decimal::Decimal;
use steadyinvest_core::normalize::{RawAmount, RawFinancials};

/// The provider's hundredths listing codes and their major currency — exact codes: the case
/// carries the meaning (`GBp` is pence, `GBP` pounds; a lower-case `gbx` is no known code and is
/// left as it is, the app then refuses it as another currency).
const SUBUNITS: [(&str, &str); 4] = [
    ("GBX", "GBP"),
    ("GBp", "GBP"),
    ("ZAc", "ZAR"),
    ("ILA", "ILS"),
];

/// PURE: the major currency of a hundredths listing code (`GBX` → `GBP`), `None` for any other
/// code. Surrounding blanks are ignored, the case is not.
pub fn subunit_major(code: &str) -> Option<&'static str> {
    let code = code.trim();
    SUBUNITS
        .iter()
        .find(|(subunit, _)| *subunit == code)
        .map(|(_, major)| *major)
}

/// PURE: the provider quotes the symbol in a currency's hundredths (`GBX`, `GBp`, `ZAc`, `ILA`).
pub fn is_subunit_listing(code: &str) -> bool {
    subunit_major(code).is_some()
}

/// PURE: a price in hundredths → the same price in the major unit, exactly (÷ 100 moves the
/// decimal point; `None` only past `Decimal`'s 28-digit scale — absent, never rounded in silence).
pub fn from_hundredths(price: Decimal) -> Option<Decimal> {
    let mut major = price;
    major
        .set_scale(price.scale().checked_add(2)?)
        .ok()
        .map(|()| major)
}

/// PURE: converts the PRICE figures of a fetch quoted in hundredths — each year's high and low
/// (those labelled with the listing code) and the latest close — and relabels the listing in the
/// major currency. The statement amounts labelled with the listing code (a row that stated no
/// currency of its own) are relabelled to the major currency WITHOUT dividing — fundamentals are
/// served in the major unit. Returns the provider's original listing code when a conversion happened (the
/// record the app keeps for the traceability), `None` otherwise (nothing touched).
///
/// A price that cannot be stated in the major unit (`from_hundredths` → `None`) becomes absent.
pub(crate) fn convert_prices(
    financials: &mut RawFinancials,
    latest_price: &mut Option<Decimal>,
) -> Option<String> {
    let code = financials.native_currency.trim().to_string();
    let major = subunit_major(&code)?;
    let convert = |amount: &mut Option<RawAmount>| {
        if amount.as_ref().is_some_and(|a| a.currency.trim() == code) {
            *amount = amount.take().and_then(|a| {
                from_hundredths(a.value).map(|value| RawAmount {
                    value,
                    currency: major.to_string(),
                })
            });
        }
    };
    let relabel = |amount: &mut Option<RawAmount>| {
        if let Some(a) = amount.as_mut()
            && a.currency.trim() == code
        {
            a.currency = major.to_string();
        }
    };
    for year in &mut financials.years {
        convert(&mut year.high_price);
        convert(&mut year.low_price);
        for statement in [
            &mut year.sales,
            &mut year.eps,
            &mut year.dividend_per_share,
            &mut year.pre_tax_profit,
            &mut year.net_profit,
            &mut year.book_value_per_share,
        ] {
            relabel(statement);
        }
    }
    *latest_price = latest_price.and_then(from_hundredths);
    financials.native_currency = major.to_string();
    Some(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use steadyinvest_core::normalize::RawYear;

    fn d(s: &str) -> Decimal {
        Decimal::from_str_exact(s).unwrap()
    }

    fn amount(value: &str, currency: &str) -> Option<RawAmount> {
        Some(RawAmount {
            value: d(value),
            currency: currency.to_string(),
        })
    }

    /// A London-style fetch: prices in the listing code, statements in their own currency.
    fn fetch_in(code: &str) -> RawFinancials {
        RawFinancials {
            native_currency: code.to_string(),
            years: vec![RawYear {
                sales: amount("5000000000", "GBP"),
                eps: amount("0.4512", "GBP"),
                dividend_per_share: amount("0.15", "GBP"),
                book_value_per_share: amount("3.2", code),
                high_price: amount("1234.5", code),
                low_price: amount("987", code),
                ..RawYear::empty(2024)
            }],
            splits: vec![],
        }
    }

    #[test]
    fn each_hundredths_code_names_its_major_currency() {
        for (code, major) in [
            ("GBX", "GBP"),
            ("GBp", "GBP"),
            ("ZAc", "ZAR"),
            ("ILA", "ILS"),
            (" GBX ", "GBP"),
        ] {
            assert_eq!(subunit_major(code), Some(major), "{code:?}");
            assert!(is_subunit_listing(code), "{code:?}");
        }
        // The case carries the meaning: GBP is pounds; `gbx`, `zac` are no known code.
        for code in ["GBP", "gbx", "gbp", "zac", "ila", "ZAR", "ILS", "USD", ""] {
            assert_eq!(subunit_major(code), None, "{code:?}");
            assert!(!is_subunit_listing(code), "{code:?}");
        }
    }

    #[test]
    fn hundredths_become_the_major_unit_exactly() {
        assert_eq!(from_hundredths(d("1234.5")), Some(d("12.345")));
        assert_eq!(from_hundredths(d("987")), Some(d("9.87")));
        assert_eq!(from_hundredths(d("0.0001")), Some(d("0.000001")));
        assert_eq!(from_hundredths(d("-50")), Some(d("-0.5")));
        // Past the 28-digit scale: absent, never a rounded figure.
        let finest = Decimal::new(1, 27);
        assert_eq!(from_hundredths(finest), None);
    }

    #[test]
    fn a_hundredths_listing_has_its_prices_converted_and_its_statements_untouched() {
        for (code, major) in [
            ("GBX", "GBP"),
            ("GBp", "GBP"),
            ("ZAc", "ZAR"),
            ("ILA", "ILS"),
        ] {
            let mut fin = fetch_in(code);
            let mut latest = Some(d("1250.5"));
            assert_eq!(
                convert_prices(&mut fin, &mut latest).as_deref(),
                Some(code),
                "the original listing code is the record"
            );
            assert_eq!(fin.native_currency, major);
            assert_eq!(latest, Some(d("12.505")));
            let y = &fin.years[0];
            assert_eq!(y.high_price, amount("12.345", major), "{code}");
            assert_eq!(y.low_price, amount("9.87", major), "{code}");
            // Fundamentals are never divided: the statements' per-share figures stay as served.
            assert_eq!(y.eps, amount("0.4512", "GBP"));
            assert_eq!(y.dividend_per_share, amount("0.15", "GBP"));
            assert_eq!(y.sales, amount("5000000000", "GBP"));
            // A statement amount that stated no currency of its own (labelled with the listing
            // code) is relabelled to the major currency, never divided (PR #291 review).
            assert_eq!(y.book_value_per_share, amount("3.2", major), "{code}");
        }
    }

    #[test]
    fn any_other_listing_is_left_exactly_as_served() {
        for code in ["GBP", "gbx", "USD", "CHF", ""] {
            let before = fetch_in(code);
            let mut fin = before.clone();
            let mut latest = Some(d("1250.5"));
            assert_eq!(convert_prices(&mut fin, &mut latest), None, "{code:?}");
            assert_eq!(fin, before, "{code:?}");
            assert_eq!(latest, Some(d("1250.5")));
        }
    }

    #[test]
    fn a_price_labelled_otherwise_or_absent_is_not_touched() {
        let mut fin = fetch_in("GBX");
        fin.years[0].high_price = amount("10", "USD");
        fin.years[0].low_price = None;
        let mut latest = None;
        assert!(convert_prices(&mut fin, &mut latest).is_some());
        assert_eq!(fin.years[0].high_price, amount("10", "USD"));
        assert_eq!(fin.years[0].low_price, None);
        assert_eq!(latest, None, "absent stays absent, never zero");
    }
}
