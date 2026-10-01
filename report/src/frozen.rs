//! Story 8.8 — what the frozen verdict and the current one share, for every surface (the study
//! screen, the PDF): whether they differ, which inputs changed, and the cause line (FR29, A13 —
//! derived from the stored study: the method, the cells' provenance newer than the freeze, the AI
//! marks). Pure; the words are app text, posture-scanned by the app's inventory.

use steadyinvest_contract::{FrozenVerdict, Source, Study, Timestamp};

use crate::form::ABSENT;

pub const CAUSE_REFRESH: &str = "rafraîchissement du {date}";
pub const CAUSE_OWNER: &str = "modification de votre part";
pub const CAUSE_AI: &str = "proposition de l'IA validée";
pub const CAUSE_METHOD: &str = "changement de méthode ({from} → {to})";
pub const CAUSE_UNKNOWN: &str = "cause inconnue";
/// G3: the current verdict is no longer full — its open inputs (the spec's « entrées ouvertes »).
pub const CAUSE_OPEN: &str = "entrées ouvertes : {list}";

/// The frozen verdict and the current one (the same shape) agree on every fact, figure, input,
/// hash and method — the freeze time aside.
pub fn same(frozen: &FrozenVerdict, current: &FrozenVerdict) -> bool {
    let mut f = frozen.clone();
    f.frozen_at = current.frozen_at.clone();
    &f == current
}

/// Every input either side has whose value differs — `(key, frozen, current)`, keys ordered; an
/// input missing on a side (a year added since) reads « absent » there.
pub fn changed_inputs(
    frozen: &FrozenVerdict,
    current: &FrozenVerdict,
) -> Vec<(String, String, String)> {
    let keys: std::collections::BTreeSet<&String> =
        frozen.inputs.keys().chain(current.inputs.keys()).collect();
    keys.into_iter()
        .filter_map(|k| {
            let f = frozen.inputs.get(k).map_or(ABSENT, String::as_str);
            let c = current.inputs.get(k).map_or(ABSENT, String::as_str);
            (f != c).then(|| (k.clone(), f.to_string(), c.to_string()))
        })
        .collect()
}

/// The cause line (Decision 4): derived from the stored study — the method, the changed cells' own
/// provenance, the AI marks; a current verdict no longer full names its open inputs
/// (`open_inputs`, G3). Each cause once, in a fixed order.
pub fn causes(
    study: &Study,
    frozen: &FrozenVerdict,
    current: &FrozenVerdict,
    changed_inputs: &[(String, String, String)],
    open_inputs: Option<&str>,
    day_month: &dyn Fn(&Timestamp) -> String,
) -> String {
    let mut out: Vec<String> = Vec::new();
    if frozen.method_version != current.method_version {
        out.push(
            CAUSE_METHOD
                .replace("{from}", &frozen.method_version)
                .replace("{to}", &current.method_version),
        );
    }
    // A provider write at or after the freeze (the freeze and a fetch can share a second).
    let since = |t: &Timestamp| t.0 >= frozen.frozen_at.0;
    let latest = |a: Option<Timestamp>, b: &Timestamp| match a {
        Some(a) if a.0 >= b.0 => Some(a),
        _ => Some(b.clone()),
    };
    let any_refresh = study
        .years
        .iter()
        .flat_map(year_cells)
        .filter(|c| c.provenance.source == Source::Provider && since(&c.provenance.timestamp))
        .map(|c| c.provenance.timestamp.clone())
        .fold(None, |acc, t| latest(acc, &t));
    let (mut refresh, mut owner, mut ai, mut unknown) = (None::<Timestamp>, false, false, false);
    for (key, _, _) in changed_inputs {
        if let Some(rest) = key.strip_prefix('y')
            && let Some((year, field)) = rest.split_once('.')
        {
            let cell = year.parse::<i32>().ok().and_then(|year| {
                study
                    .years
                    .iter()
                    .find(|y| y.year == year)
                    .and_then(|y| year_cell(y, field))
            });
            match cell {
                Some(c) if c.provenance.ai_origin.is_some() => ai = true,
                // The changed cell's OWN refresh date.
                Some(c) if c.provenance.source == Source::Provider => {
                    refresh = latest(refresh, &c.provenance.timestamp)
                }
                Some(_) => owner = true,
                // A year that left the study: the owner removed it (a refresh never does).
                None => owner = true,
            }
            continue;
        }
        match key.as_str() {
            // No provenance on these: a refresh seen since the freeze, else unknown (the
            // holdings price writes the current price too).
            // The current price records its origin (G3 review, 2026-10-01): a price typed since the
            // freeze is the owner's edit, a fetched one the refresh of that day. Without a recorded
            // origin (an older price), the TTM EPS too: a refresh seen since the freeze, else
            // unknown (the holdings price writes the current price as well).
            "j.current_price"
                if study
                    .judgment
                    .current_price_origin
                    .as_ref()
                    .is_some_and(|o| since(&o.at)) =>
            {
                let o = study
                    .judgment
                    .current_price_origin
                    .as_ref()
                    .expect("guarded");
                match o.source {
                    Source::Provider => refresh = latest(refresh, &o.at),
                    _ => owner = true,
                }
            }
            "j.current_price" | "q.ttm_quarterly_eps" => match &any_refresh {
                Some(t) => refresh = latest(refresh, t),
                None => unknown = true,
            },
            k if k.starts_with("q.") => unknown = true,
            k => {
                let field = k.trim_start_matches("j.");
                let placed = steadyinvest_contract::DraftField::from_key(field)
                    .and_then(|f| f.ai_slot(&study.judgment.ai_placed))
                    .is_some_and(|slot| slot.is_some());
                if placed { ai = true } else { owner = true }
            }
        }
    }
    if let Some(t) = &refresh {
        out.push(CAUSE_REFRESH.replace("{date}", &day_month(t)));
    }
    if owner {
        out.push(CAUSE_OWNER.to_string());
    }
    if ai {
        out.push(CAUSE_AI.to_string());
    }
    if let Some(list) = open_inputs {
        out.push(CAUSE_OPEN.replace("{list}", list));
    }
    if unknown || out.is_empty() {
        out.push(CAUSE_UNKNOWN.to_string());
    }
    out.join(", ")
}

