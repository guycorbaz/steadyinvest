//! Story 8.4 AC 14 (NFR-S4, NFR-A2 — the Story 8.3 suite re-run over stdio): with unique markers in
//! every portfolio / watchlist / cache table and a marked app-config beside the server, no stdout
//! byte of any tool on any page carries a marker, a key-shaped value or a configuration value —
//! the resolved dossier identity excepted.

mod support;

use serde_json::json;
use support::*;
use uuid::Uuid;

#[test]
fn no_tool_on_any_page_exposes_portfolio_watchlist_cache_or_config_data() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture_dossier(dir.path());
    let markers = seed_markers(&path);
    assert!(markers.len() >= 20, "every denied table carried markers");

    // A marked app-config where the server would look for one (it must never echo it).
    let home = dir.path().join("home");
    let config_dir = home.join("config").join("steadyinvest");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("config.json"),
        r#"{"reference_currency":"CONFIG-MARKER-CUR","active_portfolio_id":"CONFIG-MARKER-PF",
            "price_fallback_provider":"CONFIG-MARKER-PROVIDER"}"#,
    )
    .unwrap();

    let mut s = spawn(&["--dossier", path.to_str().unwrap()], &home);
    let dossier = s.ok("list_studies", json!({}))["dossier"].clone();
    // Drafts of every kind, so the record has content to page through.
    for (tool, extra) in [
        (
            "submit_draft_study",
            json!({ "security_ticker": "MSFT", "native_currency": "USD" }),
        ),
        (
            "submit_draft_note",
            json!({ "study_id": Uuid::from_u128(STUDY_A).to_string(), "note_text": "n" }),
        ),
        (
            "submit_draft_value",
            json!({ "study_id": Uuid::from_u128(STUDY_B).to_string(), "field": "sales",
                    "fiscal_year": 2024, "proposed_value": "950" }),
        ),
    ] {
        s.ok(tool, submit_args(&dossier, extra));
    }

    // Every tool, every page (limit 1 forces paging).
    let mut offset = 0;
    loop {
        let page = s.ok("list_studies", json!({ "offset": offset, "limit": 1 }));
        let items = page["studies"].as_array().unwrap().len();
        if items == 0 {
            break;
        }
        offset += 1;
    }
    assert_eq!(offset, 2);
    for id in [STUDY_A, STUDY_B] {
        let id = Uuid::from_u128(id).to_string();
        s.ok("get_study", json!({ "study_id": id }));
        s.ok("get_notes", json!({ "study_id": id }));
        let mut offset = 0;
        loop {
            let page = s.ok(
                "get_judgment_history",
                json!({ "study_id": id, "offset": offset, "limit": 1 }),
            );
            if page["snapshots"].as_array().unwrap().is_empty() {
                break;
            }
            offset += 1;
        }
    }
    let mut offset = 0;
    loop {
        let page = s.ok("get_drafts_record", json!({ "offset": offset, "limit": 1 }));
        if page["drafts"].as_array().unwrap().is_empty() {
            break;
        }
        offset += 1;
    }
    assert_eq!(offset, 3);
    // A refusal too (its body is output as well).
    s.refused(
        "get_study",
        json!({ "study_id": Uuid::from_u128(9).to_string() }),
    );

    let transcript = s.transcript.join("\n");
    assert!(transcript.contains("NESN.SW"), "the studies are read");
    for marker in &markers {
        assert!(!transcript.contains(marker), "{marker} leaked over stdio");
    }
    assert!(
        !transcript.contains("CONFIG-MARKER"),
        "an app-config value leaked over stdio"
    );
    // No key-shaped value: nothing named like a secret, nothing from the keychain service.
    for needle in ["api_key", "apiKey", "secret", "password", "provider:eodhd"] {
        assert!(!transcript.contains(needle), "{needle} over stdio");
    }
}
