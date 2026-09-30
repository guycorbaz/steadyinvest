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

/// The cause line (Decision 4): derived from the stored study — the method, the cells' provenance
/// newer than the freeze, the AI marks. Each cause once, in a fixed order.
pub fn causes(
    study: &Study,
    frozen: &FrozenVerdict,
    current: &FrozenVerdict,
    changed_inputs: &[(String, String, String)],
    day_month: &dyn Fn(&Timestamp) -> String,
) -> String {
    let mut method = None;
    if frozen.method_version != current.method_version {
        method = Some(
            CAUSE_METHOD
                .replace("{from}", &frozen.method_version)
                .replace("{to}", &current.method_version),
        );
    }
    // The latest provider write after the freeze (a refresh), if any.
    let after = |t: &Timestamp| t.0 > frozen.frozen_at.0;
    let latest_refresh = study
        .years
        .iter()
        .flat_map(|y| {
            [
                Some(&y.sales),
                Some(&y.eps),
                Some(&y.high_price),
                Some(&y.low_price),
                y.dividend_per_share.as_ref(),
                y.pre_tax_profit.as_ref(),
                y.book_value_per_share.as_ref(),
            ]
        })
        .flatten()
        .filter(|c| c.provenance.source == Source::Provider && after(&c.provenance.timestamp))
        .map(|c| c.provenance.timestamp.clone())
        .max_by(|a, b| a.0.cmp(&b.0));
    let (mut refresh, mut owner, mut ai, mut unknown) = (false, false, false, false);
    for (key, _, _) in changed_inputs {
        if let Some(rest) = key.strip_prefix('y')
            && let Some((year, field)) = rest.split_once('.')
        {
            let cell = year.parse::<i32>().ok().and_then(|year| {
                study
                    .years
                    .iter()
                    .find(|y| y.year == year)
                    .and_then(|y| match field {
                        "sales" => Some(&y.sales),
                        "eps" => Some(&y.eps),
                        "high_price" => Some(&y.high_price),
                        "low_price" => Some(&y.low_price),
                        "dividend_per_share" => y.dividend_per_share.as_ref(),
                        "pre_tax_profit" => y.pre_tax_profit.as_ref(),
                        "book_value_per_share" => y.book_value_per_share.as_ref(),
                        _ => None,
                    })
            });
            match cell {
                Some(c) if c.provenance.ai_origin.is_some() => ai = true,
                Some(c) if c.provenance.source == Source::Provider => refresh = true,
                Some(_) => owner = true,
                None => unknown = true,
            }
            continue;
        }
        match key.as_str() {
            "j.current_price" | "q.ttm_quarterly_eps" => {
                if latest_refresh.is_some() {
                    refresh = true
                } else {
                    unknown = true
                }
            }
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
    let mut out: Vec<String> = Vec::new();
    out.extend(method);
    if refresh {
        match &latest_refresh {
            Some(t) => out.push(CAUSE_REFRESH.replace("{date}", &day_month(t))),
            None => unknown = true,
        }
    }
    if owner {
        out.push(CAUSE_OWNER.to_string());
    }
    if ai {
        out.push(CAUSE_AI.to_string());
    }
    if unknown || out.is_empty() {
        out.push(CAUSE_UNKNOWN.to_string());
    }
    out.join(", ")
}