/// The frozen verdict and today's differ (G3: ONE rule for the screen and the PDF) — in a value,
/// the hash or the method, or the current verdict is no longer full.
pub fn differs(frozen: &FrozenVerdict, current: &FrozenVerdict, current_full: bool) -> bool {
    !current_full || !same(frozen, current)
}

/// JJ/MM of a stamp — its UTC date, as the history's day headers (G3: one date on every surface).
pub fn day_month(t: &Timestamp) -> String {
    let d: String = t.0.chars().take(10).collect();
    match (d.get(5..7), d.get(8..10)) {
        (Some(m), Some(day)) => format!("{day}/{m}"),
        _ => d,
    }
}

fn year_cells(y: &steadyinvest_contract::YearData) -> Vec<&steadyinvest_contract::Cell> {
    [
        Some(&y.sales),
        Some(&y.eps),
        Some(&y.high_price),
        Some(&y.low_price),
        y.dividend_per_share.as_ref(),
        y.pre_tax_profit.as_ref(),
        y.book_value_per_share.as_ref(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn year_cell<'a>(
    y: &'a steadyinvest_contract::YearData,
    field: &str,
) -> Option<&'a steadyinvest_contract::Cell> {
    match field {
        "sales" => Some(&y.sales),
        "eps" => Some(&y.eps),
        "high_price" => Some(&y.high_price),
        "low_price" => Some(&y.low_price),
        "dividend_per_share" => y.dividend_per_share.as_ref(),
        "pre_tax_profit" => y.pre_tax_profit.as_ref(),
        "book_value_per_share" => y.book_value_per_share.as_ref(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use steadyinvest_contract::{
        AiOrigin, AiPlaced, Cell, Coverage, ForecastLowOption, Freshness, FrozenCriterion,
        FrozenUpsideDownside, Judgment, Money, Provenance, Review, YearData,
    };
    use uuid::Uuid;

    const FROZEN_AT: &str = "2026-09-30T10:00:00Z";

    fn cell(source: Source, at: &str, ai: bool) -> Cell {
        Cell {
            value: Some(Money::from(rust_decimal::Decimal::new(5, 0))),
            source,
            freshness: Freshness::Current,
            review: Review::Validated,
            coverage: Coverage::Present,
            provenance: Provenance {
                source,
                logical_version: 1,
                timestamp: Timestamp(at.to_string()),
                hash_of_dependencies: "h".to_string(),
                ai_origin: ai.then(|| AiOrigin {
                    draft_id: Uuid::from_u128(9),
                    client: "c".to_string(),
                    model: "m".to_string(),
                    validated_at: Timestamp(at.to_string()),
                }),
            },
            pending: None,
        }
    }

    /// 2024: sales refreshed on 02/10 (provider), eps an AI value, high price the owner's.
    fn study() -> Study {
        let judgment = Judgment {
            estimated_high_eps: None,
            estimated_low_eps: None,
            projected_sales_growth_pct: None,
            projected_eps_growth_pct: None,
            judged_avg_high_pe: None,
            judged_avg_low_pe: None,
            forecast_low_option: ForecastLowOption::AvgLowPeTimesEps,
            recent_severe_low: None,
            current_price: None,
            present_full_year_dividend: None,
            ttm_eps: None,
            ai_placed: AiPlaced::default(),
            current_price_origin: None,
        };
        let mut s = Study::new(
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            "NESN",
            "CHF",
            judgment,
            Timestamp("2026-09-01T00:00:00Z".to_string()),
        );
        s.years.push(YearData {
            year: 2024,
            sales: cell(Source::Provider, "2026-10-02T08:00:00Z", false),
            eps: cell(Source::Manual, "2026-10-01T08:00:00Z", true),
            high_price: cell(Source::Manual, "2026-10-01T08:00:00Z", false),
            low_price: cell(Source::Provider, "2026-09-01T08:00:00Z", false),
            dividend_per_share: None,
            pre_tax_profit: None,
            book_value_per_share: None,
        });
        s
    }

    fn verdict(method: &str) -> FrozenVerdict {
        FrozenVerdict {
            frozen_at: Timestamp(FROZEN_AT.to_string()),
            method_version: method.to_string(),
            inputs_hash: "h".to_string(),
            quality_value_candidate: false,
            present_zone: None,
            ud_at_or_above_target: FrozenCriterion::Unmet,
            relative_value_below_ceiling: FrozenCriterion::Unmet,
            present_price_in_low_zone: FrozenCriterion::Unmet,
            appreciation_at_or_above_double: FrozenCriterion::Unmet,
            upside_downside: FrozenUpsideDownside::Unknown,
            relative_value_pct: None,
            projected_appreciation_pct: None,
            total_return_pct: None,
            appreciation_only_pct: None,
            zones: None,
            inputs: BTreeMap::new(),
        }
    }

    fn cause(keys: &[&str], study: &Study, method: &str, open: Option<&str>) -> String {
        let changed: Vec<(String, String, String)> = keys
            .iter()
            .map(|k| (k.to_string(), "1".to_string(), "2".to_string()))
            .collect();
        causes(
            study,
            &verdict("ssg-1.2.0"),
            &verdict(method),
            &changed,
            open,
            &day_month,
        )
    }

    #[test]
    fn each_changed_input_names_its_own_cause_once_in_order() {
        let s = study();
        assert_eq!(
            cause(&["y2024.sales"], &s, "ssg-1.2.0", None),
            "rafraîchissement du 02/10"
        );
        assert_eq!(
            cause(&["y2024.eps"], &s, "ssg-1.2.0", None),
            "proposition de l'IA validée"
        );
        assert_eq!(
            cause(&["y2024.high_price"], &s, "ssg-1.2.0", None),
            "modification de votre part"
        );
        assert_eq!(
            cause(&["y2019.eps"], &s, "ssg-1.2.0", None),
            "modification de votre part",
            "a year the owner removed"
        );
        assert_eq!(
            cause(
                &[
                    "y2024.sales",
                    "y2024.high_price",
                    "j.judged_avg_high_pe",
                    "y2024.eps"
                ],
                &s,
                "ssg-1.3.0",
                None
            ),
            "changement de méthode (ssg-1.2.0 → ssg-1.3.0), rafraîchissement du 02/10, \
             modification de votre part, proposition de l'IA validée"
        );
    }

    #[test]
    fn the_current_price_follows_a_refresh_seen_since_the_freeze_else_unknown() {
        let s = study();
        assert_eq!(
            cause(&["j.current_price"], &s, "ssg-1.2.0", None),
            "rafraîchissement du 02/10"
        );
        let mut no_refresh = s.clone();
        no_refresh.years[0].sales = cell(Source::Provider, "2026-09-01T08:00:00Z", false);
        assert_eq!(
            cause(&["j.current_price"], &no_refresh, "ssg-1.2.0", None),
            "cause inconnue"
        );
        // A refresh in the freeze's own second counts (the clock is to the second).
        no_refresh.years[0].sales = cell(Source::Provider, FROZEN_AT, false);
        assert_eq!(
            cause(&["q.ttm_quarterly_eps"], &no_refresh, "ssg-1.2.0", None),
            "rafraîchissement du 30/09"
        );
    }

    #[test]
    fn a_verdict_no_longer_full_names_its_open_inputs_and_differs() {
        let s = study();
        assert_eq!(
            cause(&[], &s, "ssg-1.2.0", Some("BPA 2024 — non validé")),
            "entrées ouvertes : BPA 2024 — non validé"
        );
        assert_eq!(cause(&[], &s, "ssg-1.2.0", None), "cause inconnue");
        let v = verdict("ssg-1.2.0");
        assert!(!differs(&v, &v, true));
        assert!(
            differs(&v, &v, false),
            "same figures, no longer full: differs"
        );
        assert_eq!(
            day_month(&Timestamp("2026-09-30T23:30:00Z".to_string())),
            "30/09"
        );
    }
}
