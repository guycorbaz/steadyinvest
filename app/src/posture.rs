//! Neutral-voice / banned-verb posture gate (FR13, spec §6) — the single audit point.
//!
//! Story 2.14 consolidates the per-surface gates built incrementally across 2.1–2.13 into one
//! auditable place. A reviewer confirms FR13 coverage from this file alone.
//!
//! ## Canonical source (one truth)
//! Every scan here uses `core::method::{BANNED_VERBS_EN, BANNED_VERBS_FR}` — **reused, never
//! re-declared** in this crate. There is exactly one verb list the app scans against. The
//! `persistence` crate keeps an intentional crate-local *copy* (it must not depend on `core` —
//! dependency-graph boundary); that copy is verified *indirectly* here by scanning the rendered
//! `persistence::Error` Display strings against the canonical list (see the umbrella scan), so a
//! drift in persistence's copy cannot let a banned verb reach a UI banner unseen.
//!
//! ## Scanned surfaces (the union — `all_user_facing_app_strings_are_neutral`)
//! - **`@tr()` literals** in every `ui/**/*.slint` (read from the files at test time).
//! - **`state::USER_FACING_MESSAGES`** — Rust-side notices that never pass through `@tr()`.
//! - **`viewmodel::engine::USER_FACING_LABELS`** — dynamically-built verdict/zone/trace labels.
//! - **`labels::LABELS`** — the NAIC↔neutral runtime label table (both sets).
//! - **`entry::source_label`** outputs — the provenance words ("manuel"/"fournisseur"/"calculé")
//!   interpolated into `@tr("Source : {}", …)` as a dynamic value the template scan can't see.
//! - **`viewmodel::verify::USER_FACING_TEMPLATES`** — the verify-panel / demo-notice prose built
//!   with `format!` (the `{…}` values are data / core-gated `GoldenDeviation` strings).
//! - **rendered `persistence::Error` Display strings** (incl. the `Sqlite` static own-prefix) —
//!   the app interpolates these verbatim into the `MSG_SAVE_FAILED` banner; scanned against the
//!   canonical list so persistence's crate-local verb copy can't drift unseen.
//!
//! ## No bare literal escapes the gate (`no_bare_user_facing_literal_bypasses_tr`)
//! The `@tr()` scan only sees strings *inside* `@tr(...)`. A bare `Text { text: "Achetez"; }` —
//! or a banned verb in a **ternary branch** (`cond ? @tr("…") : "Achetez"`) or a **concatenation**
//! (`base + "Achetez"`) — would render to the user yet bypass the `@tr` scan. The leak gate scans
//! the whole right-hand side of every user-facing text property and flags every string literal
//! that is neither an `@tr(...)` argument nor an `==`/`!=` state-key comparison operand (method
//! keys like `zone == "buy"`, compared not displayed). Each surviving literal must be an
//! `@tr(...)` call or an allow-listed non-prose glyph/separator — so the union scan sees 100% of
//! rendered prose.
//!
//! ## Exemption
//! Zone-label nouns (the Buy/Neutral/Sell price-band names) are nouns naming the defined bands,
//! not imperatives, so they pass the whole-word scan by construction (spec §6).
//!
//! ## Never scanned (user free-text, not system signals — FR13 scope)
//! Tickers, the decision-rationale note, data-cell values (Money), and the bundled fixture / demo
//! study data are user/sample data, not system-generated signals: they are never wrapped in
//! `@tr()` nor registered in any inventory, so no scan ever encounters them.

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use steadyinvest_core::method::{BANNED_VERBS_EN, BANNED_VERBS_FR};

    /// Same whole-word matcher as `core::golden` (1.9) and `persistence::error` (1.10): any
    /// non-alphanumeric char is a word boundary, match is case-insensitive.
    fn contains_word(haystack: &str, needle: &str) -> bool {
        let h = haystack.to_lowercase();
        let n = needle.to_lowercase();
        h.match_indices(&n).any(|(i, _)| {
            let before_ok = i == 0
                || !h[..i]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric());
            let after = i + n.len();
            let after_ok = after == h.len()
                || !h[after..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphanumeric());
            before_ok && after_ok
        })
    }

    fn assert_neutral(text: &str, origin: &str) {
        for banned in BANNED_VERBS_EN.iter().chain(BANNED_VERBS_FR.iter()) {
            assert!(
                !contains_word(text, banned),
                "{origin}: user-visible string {text:?} contains banned verb {banned:?} (FR13)"
            );
        }
    }

    /// All `.slint` files under `ui/`, recursively.
    fn slint_files() -> Vec<PathBuf> {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).expect("ui/ readable") {
                let path = entry.expect("dir entry").path();
                if path.is_dir() {
                    walk(&path, out);
                } else if path.extension().is_some_and(|ext| ext == "slint") {
                    out.push(path);
                }
            }
        }
        let mut files = Vec::new();
        walk(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("ui"),
            &mut files,
        );
        files.sort();
        files
    }

    /// Every string literal inside every `@tr(...)` occurrence (covers the message, a `ctx`
    /// disambiguator and plural variants alike — scanning more than required is fine, the gate
    /// only gets stricter).
    fn tr_literals(source: &str) -> Vec<String> {
        let mut literals = Vec::new();
        let mut rest = source;
        while let Some(at) = rest.find("@tr(") {
            rest = &rest[at + 4..];
            let mut depth = 1usize;
            let mut chars = rest.char_indices();
            let mut current: Option<String> = None;
            let mut consumed = 0;
            while let Some((i, c)) = chars.next() {
                consumed = i + c.len_utf8();
                match current.as_mut() {
                    Some(literal) => match c {
                        '\\' => {
                            if let Some((j, escaped)) = chars.next() {
                                consumed = j + escaped.len_utf8();
                                literal.push(escaped);
                            }
                        }
                        '"' => literals.push(current.take().expect("in literal")),
                        _ => literal.push(c),
                    },
                    None => match c {
                        '"' => current = Some(String::new()),
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    },
                }
            }
            rest = &rest[consumed.min(rest.len())..];
        }
        literals
    }

    /// User-facing text properties whose value must go through `@tr()` (or a binding) so the
    /// `@tr` scan can see the rendered prose — never a bare string literal (FR13, AC2).
    const USER_FACING_SLINT_PROPS: &[&str] = &[
        "text",
        "title",
        "placeholder-text",
        "accessible-label",
        "accessible-description",
        "accessible-value",
        "accessible-placeholder-text",
        "accessible-action-default",
    ];

    /// The only bare string literals permitted in a user-facing property: single-glyph visual
    /// markers that ARE the legend's own tokens (not prose), and the empty default. Each is a
    /// symbol, carries no banned verb, and is meaningless to translate. A NEW bare literal that is
    /// not here fails the leak gate — forcing prose through `@tr()` where the union scan sees it.
    // Comments sit on their own line (not trailing) so the multi-byte glyphs can't make
    // comment-alignment formatting version-dependent.
    const BARE_LITERAL_ALLOW: &[&str] = &[
        // empty default — renders nothing
        "",
        // editable_cell: the not-available marker
        "⦸",
        // growth_chart: the drag-handle glyph
        "⇕",
        // zone_bar: provisional-verdict hatching
        "╱╱╱╱",
        // verdict_badge: provisional-verdict hatching
        "╱╱╱╱╱╱",
        // trust_markers: the "to review" tag
        "?",
        // trust_markers: the "validated" tag
        "✓",
        // collapsible_section: the folded / unfolded chevrons
        "▾",
        "▸",
        // settings: the plausibility-warning glyph (mirrors the cell marker)
        "△",
        // zone_bar / verdict: the em-dash "no value" marker
        "—",
        // single-space and punctuation-only separators (no prose, no verb)
        " ",
        " · ",
        " — ",
    ];

    /// Every bare (non-`@tr`) string literal in a user-facing text property's value — including
    /// literals nested in a ternary branch (`cond ? @tr("a") : "Achetez"`) or a concatenation
    /// (`base + " — "`), not just a value that *starts* with `"`. A first-char-only check would let
    /// a banned verb in a ternary else-branch reach the UI unscanned; this scans the whole RHS up
    /// to the statement `;`, returning every literal that is NOT an `@tr(...)` argument. Returns
    /// `(prop, literal)` pairs, whole-token matched on the property name so `placeholder-text` is
    /// not mistaken for `text`.
    fn bare_user_facing_literals(source: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for prop in USER_FACING_SLINT_PROPS {
            let mut from = 0;
            while let Some(rel) = source[from..].find(prop) {
                let start = from + rel;
                let end = start + prop.len();
                from = end;
                // Whole-token boundary on the LEFT: a `-`/`_`/alphanumeric before means this is a
                // longer identifier (e.g. the `text` inside `placeholder-text`), skip it.
                let left_ok = start == 0
                    || !source[..start]
                        .chars()
                        .next_back()
                        .is_some_and(|c| c.is_alphanumeric() || c == '-' || c == '_');
                if !left_ok {
                    continue;
                }
                // The property token must be followed (after optional ws) by a single `:` (a
                // binding) — not `::` (a path/enum like `Foo::Bar`).
                let after_prop = source[end..].trim_start();
                let Some(after_colon) = after_prop.strip_prefix(':') else {
                    continue;
                };
                if after_colon.starts_with(':') {
                    continue;
                }
                for lit in rhs_non_tr_literals(after_colon) {
                    out.push(((*prop).to_string(), lit));
                }
            }
        }
        out
    }

    /// Scan a property's right-hand side (text after the `:`) up to the statement-terminating `;`
    /// at paren depth 0, returning every string literal that is RENDERED to the user — i.e. not an
    /// argument of an `@tr(...)` call (those are translated and the `@tr` scan already covers them)
    /// and not an operand of an `==`/`!=` comparison (those are method state-keys like `zone ==
    /// "buy"`, compared not displayed — the rendered value is the ternary's result branch). Every
    /// other `"…"` is a bare literal that would reach the user unscanned.
    fn rhs_non_tr_literals(rhs: &str) -> Vec<String> {
        let chars: Vec<char> = rhs.chars().collect();
        let mut out = Vec::new();
        let mut i = 0;
        let mut paren_depth: usize = 0;
        // Paren depths at which an `@tr(` is currently open; a string is translated iff non-empty.
        let mut tr_stack: Vec<usize> = Vec::new();
        // True iff the nearest non-whitespace token ending at `idx` is `==` or `!=`.
        let comparison_before = |idx: usize| -> bool {
            let mut k = idx;
            while k > 0 && chars[k - 1].is_whitespace() {
                k -= 1;
            }
            k >= 2 && chars[k - 1] == '=' && (chars[k - 2] == '=' || chars[k - 2] == '!')
        };
        // True iff the nearest non-whitespace token starting at `idx` is `==` or `!=`.
        let comparison_after = |idx: usize| -> bool {
            let mut k = idx;
            while k < chars.len() && chars[k].is_whitespace() {
                k += 1;
            }
            k + 1 < chars.len() && chars[k + 1] == '=' && (chars[k] == '=' || chars[k] == '!')
        };
        while i < chars.len() {
            match chars[i] {
                ';' if paren_depth == 0 => break,
                '"' => {
                    let open = i;
                    let mut lit = String::new();
                    i += 1;
                    while i < chars.len() {
                        match chars[i] {
                            '\\' => {
                                i += 1;
                                if i < chars.len() {
                                    lit.push(chars[i]);
                                }
                            }
                            '"' => break,
                            ch => lit.push(ch),
                        }
                        i += 1;
                    }
                    let is_comparison = comparison_before(open) || comparison_after(i + 1);
                    if tr_stack.is_empty() && !is_comparison {
                        out.push(lit);
                    }
                }
                '@' if chars[i..].starts_with(&['@', 't', 'r']) => {
                    // `@tr` followed (after optional ws) by `(` opens a translated region.
                    let mut j = i + 3;
                    while j < chars.len() && chars[j].is_whitespace() {
                        j += 1;
                    }
                    if j < chars.len() && chars[j] == '(' {
                        paren_depth += 1;
                        tr_stack.push(paren_depth);
                        i = j + 1;
                        continue;
                    }
                }
                '(' => paren_depth += 1,
                ')' => {
                    if tr_stack.last() == Some(&paren_depth) {
                        tr_stack.pop();
                    }
                    paren_depth = paren_depth.saturating_sub(1);
                }
                _ => {}
            }
            i += 1;
        }
        out
    }

    /// The user-facing provenance display strings rendered by `entry::source_label` (shown via
    /// `@tr("Source : {}", …)`): the interpolated value is a dynamic Rust string, so the `@tr`
    /// template scan never sees it. Scanned here over every `Source` variant so a future banned
    /// verb in a provenance label is caught (only `"manuel"` was otherwise registered, as
    /// `PROVENANCE_MANUAL`).
    fn provenance_display_labels() -> Vec<&'static str> {
        use steadyinvest_contract::{Cell, Coverage, Provenance, Source, Timestamp};
        [Source::Manual, Source::Provider, Source::Derived]
            .into_iter()
            .map(|source| {
                let provenance = Provenance {
                    source,
                    logical_version: 1,
                    timestamp: Timestamp("2026-01-01T00:00:00Z".to_string()),
                    hash_of_dependencies: "posture".to_string(),
                };
                // A present cell with this source — `source_label` reveals a label only when
                // present. Reuse the real `tofill_cell` skeleton, flip coverage to Present.
                let cell = Cell {
                    coverage: Coverage::Present,
                    ..crate::viewmodel::entry::tofill_cell(provenance)
                };
                crate::viewmodel::entry::source_label(Some(&cell))
            })
            .collect()
    }

    /// Representative `persistence::Error` instances. Covers every variant whose own prose the app
    /// could surface in a banner (the app interpolates `persistence::Error` Display verbatim into
    /// the `MSG_SAVE_FAILED` catch-all). `Sqlite(rusqlite::Error)` cannot be constructed here
    /// (`app` has no `rusqlite` dep), so its static own-prefix is scanned as a literal instead; the
    /// variable tail is third-party `rusqlite` text, outside our signal.
    fn sample_persistence_error_messages() -> Vec<String> {
        use std::path::PathBuf;
        use steadyinvest_persistence::Error;
        use uuid::Uuid;
        vec![
            // `Error::Sqlite` static own-prefix (the variant needs `rusqlite` to construct).
            "sqlite operation failed:".to_string(),
            Error::JournalExists(PathBuf::from("/tmp/example.journal")).to_string(),
            Error::CorruptPayload {
                detail: "example".into(),
            }
            .to_string(),
            Error::CorruptJournalMeta {
                detail: "example".into(),
            }
            .to_string(),
            Error::NewerJournalSchema {
                file_user_version: 9,
                supported: 1,
            }
            .to_string(),
            Error::NewerRowSchema {
                row_schema_version: 9,
                supported: 1,
            }
            .to_string(),
            Error::JournalIdentityMismatch {
                study_journal_id: Uuid::nil(),
                journal_id: Uuid::nil(),
            }
            .to_string(),
            Error::Migration {
                version: 2,
                source: Box::new(Error::CorruptPayload {
                    detail: "example".into(),
                }),
            }
            .to_string(),
        ]
    }

    #[test]
    fn no_bare_user_facing_literal_bypasses_tr() {
        let mut offenders = Vec::new();
        for file in slint_files() {
            let source = std::fs::read_to_string(&file).expect("slint file readable");
            for (prop, lit) in bare_user_facing_literals(&source) {
                if !BARE_LITERAL_ALLOW.contains(&lit.as_str()) {
                    offenders.push(format!("{}: {prop}: {lit:?}", file.display()));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "user-facing text must go through @tr() so the posture scan sees it (FR13); \
             bare prose literal(s): {offenders:?}"
        );
    }

    #[test]
    fn all_user_facing_app_strings_are_neutral() {
        // The consolidated FR13 proof: scan the UNION of every user-facing string the app can
        // render, against the canonical core list. The per-surface tests below keep their precise
        // floors + failure locality; this one is the single completeness gate.
        let mut union: Vec<(String, String)> = Vec::new();
        for file in slint_files() {
            let source = std::fs::read_to_string(&file).expect("slint file readable");
            for literal in tr_literals(&source) {
                union.push((file.display().to_string(), literal));
            }
        }
        for message in crate::state::USER_FACING_MESSAGES {
            union.push(("state.rs".into(), (*message).to_string()));
        }
        for label in crate::viewmodel::engine::USER_FACING_LABELS {
            union.push(("engine.rs".into(), (*label).to_string()));
        }
        for entry in &crate::labels::LABELS {
            union.push(("labels.rs (naic)".into(), entry.naic.to_string()));
            union.push(("labels.rs (neutral)".into(), entry.neutral.to_string()));
        }
        for label in provenance_display_labels() {
            union.push(("entry::source_label".into(), label.to_string()));
        }
        for template in crate::viewmodel::verify::USER_FACING_TEMPLATES {
            union.push(("verify.rs".into(), (*template).to_string()));
        }
        for message in sample_persistence_error_messages() {
            union.push(("persistence::Error".into(), message));
        }
        assert!(
            !union.is_empty(),
            "union posture scan collected zero strings — scan broken?"
        );
        for (origin, text) in &union {
            assert_neutral(text, origin);
        }
    }

    /// Non-exhaustive advice-phrasing heuristic (AC3). The hard FR13 contract is the banned-verb
    /// gate above; this catches *advice phrasing* that is not a single banned verb (a soft "you
    /// should …" / "pensez à …"). Signals must state facts, never direct the user. Kept short and
    /// documented — not a natural-language grader.
    #[test]
    fn user_facing_strings_state_facts_not_advice() {
        const ADVICE_SCAFFOLDS: &[&str] = &[
            "pensez à",
            "n'oubliez",
            "veuillez",
            "vous devriez",
            "devriez",
            "make sure",
            "be sure to",
            "remember to",
            "don't forget",
            "you should",
            "assurez-vous",
        ];
        let mut strings: Vec<(String, String)> = Vec::new();
        for file in slint_files() {
            let source = std::fs::read_to_string(&file).expect("slint file readable");
            for literal in tr_literals(&source) {
                strings.push((file.display().to_string(), literal));
            }
        }
        for message in crate::state::USER_FACING_MESSAGES {
            strings.push(("state.rs".into(), (*message).to_string()));
        }
        for label in crate::viewmodel::engine::USER_FACING_LABELS {
            strings.push(("engine.rs".into(), (*label).to_string()));
        }
        for (origin, text) in &strings {
            let lower = text.to_lowercase();
            for scaffold in ADVICE_SCAFFOLDS {
                assert!(
                    !lower.contains(scaffold),
                    "{origin}: {text:?} reads as advice ({scaffold:?}); state a neutral fact (FR13)"
                );
            }
        }
    }

    #[test]
    fn bare_literal_detector_distinguishes_tr_bindings_and_bare_prose() {
        let source = r#"
            Text { text: "Achetez maintenant"; }
            Text { text: @tr("Prix actuel"); }
            Text { text: root.dynamic-value; }
            Text { placeholder-text: "garder"; }
            Text { accessible-label: ""; }
        "#;
        let bare = bare_user_facing_literals(source);
        // @tr() and binding values are NOT bare; the two quoted literals + the empty one are.
        assert_eq!(
            bare,
            vec![
                ("text".to_string(), "Achetez maintenant".to_string()),
                ("placeholder-text".to_string(), "garder".to_string()),
                ("accessible-label".to_string(), String::new()),
            ]
        );
    }

    #[test]
    fn bare_literal_detector_sees_ternary_branches_and_skips_state_key_comparisons() {
        // A banned verb in a ternary ELSE branch must be caught (the real FR13 hole): a first-char
        // check would miss it because the value starts with an identifier.
        let ternary = r#"Text { text: cond ? @tr("Replié") : "Achetez maintenant"; }"#;
        assert_eq!(
            bare_user_facing_literals(ternary),
            vec![("text".to_string(), "Achetez maintenant".to_string())]
        );
        // State-key comparisons (`zone == "buy"`) are operands, not rendered text — the rendered
        // value is the result branch (a scanned label binding), so the key must NOT be flagged.
        let state_key = r#"text: root.zone == "buy" ? Labels.zone-buy : Labels.zone-sell;"#;
        assert!(bare_user_facing_literals(state_key).is_empty());
        // A rendered concatenation scaffold IS caught (then allow-listed if non-prose).
        let concat = r#"text: root.id + " — " + root.detail;"#;
        assert_eq!(
            bare_user_facing_literals(concat),
            vec![("text".to_string(), " — ".to_string())]
        );
    }

    #[test]
    fn ui_tr_strings_are_neutral_no_banned_verb() {
        let files = slint_files();
        assert!(
            files.len() >= 21,
            "posture gate found only {} .slint files — scan broken?",
            files.len()
        );
        let mut total = 0;
        for file in files {
            let source = std::fs::read_to_string(&file).expect("slint file readable");
            for literal in tr_literals(&source) {
                assert_neutral(&literal, &file.display().to_string());
                total += 1;
            }
        }
        // Story 2.3 added the whole faithful §1–§5 form's labels; Story 2.4 adds the editable-cell
        // component (the not-available "n/a" marker), the §2 raw-input row labels (Ventes / Bénéfice
        // avant impôt / Valeur comptable), the source-on-demand caption and the entry-gesture hint,
        // so the scanned population grew again. Keep the floor strict so a future scan that silently
        // stops finding literals (a broken extractor) fails loudly.
        // Story 2.6 added the judgment-input labels, the §4 forecast-low selector chips, the zone-bar
        // empty-state + present-price captions, the verdict-bar facts, and the traceability surface
        // labels. Story 2.8 adds the §1 growth-chart component (its empty-state caption + the draggable
        // line's accessible label). Story 2.9 adds the undo/redo controls + the scenario-compare
        // overlay (its column labels, the "alternate" caption + the confidence words) — so the
        // scanned population grew again. Story 2.10 adds the decision-rationale note's label +
        // placeholder (the user's typed rationale itself is NEVER scanned — it's user data, FR13).
        // Story 2.11 adds the "+ année" extend-projection affordance label (the annual roll-forward).
        // Story 2.12 adds the dashboard search/sort/filter controls + the per-row archive/réactiver/
        // supprimer actions + the delete-confirm banner labels. Story 2.13 adds the Réglages help hub
        // (legend marker meanings + glossary terms/definitions + verify-engine controls/results), the
        // actionable empty state + demo CTA, and the read-only demo banner; tickers/search text and the
        // fixture data are never scanned. Story 3.2 adds the Réglages provider/key panel (provider chips,
        // key status + placeholder, save/delete/test action labels); the API key value is user data,
        // NEVER scanned (NFR-S1). Story 3.3 adds the focused cell's freshness as-of caption ("Mis à
        // jour le {}"). Story 3.4 adds the reconciliation reveal + resolve controls ("Fournisseur :
        // {}", "Accepter (fournisseur)", "Ignorer (fournisseur)"). Story 4.1 fleshes out the watchlist
        // screen (add field, list rows, link/reorder/remove actions): net +9 @tr literals. Story 4.2
        // adds the neutral buy-zone summary + per-row fact: +2. Story 4.3 builds the holdings register
        // (reference-currency fact, the add/edit form's symbole/quantité/prix fields + ajouter/
        // enregistrer/annuler/modifier/retirer actions, the empty state, the per-row quantité/prix
        // facts) and the Réglages reference-currency panel (title + CHF/EUR/USD/GBP chips): net +16.
        // Keep the floor strict against a broken scan.
        assert!(
            total >= 254,
            "posture gate scanned only {total} @tr() literals — extraction broken?"
        );
    }

    #[test]
    fn label_table_strings_are_neutral_no_banned_verb() {
        for entry in &crate::labels::LABELS {
            assert_neutral(entry.naic, "labels.rs (naic set)");
            assert_neutral(entry.neutral, "labels.rs (neutral set)");
        }
    }

    #[test]
    fn rust_side_user_facing_messages_are_neutral_no_banned_verb() {
        // Story 2.2 adds Rust-side user-facing strings (create-dialog refusals, journal banners)
        // that never pass through `@tr()`, so the .slint scan above misses them. They are collected
        // in `state::USER_FACING_MESSAGES` for exactly this gate (FR13). Persistence error messages
        // spliced into some banners are gated in their own crate's posture test, not re-scanned here.
        //
        // Story 2.3/2.4 note: every NEW user-facing string the faithful/editable form introduces
        // (section titles, column/row labels, the §2 raw-input row labels, the not-available marker,
        // the source caption, the entry hint) is a French `@tr()` literal in `ui/**/*.slint`, so the
        // `.slint` scan above covers it — the form adapter (`viewmodel/form.rs`) emits only data +
        // enum-derived state strings, so it has no label inventory to register here. Story 2.4 adds
        // two Rust-side notices (clipboard-unavailable, paste-clipped) to `state.rs`. Story 2.5 adds
        // three more (soft-lock refusal, the unlock-all confirmation + completion notices — each with
        // a `{n}` count placeholder that is harmless to the banned-verb scan).
        for message in crate::state::USER_FACING_MESSAGES {
            assert_neutral(message, "state.rs (journal/create/entry notices)");
        }
        // Guard the count so a future message added without registering it here is caught. Story 2.6
        // adds the normalize-failure notice (`MSG_NORMALIZE_FAILED`); Story 2.12 adds the six dashboard
        // archive/un-archive/delete confirm + done templates; Story 2.13 adds the two verify-engine
        // summary templates + the demo-unavailable notice; Story 3.2 adds the seven provider-key
        // notices (saved/deleted/testing/ok/invalid/forbidden/keychain-unavailable). Story 3.3 retires
        // MSG_PROVIDER_DONE (folded into the refresh path) and adds the four refresh-cause notices
        // (no-change / price / input / both): net 34 − 1 + 4 = 37. Story 3.5 adds the three
        // graceful-failure cause notices (offline / quota / no-data): 37 + 3 = 40. Story 3.6 adds
        // the annual-update re-validation-scope clause (MSG_REFRESH_REVALIDATE): 40 + 1 = 41. Story
        // 4.1 adds the watchlist "no study for this ticker" notice (MSG_WATCH_NO_STUDY): 41 + 1 = 42.
        // Story 4.3 adds the two holdings-register validation notices (invalid number / empty
        // symbol): 42 + 2 = 44.
        assert_eq!(
            crate::state::USER_FACING_MESSAGES.len(),
            44,
            "state.rs message inventory changed — register the new notice"
        );
    }

    /// Story 2.6 adds Rust-side user-facing labels in `viewmodel/engine.rs` (open-gate field/state
    /// nouns, trend nouns, the temporal-provenance pieces, the traceability labels) that are built
    /// dynamically and so never pass through `@tr()`. They are collected in
    /// `engine::USER_FACING_LABELS` for exactly this gate (FR13). The zone-label nouns
    /// (ACHAT/NEUTRE/VENTE) come from the `Labels` table (scanned in `label_table_strings_…`) and
    /// are method nouns, banned-verb-exempt.
    #[test]
    fn engine_user_facing_labels_are_neutral_no_banned_verb() {
        for label in crate::viewmodel::engine::USER_FACING_LABELS {
            assert_neutral(
                label,
                "viewmodel/engine.rs (verdict/zone/traceability labels)",
            );
        }
        assert_eq!(
            crate::viewmodel::engine::USER_FACING_LABELS.len(),
            22,
            "engine.rs label inventory changed — register the new label"
        );
    }

    #[test]
    fn tr_literal_extraction_handles_context_placeholders_and_escapes() {
        let source = r#"
            Text { text: @tr("Bonjour {}", name); }
            Text { text: @tr("ctx" => "Avec \"guillemets\""); }
            Text { text: @tr("{n} item" | "{n} items" % count); }
        "#;
        let literals = tr_literals(source);
        assert_eq!(
            literals,
            vec![
                "Bonjour {}".to_string(),
                "ctx".to_string(),
                "Avec \"guillemets\"".to_string(),
                "{n} item".to_string(),
                "{n} items".to_string(),
            ]
        );
    }

    #[test]
    fn the_word_matcher_is_whole_word_and_case_insensitive() {
        assert!(contains_word("Veuillez Acheter maintenant", "acheter"));
        assert!(contains_word("you should do this", "should"));
        // Substrings are not words: the app name embeds "invest".
        assert!(!contains_word("steadyinvest", "invest"));
        // A hyphen IS a word boundary: a key like "zone-hold" would match, which is exactly
        // why the gate scans display strings, never the stable keys.
        assert!(contains_word("zone-hold", "hold"));
        // Multi-word phrases match case-insensitively too.
        assert!(contains_word("mais Il Faut le noter", "il faut"));
    }
}
