//! Crate-local banned-verb posture gate (the 1.9/1.10/1.11 local-gate pattern, FR13): every
//! user-visible string this crate introduces — the `@tr()` literals in `ui/**/*.slint` (read
//! from the files at test time) and the `labels.rs` display strings — is scanned against
//! `core::method::BANNED_VERBS_EN/FR` (reused, never copied). Zone labels are nouns naming the
//! defined price bands, not imperatives, so they pass the whole-word scan by construction.

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
        // Story 2.11 adds the "+ année" extend-projection affordance label (the annual roll-forward);
        // the user's entered data is never scanned. Keep the floor strict against a broken scan.
        assert!(
            total >= 163,
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
        // adds the normalize-failure notice (`MSG_NORMALIZE_FAILED`).
        assert_eq!(
            crate::state::USER_FACING_MESSAGES.len(),
            14,
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
