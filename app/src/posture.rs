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
        // modal_dialog: the variant glyphs — a refusal to acknowledge, a form to fill
        "⚠",
        "✎",
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
    /// could surface in a banner. Since 2026-09-26 no user-visible string carries it any more
    /// (the rails name the failure in French and LOG the Display — SQLite's « attempt to write a
    /// readonly database » had reached the refusal dialog; `no_error_display_reaches_a_user_string`
    /// keeps it so); the scan stays as defence in depth, the log being read by people too.
    /// `Sqlite(rusqlite::Error)` cannot be constructed here
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
            Error::WriteProtected { directory: false }.to_string(),
            Error::WriteProtected { directory: true }.to_string(),
            Error::WriteProtectedOutdated {
                file_user_version: 1,
                supported: 9,
                directory: true,
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
        // Story 4.4 adds the holdings price-refresh button + per-row zone marker ("◆ {}" ×3) + the
        // "no linked study" hint + the present-price fact + the freshness murmurs (périmé / à jour le
        // {}): net +8 = 262. Issue #52 adds the in-flight button label ("Rafraîchissement…"): +1 = 263.
        // Story 4.5 adds the per-holding trailing-stop column (stop fact + "◆ sous le stop" + the
        // "Seuil %" field + définir/mettre-à-jour/retirer actions) and the Réglages default-% panel
        // (title + "Pourcentage (0–100)" + enregistrer/effacer): net +10 = 273. The 4.5 review adds
        // the "à {} {} au-dessus" distance-above fact (AC4): +1 = 274. Story 4.6 adds the capital-at-
        // risk header fact (with + without the "% du capital investi" clause): +2 = 276. Story 4.7 adds
        // the neutral trigger panel — the two trigger facts (stop / sell-zone), the rationale
        // placeholder, and the Enregistrer-une-vente / Relever-le-stop / Ignorer actions: +6 = 282.
        // Story 5.2 adds the dashboard export/import controls (Exporter + the import path placeholder +
        // "Importer une étude"): +3 = 285. Story 5.3 adds the Réglages whole-journal panel (title
        // "Journal complet", the explanatory caption, "Exporter le journal", the import path
        // placeholder, "Importer un journal"): +5 = 290. Story 5.4 adds the Réglages backup/restore
        // panel (title "Sauvegarde & restauration", the caption, "Créer une sauvegarde", the restore
        // path placeholder, "Restaurer", "Confirmer la restauration", "Annuler"): +7 = 297. Story 5.5
        // adds the Réglages journal-location panel (title "Emplacement du journal", "Journal actuel :",
        // "Ouvrir un journal…", "Créer un journal…", "Journaux récents", "(actuel)", "Ouvrir", "Lever
        // le verrou et ouvrir"): +8 = 305. Story 7.4 adds the Réglages "Twelve Data" provider chip: +1
        // = 306. Story 5.1 adds the dashboard "Confronter" action + the confront overlay (title, the
        // "Décision du {}" caption, the neutral empty state, the "Estimation haute/basse : {}" + "Trait
        // clair : cours réel" legend, "Fermer"): +8 = 314. Story 5.6 adds the dashboard "Exporter PDF"
        // row action: +1 = 315. Story 6.1 adds the Portefeuille multi-portfolio controls (the add-name
        // placeholder + "Ajouter un portefeuille", the rename placeholder + "Renommer", "Supprimer le
        // portefeuille"): +5 = 320. Story 6.2 replaces the single capital-at-risk header with the
        // per-currency block ("Capital à risque par devise :" + the two per-row templates, −2 old
        // single-total templates) and adds the "Devise :" holding-currency picker label: net +2 = 322.
        // The 6.2 review then unified the Réglages reference-currency chips on the pushed
        // SUPPORTED_CURRENCIES model (a `for` over plain codes — currency codes are not
        // translatable copy), retiring the four hardcoded @tr("CHF"/"EUR"/"USD"/"GBP") chips:
        // −4 = 318. Story 6.3 adds the transaction-ledger surface (the Transactions toggle pair,
        // the sell-quantity placeholder, the ledger empty state, the Achat/Vente row nouns + the
        // qty×price and Frais templates, the row Modifier/Supprimer, the five form placeholders,
        // the record/update/abandon action labels): +18 = 336. Its review adds the ledger-form
        // « Enregistrer une vente » (a second occurrence of the 4.7 label): +1 = 337. Story 6.4
        // adds the dividend surface (the Dividende row noun, the Retenue row template, the net
        // template, « Enregistrer un dividende », the reinvestable-cash header + its amount
        // template, the Réglages withholding panel title + placeholder + the Enregistrer reuse):
        // +9 = 346. Its review adds the on-form dividend-semantics caption (the « Frais (vide =
        // 0) » placeholder is a buy/sell fact, so the dividend meaning is stated on the form):
        // +1 = 347. Story 6.5 adds the FX panel (the Taux de change title, the empty state, the
        // date-source template, the two refresh labels, the three manual-entry placeholders, the
        // Enregistrer le taux action): +9 = 356. Story 6.6 adds the consolidation block (the
        // header template, the three per-bank line variants, the two global-total variants, the
        // global-missing template, the rates footnote): +8 = 364. Its review adds the two
        // plain-« indisponible » variants (per-bank + global — an overflow/read-failure absence
        // must name itself, never render a dangling empty amount): +2 = 366. Story 6.7 adds the
        // Réglages « Concentration et diversification par taille » panel (title, threshold
        // placeholder + Enregistrer, the table caption, the three class rows' labels +
        // boundary/target placeholders, « Enregistrer la table ») and the Portefeuille
        // concentration block (the header template, the per-security line variants incl. the
        // near-threshold murmur, the two Parts-indisponibles variants, « Concentration
        // indisponible », the size-mix header + line variants with the nested Petite/Moyenne/
        // Grande nouns, the three non-classé reason templates, the rates footnote occurrence):
        // +39 = 405. Its review adds the « conversion impossible » non-classé variant (a
        // sales-conversion overflow must name its own reason, never « chiffre d'affaires
        // indisponible »): +1 = 406. Story 6.8 adds the replacement-candidates panel (the two
        // header variants, the empty state, the four candidate-line variants, the H/B template,
        // the two held-share variants, the currency-missing template, the two currency-exposure
        // variants, « Ouvrir l'étude » / « Études » / « Fermer », the trigger-panel
        // « Candidats » action): +17 = 423. Its review adds the honest-header variant (a
        // trigger-open must never assert « Vente enregistrée »), the watchlist-unavailable
        // state, the three zone-noun line variants (a known zone without a statable distance),
        // the generic exposure-indisponible variant and the FR28 rates footnote: +7 = 430.
        // Story 6.9 adds the Réglages fallback rows (the section caption, the three field
        // labels Prix/Fondamentaux/Taux de change, the three « Aucun repli » chips, the
        // EODHD/Twelve Data chip occurrences across the price/fundamentals/fx rows): +12 = 442.
        // Issue #95 adds the four « étude indisponible » read-failure variants (the holdings
        // register row, the non-classé reason, the candidate line, the confront overlay —
        // a read FAILURE must never state an absence): +4 = 446. Issue #84 adds the
        // « Positions vendues » section (the show/hide toggle pair, the sold-row fact line,
        // the « Racheter : … » caption, and second occurrences of the ledger vocabulary:
        // Transactions/Masquer, the empty state, the three kind nouns, the qty×price /
        // Frais / Retenue / net templates, the five form placeholders, « Enregistrer un
        // achat »): +20 = 466. Issue #65 adds the import-arbitration banner actions
        // (« Confirmer l'import » + an « Annuler » occurrence): +2 = 468. Issue #34 (PR 2) adds
        // the « Historique » panel (the toggle pair, the panel title, the unavailable + empty
        // states, the per-entry Détail/Masquer pair): +7 = 475. Issue #48 adds the three
        // below-band fact lines (watchlist, register zone slot, candidate line): +3 = 478.
        // Issue #98 (PR 1) adds the holding-form « Secteur (facultatif) » placeholder and the
        // register's « Secteur : {} » caption: +2 = 480. Issue #98 (PR 3) adds the six sector
        // exposure lines on the candidates panel (non-renseigné per candidate, missing-rate,
        // flagged + plain share, indisponible, the panel-level blind-spot line): +6 = 486.
        // The UX pass (2026-09-23, Portefeuille first): the ModalDialog component (its default
        // « Action refusée » title, Compris/Annuler/Enregistrer, every labelled field + placeholder
        // of the position / buy / sell / dividend / stop / portfolio forms, the study-lookup
        // captions) and the carded Portefeuille screen (card titles + subtitles, the dialog
        // openers' titles and explanatory sentences, the « Aucune étude liée » cause bands, the
        // all-unclassified band, the …-suffixed action labels) replace the five-placeholder ledger
        // row, the inline add/edit form, the trailing-stop field and the inline ledger-delete
        // confirm. The running tally above had drifted below what the scan actually finds, so the
        // floor is RE-BASED on the measured total after this pass: 594. Its PR 2 (Liste de
        // suivi card + add-value dialog, the Études card + create-study dialog, the Réglages
        // FX-rate dialog, the derived confirm titles/verbs for the study action / older import /
        // restore prompts) measures 612. The drop-down pass (Guy, 2026-09-24: constrained choices as
        // lists — the study currency, the FX base currency, the position symbol; their placeholders
        // and the « aucune étude dans le dossier » band) measures 620; the est-low EPS guidance
        // (the caption under the field + the glossary entry, Guy 2026-09-24) measures 623. Story
        // 7.2 adds the « Revue » screen (the fifth destination, the card titles and subtitles, the
        // share-block templates and their reason bands, the position rows' study / zone / data
        // words, the due-list reasons, the counts) and measures 706. Story 7.1 adds the company
        // comparison (the picker card on Études, the thirty row labels, the four group titles,
        // the zone / state words, the currency-mix band) and measures 768. Story 7.3 adds the
        // « Examen rapide » (the picker card, the two ladders' labels, the price record, the
        // reader's fields, the four conclusions) and measures 850. Story 7.3 PR 2 adds the watchlist
        // criblage (the button, the card, its column heads, the row states and fact words, the
        // quota band, the « Retour à la liste de suivi » label) and measures 892. The G1 review of
        // 7.2 adds the sector murmur « atteint ou dépassé » (Decision 6), the size block's and the
        // global total's plain « indisponible », « Parts indisponibles. », the « non calculable »
        // study band and the two unknown-last-save variants (position caption, due line): +7 =
        // 899. Its follow-up review adds the two due reasons « ancienneté inconnue » and
        // « données non calculables » and the mixed-links band (lots linked to different
        // studies); the stop captions and the counts line are reworded in place: +3 = 902.
        // Floor strict.
        // The G1 on-screen check adds the stop caption without a repeated level list (every level
        // breached): +1 = 903.
        // integ/g1-a-to-h (G1 fix PRs A–H combined): 892 + B 11 + C 9 + D 26 + E 15 = 953, measured.
        // G1 I (locale numbers + its review) adds no @tr literal (its refusals are MSG_*; the new
        // Slint lines are callbacks and handlers): 953 + 0 = 953, measured.
        // G1 final review of the « Revue » (area 2): the empty dossier's size statement, the
        // global total's both-causes band and the not-compared legacy stop: 953 + 3 = 956,
        // measured (the reworded trigger / mixed-links / due-subtitle lines replace, not add).
        // Its G3 review: the mixed-links band's four more variants (a lot without a study, an
        // unreadable one), the other studies' signals / high zone, the unreadable lot's stop, and
        // the due count on its own line: 956 + 8 = 964, measured.
        // G1 M (#237): rows 5 / 6 of the comparison and the study screen's §2 average column say
        // the years actually averaged (comparison 2 → 6 labels, study 1 → 4 titles): +7 = 960,
        // measured. The same pass, area 4 (#237): the rate « — » cause, the « different years »
        // band and criblage word, the criblage's « fenêtre incomplète », the kept-examination band
        // and its « Ouvrir l'examen » (the ladder's unavailable band is reworded in place): +6 =
        // 966, measured. Its G3 review: the study screen's « Moy. a / b ans » goes (−1); the
        // criblage names which ladder is short or absent (6 years-column words, the « fenêtre
        // incomplète » one reworded: +5) and a rate's « — » cause (+3); the kept failure's band
        // and its « Compris » (+2): +9 = 975, measured.
        // G1 final review (K): every read failure of the portfolio, watchlist and FX surfaces is
        // « indisponible » (portfolios, positions, ledger ×2, sold positions title + band, FX
        // rates, watchlist, a watched study): 953 + 9 = 962, measured. (The ledger sale's
        // « Quantité vendue » replaces its « (vide = toute la position) » occurrence: ±0.)
        // The legacy lot's uncompared stop is stated (« non comparé au prix : le lot n'a pas de
        // devise renseignée »): 962 + 1 = 963, measured.
        // integ/g1-final2: 953 + L 11 + M 22 + K 10 = 996, measured.
        // G1 P: a watched study that was deleted is worded as an absence, and the reinvestable
        // dividends say « indisponible » on a failed read: 996 + 2 = 998, measured.
        // fix/reports-naic (Guy's decisions, 2026-09-26): the comparison's judged-value note
        // (« * valeur jugée par l'analyste … »), the review share row whose amount stands while
        // its share is blocked, and « Total global : … (100 %) »: 998 + 3 = 1001, measured (row
        // 20's words, rows 12 / 14's labels and the counts line are reworded in place). Its G3
        // review keeps the target on the blocked-share row (« … · cible {} % »): 1001 + 1 =
        // 1002, measured (the judged note, rows 11 / 13 / 15, the counts line and the
        // unclassified bands are reworded in place). The 2026-09-26 on-screen defect names the
        // cause of a read-only dossier in its two bands (the dialog, « Portefeuilles ») — a
        // protected file, a protected directory, beside the newer schema: 1002 + 4 = 1006,
        // measured. The second G3 review names a side file (-wal / -shm) this account cannot
        // write, in both bands: 1006 + 2 = 1008, measured.
        // ssg-1.2.0: the glossary's « Année d'une étude » term and definition (the BPA and the
        // year definitions are reworded in place): 1008 + 2 = 1010, measured.
        assert!(
            total >= 1010,
            "posture gate scanned only {total} @tr() literals — extraction broken?"
        );
    }

    /// Issue #34 (FR51, PR 2): the « Historique » timeline builds its French lines in Rust (the
    /// verdict-trace precedent) — its vocabulary is inventoried and scanned like engine's.
    #[test]
    fn history_user_facing_labels_are_neutral_no_banned_verb() {
        for label in crate::viewmodel::history::HISTORY_USER_FACING_LABELS {
            assert_neutral(label, "viewmodel/history.rs (timeline labels)");
        }
        assert_eq!(
            crate::viewmodel::history::HISTORY_USER_FACING_LABELS.len(),
            17,
            "history.rs label inventory changed — register the new label"
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
        // the annual-update contradiction clause (MSG_REFRESH_CONTRADICTED — the #110 (b) freeze-and-
        // notify replacement for the retired re-validation-scope clause, same count): 40 + 1 = 41. Story
        // 4.1 adds the watchlist "no study for this ticker" notice (MSG_WATCH_NO_STUDY): 41 + 1 = 42.
        // Story 4.3 adds the two holdings-register validation notices (invalid number / empty
        // symbol): 42 + 2 = 44. Story 4.4 adds the two holdings price-refresh notices (refreshing /
        // nothing-linked): 44 + 2 = 46. Story 4.5 adds the trailing-stop validation notice: 46 + 1 = 47.
        // Story 4.7 adds the recorded-sell confirmation (MSG_HOLDING_SOLD): 47 + 1 = 48. Story 5.2
        // adds the study export/import notices (exported / imported / updated / export-missing +
        // integrity / version / malformed import refusals): 48 + 7 = 55 (the "updated" notice was
        // added by the 5.2 review — surface an overwrite distinctly). Story 5.3 adds the two
        // whole-journal notices (MSG_JOURNAL_EXPORTED + the MSG_JOURNAL_IMPORTED count template); the
        // integrity / version / malformed import refusals are reused from 5.2: 55 + 2 = 57. Story 5.4
        // adds the backup/restore notices (backup-created, restore-done/failed, 4 restore refusals
        // [integrity / newer-schema / not-a-journal / unreadable], the confirm template + 2 reason
        // clauses [stale / foreign]): 57 + 10 = 67. Story 5.5 adds the journal-location notices
        // (opened, created, open-failed, locked-elsewhere, lock-reclaimable, sync-folder warning, the
        // stale-version template): 67 + 7 = 74. Story 7.4 adds the "no provider selected" fetch notice
        // (MSG_PROVIDER_NONE): 74 + 1 = 75. Story 6.1 adds the three multiple-portfolio notices
        // (invalid name + the two guarded-delete refusals: has-holdings / last): 75 + 3 = 78. Story 6.2
        // adds the unsupported-holding-currency notice (MSG_HOLDING_INVALID_CURRENCY): 78 + 1 = 79.
        // Story 6.3 adds the six transaction-ledger notices (buy-recorded, partial-sold, updated,
        // deleted, the over-sell refusal, the invalid-date refusal): 79 + 6 = 85; its review adds
        // the ledger-backed direct-edit refusal (MSG_LEDGER_BACKED): 85 + 1 = 86. Story 6.4 adds
        // the two dividend notices (recorded + the withholding-exceeds-gross refusal): 86 + 2 = 88;
        // its review adds the retired-holding scope refusal (MSG_DIVIDEND_RETIRED): 88 + 1 = 89.
        // Story 6.5 adds the six FX notices (recorded, invalid-rate, same-currency, refreshing,
        // the refreshed-count template, no-pairs): 89 + 6 = 95; its review adds three more
        // (journal-changed-in-flight, invalid-currency [a currency problem must not be reported
        // as a rate problem], future-date): 95 + 3 = 98. Story 6.7 adds the two risk-settings
        // refusals (concentration-threshold invalid, size-table invalid): 98 + 2 = 100. Story
        // 6.9 adds the fallback-chain notice (MSG_PROVIDER_FALLBACK — a fetch served by a
        // non-primary member names itself, FR26): 100 + 1 = 101. Issue #96 splits the one opaque
        // size-table refusal into a field-naming template + a crossed-pair notice: 101 − 1 + 2 = 102.
        // Issue #101 adds the skipped-keyless-fallback clause (MSG_FALLBACK_NO_KEY): 102 + 1 = 103.
        // Issue #85 adds the unknown-transaction-kind notice (MSG_LEDGER_UNKNOWN_KIND): 103 + 1 = 104.
        // Issue #88 adds the two percent-panel refusals (trailing-stop + withholding): 104 + 2 = 106.
        // Issue #60 adds the holding-amount-out-of-range refusal (MSG_HOLDING_AMOUNT_OUT_OF_RANGE): 106 + 1 = 107.
        // Issue #61 adds the un-protected-exposure line (MSG_UNSTOPPED_EXPOSURE): 107 + 1 = 108.
        // Issue #100 adds the refresh-cancelled notice (MSG_REFRESH_CANCELLED): 108 + 1 = 109.
        // Issue #90 adds the FX-rate-deleted notice (MSG_FX_DELETED — the panel's repair path): 109 + 1 = 110.
        // Issue #41 adds the blank-key notice (MSG_KEY_BLANK — a blank save is not a silent no-op): 110 + 1 = 111.
        // Issue #42 adds two key-test verdicts (MSG_KEY_OK_QUOTA accepted-but-quota, MSG_KEY_TEST_INCONCLUSIVE network): 111 + 2 = 113.
        // Issue #44 adds three keychain-cause notices (MSG_KEY_AMBIGUOUS, MSG_KEY_TOO_LONG, MSG_KEYCHAIN_ERROR): 113 + 3 = 116.
        // Issue #37 adds the unmatched-provider-years notice (MSG_REFRESH_UNMATCHED_YEARS): 116 + 1 = 117.
        // Issue #35 adds the max-year-window notice (MSG_YEARS_MAX — the extend-history cap): 117 + 1 = 118.
        // Issue #63 adds the export-unreadable notice (MSG_EXPORT_UNREADABLE — a present-but-unreadable
        // study told apart from a truly absent one): 118 + 1 = 119.
        // Issue #67 adds the uncheckpointed-backup refusal (MSG_RESTORE_UNCHECKPOINTED — a raw copy
        // of a live journal whose -wal holds commits the .db lacks): 119 + 1 = 120.
        // Issue #65 adds the older-import arbitration prompt (MSG_IMPORT_CONFIRM — a same-journal
        // version regression is stated and confirmed, never merged silently): 120 + 1 = 121.
        // Issue #218 adds the no-study refusal for a position (MSG_HOLDING_NO_STUDY — a position is
        // only added for a ticker with a study, in that study's currency): 121 + 1 = 122.
        // Story 7.2 adds the review's export outcome (MSG_REVIEW_EXPORTED) and the nine quality
        // flags worded as neutral facts (MSG_FLAG_*): 122 + 10 = 132. Story 7.1 adds the
        // comparison's export outcome (MSG_COMPARISON_EXPORTED): 132 + 1 = 133. The walk of
        // 2026-09-24 adds the watchlist duplicate refusal (MSG_WATCH_DUPLICATE): 134. Story 7.3
        // adds the examination's export / study-created outcomes, its two source words and its
        // blank-symbol refusal: 139. The G1 review adds the examination's blank-currency refusal
        // and the study-created-but-empty notice (MSG_QUICK_BLANK_CURRENCY,
        // MSG_QUICK_SCREEN_STUDY_EMPTY) and the criblage's unreadable-watchlist refusal
        // (MSG_SCREENING_LIST_UNREADABLE): 142. The G1 review of 7.1 (#237) adds the marker of a
        // comparison pick whose study is gone (MSG_COMPARISON_PICK_GONE): 142 + 1 = 143. The G1 I
        // review names the ambiguous typed number (MSG_NUMBER_AMBIGUOUS_COMMA / _POINT, the
        // expected spelling per format), the study entry that is no number
        // (MSG_VALUE_NOT_A_NUMBER — refused, the value kept) and the pasted lines kept by year
        // (MSG_PASTE_LINES_KEPT): +4.
        assert_eq!(
            crate::state::USER_FACING_MESSAGES.len(),
            221,
            // integ/g1-a-to-h: A 142 + C 1 + E 6 + H 4 = 153, measured; + I 4 = 157, measured.
            // The I on-screen check names an ambiguous size-table field (MSG_SIZE_FIELD_AMBIGUOUS,
            // issue #96): 157 + 1 = 158, measured. The G1 final review of the study PDF export
            // names its own refusals (MSG_STUDY_PDF_UNRENDERABLE, MSG_EXPORT_WRITE_FAILED):
            // 158 + 2 = 160, measured. Its G3 review refuses a completed « .pdf » name that is taken
            // (MSG_EXPORT_NAME_TAKEN): 160 + 1 = 161, measured. The G1 final review (L10) states
            // that no dossier is open after a refused switch that lost the previous one
            // (MSG_NO_JOURNAL_OPEN): 161 + 1 = 162. The worker-gone cause moves out of three rails
            // into a registered message (MSG_FETCH_WORKER_GONE): 162 + 1 = 163.
            // G1 final review (K), counted from 158 on its own branch: « Retirer » refused up front
            // by cause (MSG_HOLDING_HAS_TRANSACTIONS, MSG_HOLDING_LEDGER_UNREADABLE): +2.
            // Each ledger amount refusal names its field (MSG_LEDGER_QUANTITY_EMPTY / _INVALID_
            // QUANTITY / _PRICE / _FEES / _OUT_OF_RANGE / _ROW_INVALID, MSG_DIVIDEND_INVALID_
            // QUANTITY / _GROSS / _WITHHOLDING) and an unreadable linked study refuses the trigger
            // sale and the stop (MSG_SELL_STUDY_UNAVAILABLE, MSG_STOP_STUDY_UNAVAILABLE):
            // +11. A restore whose checkpoint or safety snapshot of the
            // current dossier fails is refused by name (MSG_RESTORE_CHECKPOINT_FAILED,
            // MSG_RESTORE_SNAPSHOT_FAILED): +2. « Lier une étude » names an
            // unreadable study list or watchlist (MSG_WATCH_STUDY_UNAVAILABLE,
            // MSG_WATCH_LINK_LIST_UNREADABLE): +2. integ/g1-final2: 163 + K 17 = 180, measured.
            // G1 P (D5): a currency-less lot's trigger sale is refused against a study in another
            // currency (MSG_SELL_STUDY_OTHER_CURRENCY): 180 + 1 = 181, measured. A restore
            // never replaces an earlier `-prerestore` and a failed rollback says the dossier was
            // replaced (MSG_RESTORE_SNAPSHOT_EXISTS, MSG_RESTORE_ROLLBACK_FAILED): 181 + 2 = 183. A write
            // rail's failed READ is named as a read (MSG_READ_FAILED): 183 + 1 = 184, measured.
            // G1 P review (L-c): a legacy lot's cost-basis seed is stated
            // (MSG_STOP_SEEDED_FROM_COST): 184 + 1 = 185, measured. A leftover
            // -prerestore is named at startup (MSG_PRERESTORE_FOUND): 185 + 1 = 186, measured. A reference
            // change names the legacy stops it does not convert
            // (MSG_LEGACY_STOPS_REFERENCE_CHANGED): 186 + 1 = 187, measured. The 2026-09-26
            // on-screen defect names a dossier protected against writing — the write refusals by
            // cause (MSG_READ_ONLY_FILE_WRITE, MSG_READ_ONLY_DIR_WRITE), the state lines
            // (MSG_STARTUP_FILE_PROTECTED, MSG_STARTUP_DIR_PROTECTED), the unmigratable open
            // (MSG_OPEN_PROTECTED_OUTDATED) and a write the OS refused on the spot
            // (MSG_WRITE_REFUSED_BY_SYSTEM): 187 + 6 = 193, measured. No raw error Display reaches
            // the user (same defect, closed everywhere): the save / open / read templates with a
            // cause (MSG_SAVE_FAILED_CAUSE, MSG_JOURNAL_OPEN_FAILED_CAUSE, MSG_READ_SUBJECT_FAILED,
            // MSG_READ_SUBJECT_FAILED_CAUSE), the seven cause kinds (MSG_CAUSE_*) and the eight
            // read subjects (MSG_SUBJECT_*): 193 + 19 = 212, measured. The G3 review names the
            // protected-and-outdated cause by what is protected (MSG_CAUSE_OUTDATED_FILE /
            // _DIR, replacing MSG_OPEN_PROTECTED_OUTDATED), a file replaced while open
            // (MSG_CAUSE_REPLACED) and a configured dossier refused by name at startup
            // (MSG_CONFIGURED_REFUSED): 212 - 1 + 4 = 215, measured. The second G3 review: no dossier
            // open after a refused configured one (MSG_CONFIGURED_REFUSED_NONE,
            // MSG_CONFIGURED_UNREADABLE_NONE), the read copy failure and a concurrent change
            // (MSG_CAUSE_READ_COPY, MSG_CAUSE_CHANGED_DURING_COPY), an unwritable side file
            // (MSG_STARTUP_SIDECAR_PROTECTED, MSG_READ_ONLY_SIDECAR_WRITE): 215 + 6 = 221, measured.
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
            // G1 M (#237): the years a comparison cell's §2 average runs over (AVG_OVER_ONE_YEAR,
            // AVG_OVER_YEARS): 23 + 2 = 25.
            25,
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

    // ── No raw error Display in a user-visible string (2026-09-26, strengthened by G3 L8) ──
    //
    // A persistence / SQLite / file / serde / provider error's `Display` is English third-party
    // text: it goes to the LOG, never into a String the app may show (a banner, a dialog, a
    // notice, a status line, a PDF). The rails name what failed in French (`state::save_error`,
    // `read_failure`, `open_error`, `io_save_error`, `provider_failure_notice`). This scan walks
    // the app's Rust sources — outside comments, logging / stderr / panic / assert macros and test
    // modules, string literals honoured when matching brackets — for every shape that turns an
    // error into a String:
    // - the ERROR BINDINGS: `error` / `e` / `err`, plus every name bound by `Err(x)`,
    //   `map_err(|x|`, `unwrap_or_else(|x|`, `or_else(|x|` in the file;
    // - such a binding in a format string (`{x}`, `{x:?}`), as a `format!` / `write!` argument
    //   (a multi-line call included), or `x.to_string()`; and `ToString::to_string` anywhere.
    // A binding that holds a French message (`Err(message)` of a rail already in French) is
    // handed on as-is — never re-formatted — so it does not match.
    //
    // The other crates hand the app TYPED errors only (no public `Result<_, String>`), so their
    // English can only become text in the app — where this scan looks
    // (`sibling_crates_hand_the_app_typed_errors_only`).

    /// Stderr-only sites, allowed by name with their reason: `(file, snippet)`.
    const ERROR_DISPLAY_ALLOW: &[(&str, &str)] = &[
        // `config::load`'s warning reaches stderr only (`main.rs` `eprintln!`), never the UI.
        ("config.rs", "unreadable ({error}); defaults in effect"),
        ("config.rs", "invalid ({error}); defaults in effect"),
    ];

    fn rust_sources(crate_dir: &str) -> Vec<PathBuf> {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).expect("src/ readable") {
                let path = entry.expect("dir entry").path();
                if path.is_dir() {
                    walk(&path, out);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    out.push(path);
                }
            }
        }
        let mut files = Vec::new();
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(crate_dir)
            .join("src");
        walk(&root, &mut files);
        files.sort();
        files
    }

    /// One pass over Rust source: the code WITHOUT its comments, and per byte of that code
    /// whether it lies inside a string / char literal (plain, escaped, byte and raw `r#"…"#`
    /// strings; a `'a` lifetime is no literal). Comments and literals are read together, so a
    /// lone `"` in a comment cannot flip the rest of the file (second G3 L8).
    fn lex(source: &str) -> (String, Vec<bool>) {
        let b = source.as_bytes();
        let mut code = String::with_capacity(source.len());
        let mut mask: Vec<bool> = Vec::with_capacity(source.len());
        let push = |code: &mut String, mask: &mut Vec<bool>, text: &str, lit: bool| {
            code.push_str(text);
            mask.extend(std::iter::repeat_n(lit, text.len()));
        };
        let mut i = 0;
        while i < b.len() {
            let rest = &source[i..];
            if rest.starts_with("//") {
                i += rest.find('\n').unwrap_or(rest.len());
                continue;
            }
            if rest.starts_with("/*") {
                i += rest.find("*/").map_or(rest.len(), |p| p + 2);
                continue;
            }
            let ident_before = i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
            // Raw strings: r"…", r#"…"#, br#"…"#.
            let raw_at = if !ident_before && rest.starts_with("br") {
                Some(2)
            } else if !ident_before && rest.starts_with('r') {
                Some(1)
            } else {
                None
            };
            if let Some(skip) = raw_at {
                let hashes = rest[skip..].bytes().take_while(|&c| c == b'#').count();
                if rest[skip + hashes..].starts_with('"') {
                    let close = format!("\"{}", "#".repeat(hashes));
                    let body = skip + hashes + 1;
                    let end = rest[body..]
                        .find(&close)
                        .map_or(rest.len(), |p| body + p + close.len());
                    push(&mut code, &mut mask, &rest[..end], true);
                    i += end;
                    continue;
                }
            }
            if b[i] == b'"' {
                let mut j = i + 1;
                while j < b.len() && b[j] != b'"' {
                    j += if b[j] == b'\\' { 2 } else { 1 };
                }
                let end = (j + 1).min(b.len());
                push(&mut code, &mut mask, &source[i..end], true);
                i = end;
                continue;
            }
            if b[i] == b'\'' {
                // 'x', '\n', '\'' — else a lifetime / label.
                let len = if rest[1..].starts_with('\\') {
                    rest[2..].find('\'').map(|p| p + 3)
                } else {
                    rest[1..]
                        .chars()
                        .next()
                        .filter(|c| rest[1 + c.len_utf8()..].starts_with('\''))
                        .map(|c| c.len_utf8() + 2)
                };
                if let Some(len) = len {
                    push(&mut code, &mut mask, &rest[..len], true);
                    i += len;
                    continue;
                }
            }
            let ch = rest.chars().next().expect("char");
            let mut buf = [0u8; 4];
            push(&mut code, &mut mask, ch.encode_utf8(&mut buf), false);
            i += ch.len_utf8();
        }
        (code, mask)
    }

    /// The byte index of the bracket closing the one at `open`, literals skipped (`mask` from
    /// [`lex`]); the end of `code` when unbalanced.
    fn group_end(code: &str, mask: &[bool], open: usize) -> usize {
        let b = code.as_bytes();
        let (o, c) = match b[open] {
            b'(' => (b'(', b')'),
            b'{' => (b'{', b'}'),
            b'<' => (b'<', b'>'),
            _ => (b'[', b']'),
        };
        let mut depth = 1usize;
        for (i, &ch) in b.iter().enumerate().skip(open + 1) {
            if mask[i] {
                continue;
            }
            if ch == o {
                depth += 1;
            } else if ch == c {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
        }
        b.len() - 1
    }

    /// `source` without comments, without `#[cfg(test)] mod …` modules, and without the bodies of
    /// the macros that never reach the user (logging, stderr, panics, asserts).
    fn user_reachable_code(source: &str) -> String {
        let (mut code, _) = lex(source);
        for marker in ["#[cfg(test)]\nmod ", "#[cfg(test)]\n    mod "] {
            while let Some(start) = code.find(marker) {
                let open = start + code[start..].find('{').expect("test module body");
                let (_, mask) = lex(&code);
                let end = group_end(&code, &mask, open);
                code.replace_range(start..=end, "");
            }
        }
        for mac in [
            "tracing::trace!(",
            "tracing::debug!(",
            "tracing::info!(",
            "tracing::warn!(",
            "tracing::error!(",
            "eprintln!(",
            "panic!(",
            "unreachable!(",
            "assert!(",
            "assert_eq!(",
            "assert_ne!(",
            "debug_assert!(",
        ] {
            let mut from = 0;
            while let Some(p) = code[from..].find(mac) {
                let start = from + p;
                let (_, mask) = lex(&code);
                if mask[start] {
                    from = start + mac.len();
                    continue;
                }
                let end = group_end(&code, &mask, start + mac.len() - 1);
                code.replace_range(start..=end, "");
                from = start;
            }
        }
        code
    }

    fn is_ident_byte(c: u8) -> bool {
        c.is_ascii_alphanumeric() || c == b'_'
    }

    /// The identifier starting at `at` (leading whitespace skipped), and where it ends.
    fn ident_at(code: &str, at: usize) -> Option<(&str, usize)> {
        let b = code.as_bytes();
        let start = (at..b.len()).find(|&i| !b[i].is_ascii_whitespace())?;
        let end = (start..b.len())
            .find(|&i| !is_ident_byte(b[i]))
            .unwrap_or(b.len());
        (end > start).then(|| (&code[start..end], end))
    }

    /// Skip whitespace and the pattern keywords `move` / `ref` / `mut` from `at`.
    fn skip_keywords(code: &str, mut at: usize) -> usize {
        loop {
            match ident_at(code, at) {
                Some((kw @ ("move" | "ref" | "mut"), end))
                    if code
                        .as_bytes()
                        .get(end)
                        .is_some_and(|c| c.is_ascii_whitespace()) =>
                {
                    let _ = kw;
                    at = end;
                }
                _ => return at,
            }
        }
    }

    fn is_binding_name(name: &str) -> bool {
        !name.starts_with('_')
            && name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            && !matches!(name, "ref" | "mut" | "move")
    }

    /// The error bindings of `code` (see the section comment): `error` / `e` / `err`, every
    /// name bound by an `Err(…)` PATTERN (`Err(x)`, `Err(ref x)`, `Err(E::V { a, b: c })`), by
    /// `map_err` / `unwrap_or_else` / `or_else` closures (`move`, spacing and all), and by
    /// `Some(x) = ….err()`.
    fn error_bindings(code: &str) -> std::collections::BTreeSet<String> {
        let (_, mask) = lex(code);
        let b = code.as_bytes();
        let mut names: std::collections::BTreeSet<String> = ["error", "e", "err"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        // Closures.
        for opener in ["map_err(", "unwrap_or_else(", "or_else("] {
            for (p, _) in code.match_indices(opener) {
                if mask[p] {
                    continue;
                }
                let at = skip_keywords(code, p + opener.len());
                let at = (at..b.len())
                    .find(|&i| !b[i].is_ascii_whitespace())
                    .unwrap_or(b.len());
                if b.get(at) != Some(&b'|') {
                    continue;
                }
                let at = skip_keywords(code, at + 1);
                if let Some((name, _)) = ident_at(code, at)
                    && is_binding_name(name)
                {
                    names.insert(name.to_string());
                }
            }
        }
        // `Err(…)` patterns.
        for (p, _) in code.match_indices("Err(") {
            if mask[p] || (p > 0 && is_ident_byte(b[p - 1])) {
                continue;
            }
            let open = p + 3;
            let close = group_end(code, &mask, open);
            let after = code[close + 1..].trim_start();
            let is_pattern = after.starts_with("=>")
                || after.starts_with('|')
                || (after.starts_with('=') && !after.starts_with("=="));
            if !is_pattern {
                continue; // `Err(x)` builds a value
            }
            let inner = &code[open + 1..close];
            if let Some(brace) = inner.find('{') {
                // A destructured variant: every bound field (`a`, `b: c` → `c`).
                let fields = &inner[brace + 1..inner.rfind('}').unwrap_or(inner.len())];
                for field in fields.split(',') {
                    let bound = field.rsplit(':').next().unwrap_or("").trim();
                    let bound = bound
                        .trim_start_matches("ref ")
                        .trim_start_matches("mut ")
                        .trim();
                    if bound.bytes().all(is_ident_byte) && is_binding_name(bound) {
                        names.insert(bound.to_string());
                    }
                }
            } else {
                let at = skip_keywords(code, open + 1);
                if let Some((name, end)) = ident_at(code, at)
                    && code[end..].trim_start().starts_with(')')
                    && is_binding_name(name)
                {
                    names.insert(name.to_string());
                }
            }
        }
        // `Some(x) = … .err()` within one statement.
        for (p, _) in code.match_indices(".err()") {
            if mask[p] {
                continue;
            }
            let statement_start = code[..p].rfind([';', '{', '}']).map_or(0, |s| s + 1);
            let statement = &code[statement_start..p];
            if let Some(some) = statement.find("Some(")
                && let Some((name, end)) = ident_at(statement, some + 5)
                && statement[end..].trim_start().starts_with(')')
                && is_binding_name(name)
            {
                names.insert(name.to_string());
            }
        }
        names
    }

    /// Every error-Display shape in `code` (see the section comment), as short site excerpts.
    fn error_display_sites(code: &str) -> Vec<String> {
        let bindings = error_bindings(code);
        let (_, mask) = lex(code);
        let b = code.as_bytes();
        let excerpt = |at: usize| -> String {
            let start = code[..at].rfind('\n').map_or(0, |p| p + 1);
            let end = code[at..].find('\n').map_or(code.len(), |p| at + p);
            code[start..end].trim().to_string()
        };
        let mut sites = Vec::new();
        if let Some(p) = code.find("ToString::to_string") {
            sites.push(excerpt(p));
        }
        // A binding in a format string: `{x}`, `{x:?}`, `{x:#}`, `{x:>8}`…
        for (p, _) in code.match_indices('{') {
            if !mask[p] {
                continue;
            }
            if let Some((name, end)) = ident_at(code, p + 1)
                && end == p + 1 + name.len()
                && matches!(b.get(end), Some(b'}') | Some(b':'))
                && bindings.contains(name)
            {
                sites.push(excerpt(p));
            }
        }
        // `x.to_string()`.
        for name in &bindings {
            let call = format!("{name}.to_string()");
            for (p, _) in code.match_indices(&call) {
                let bounded = p == 0 || !(is_ident_byte(b[p - 1]) || b[p - 1] == b'.');
                if bounded && !mask[p] {
                    sites.push(excerpt(p));
                }
            }
        }
        // A binding passed as a `format!` / `write!` ARGUMENT (multi-line calls, named args).
        for mac in ["format!(", "write!(", "writeln!("] {
            for (p, _) in code.match_indices(mac) {
                if mask[p] {
                    continue;
                }
                let open = p + mac.len() - 1;
                let end = group_end(code, &mask, open);
                let mut depth = 0i32;
                let mut arg_start = open + 1;
                let mut args = Vec::new();
                for i in open + 1..end {
                    if mask[i] {
                        continue;
                    }
                    match b[i] {
                        b'(' | b'[' | b'{' => depth += 1,
                        b')' | b']' | b'}' => depth -= 1,
                        b',' if depth == 0 => {
                            args.push(&code[arg_start..i]);
                            arg_start = i + 1;
                        }
                        _ => {}
                    }
                }
                args.push(&code[arg_start..end]);
                for arg in args.iter().skip(1) {
                    // `name = value` → the value.
                    let value = match arg.split_once('=') {
                        Some((lhs, rhs))
                            if lhs.trim().bytes().all(is_ident_byte) && !rhs.starts_with('=') =>
                        {
                            rhs
                        }
                        _ => arg,
                    };
                    let value = value.trim().trim_start_matches('&').trim();
                    if bindings.contains(value) {
                        sites.push(excerpt(p));
                    }
                }
            }
        }
        sites
    }

    #[test]
    fn no_error_display_reaches_a_user_string() {
        let mut offenders = Vec::new();
        let files = rust_sources("app");
        assert!(
            files.len() >= 40,
            "found only {} .rs files — scan broken?",
            files.len()
        );
        for file in files {
            let name = file.file_name().unwrap().to_string_lossy().into_owned();
            // The gate itself, and the test-only rail suite.
            if name == "posture.rs" || file.ends_with("state/tests.rs") {
                continue;
            }
            let source = std::fs::read_to_string(&file).expect("source readable");
            for site in error_display_sites(&user_reachable_code(&source)) {
                let allowed = ERROR_DISPLAY_ALLOW
                    .iter()
                    .any(|(f, snippet)| *f == name && site.contains(snippet));
                if !allowed {
                    offenders.push(format!("{}: {site}", file.display()));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "an error's Display can reach a user-visible string — name it in French (a MSG_*), \
             log the detail: {offenders:#?}"
        );
    }

    /// Every `Result<…, String>` in `code` — any signature (`pub`, `async`, `const`, trait
    /// methods) and any `type` alias alike.
    fn stringly_results(code: &str) -> Vec<String> {
        let (_, mask) = lex(code);
        let mut found = Vec::new();
        for (p, _) in code.match_indices("Result<") {
            if mask[p] || (p > 0 && is_ident_byte(code.as_bytes()[p - 1])) {
                continue;
            }
            let open = p + "Result".len();
            let close = group_end(code, &mask, open);
            let inner = &code[open + 1..close];
            let mut depth = 0i32;
            let mut start = 0;
            let mut args = Vec::new();
            for (i, c) in inner.char_indices() {
                match c {
                    '<' | '(' | '[' => depth += 1,
                    '>' | ')' | ']' => depth -= 1,
                    ',' if depth == 0 => {
                        args.push(inner[start..i].trim());
                        start = i + 1;
                    }
                    _ => {}
                }
            }
            args.push(inner[start..].trim());
            args.retain(|a| !a.is_empty()); // a trailing comma
            if args.len() == 2 && args[1] == "String" {
                found.push(
                    code[p..=close]
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "),
                );
            }
        }
        found
    }

    #[test]
    fn sibling_crates_hand_the_app_typed_errors_only() {
        // An English String built from an error inside another crate would bypass the app scan:
        // none of their code carries a stringly error — public or not, signature or alias.
        let mut offenders = Vec::new();
        for krate in ["persistence", "ingestion", "report", "core", "contract"] {
            for file in rust_sources(krate) {
                let source = std::fs::read_to_string(&file).expect("source readable");
                for site in stringly_results(&user_reachable_code(&source)) {
                    offenders.push(format!("{}: {site}", file.display()));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "a sibling crate returns a stringly error: {offenders:#?}"
        );
    }

    #[test]
    fn the_error_display_scan_sees_through_logging_and_tests_only() {
        let sites = |s: &str| error_display_sites(&user_reachable_code(s));
        // The classic shapes.
        assert_eq!(
            sites(r#"let m = format!("{MSG_SAVE_FAILED} {error}");"#).len(),
            1
        );
        assert_eq!(
            sites("journal.list().map_err(|e| e.to_string())?;").len(),
            1
        );
        assert_eq!(sites("x.map_err(ToString::to_string)?;").len(), 1);
        // Any binding name, from `Err(..)`, `map_err(|..|`, `unwrap_or_else(|..|`.
        assert_eq!(
            sites("match r { Err(why) => show(&why.to_string()), _ => {} }").len(),
            1
        );
        assert_eq!(sites("r.map_err(|failure| failure.to_string())").len(), 1);
        assert_eq!(sites("r.unwrap_or_else(|x| format!(\"{x:?}\"))").len(), 1);
        // Second G3 L8: `move` and spacing, `ref`, destructured fields, `.err()`.
        assert_eq!(sites("r.map_err(move |x| x.to_string())").len(), 1);
        assert_eq!(sites("r.map_err( | x | x.to_string())").len(), 1);
        assert_eq!(
            sites("match r { Err(ref x) => show(x.to_string()), _ => {} }").len(),
            1
        );
        assert_eq!(
            sites("match r { Err(E::Restore { detail }) => format!(\"{detail}\"), _ => {} }").len(),
            1
        );
        assert_eq!(
            sites("match r { Err(E::V { detail: why, .. }) => why.to_string(), _ => {} }").len(),
            1
        );
        assert_eq!(
            sites("if let Some(fault) = r.err() { show(fault.to_string()); }").len(),
            1
        );
        // Every hole shape and a named argument.
        assert_eq!(sites(r#"format!("{error:#}")"#).len(), 1);
        assert_eq!(sites(r#"format!("{e:>8}")"#).len(), 1);
        assert_eq!(sites(r#"format!("{msg}", msg = error)"#).len(), 1);
        // A multi-line `format!` argument.
        assert_eq!(
            sites("let s = format!(\n    \"{} {}\",\n    MSG,\n    error\n);").len(),
            1
        );
        // Logged, in a test module, or in a comment: fine — a lone `"` in a comment included.
        assert!(sites("tracing::warn!(\n    \"read failed: {error}\"\n);").is_empty());
        assert!(sites("tracing::warn!(\"a ) in a literal {}\", error);").is_empty());
        assert!(
            sites("#[cfg(test)]\nmod tests {\n    fn f() { let s = err.to_string(); }\n}\n")
                .is_empty()
        );
        assert!(sites("// format!(\"{error}\")\nlet a = 1;").is_empty());
        assert_eq!(
            sites("// a lone \" quote\nlet m = format!(\"{error}\");").len(),
            1,
            "a quote in a comment does not hide the next line"
        );
        assert!(sites("let c = '\"'; let s = \"{error}-free literal\";").len() == 1);
        // A French message handed on, a value built, a longer name: no error Display.
        assert!(sites("match r { Err(message) => refuse(&ui, &message), _ => {} }").is_empty());
        assert!(sites("return Err(invalid.to_string());").is_empty());
        assert!(sites("let k = base.to_string(); let v = self.e.to_string();").is_empty());
    }

    #[test]
    fn the_stringly_result_scan_sees_every_signature_shape() {
        let found = |s: &str| stringly_results(&user_reachable_code(s));
        assert_eq!(found("pub fn f() -> Result<u8, String> {}").len(), 1);
        assert_eq!(found("pub async fn f() -> Result<u8, String> {}").len(), 1);
        assert_eq!(found("pub const fn f() -> Result<u8, String> {}").len(), 1);
        assert_eq!(
            found("trait T { fn f(&self) -> Result<u8, String>; }").len(),
            1
        );
        assert_eq!(
            found("pub type Out<T> = std::result::Result<T, String>;").len(),
            1
        );
        assert_eq!(
            found("fn f() -> Result<\n    Vec<(u8, u8)>,\n    String,\n> {}").len(),
            1
        );
        assert!(found("fn f() -> Result<Vec<String>, Error> {}").is_empty());
        assert!(found("let v = r.get::<_, String>(0)?;").is_empty());
    }
}
