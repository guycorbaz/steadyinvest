//! Story 8.4 AC 6 / 7 / 16 / 17: the real binary over stdio on a temp `--dossier` — the eight
//! tools, every response naming the dossier, one draft of each kind recorded `pending` with the
//! studies untouched, an idempotent retry, one refusal per family with its §3.3 message, and the
//! named failures that touch nothing and leave the server serving.

mod support;

use serde_json::{Value, json};
use support::*;
use uuid::Uuid;

fn study_id(n: u128) -> String {
    Uuid::from_u128(n).to_string()
}

#[test]
fn the_eight_tools_read_and_record_propositions_without_touching_the_studies() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture_dossier(dir.path());
    let home = dir.path().join("home");
    let mut s = spawn(&["--dossier", path.to_str().unwrap()], &home);

    assert_eq!(
        s.tool_names(),
        vec![
            "list_studies",
            "get_study",
            "get_judgment_history",
            "get_notes",
            "get_drafts_record",
            "submit_draft_study",
            "submit_draft_note",
            "submit_draft_value",
        ]
    );

    let list = s.ok("list_studies", json!({}));
    assert_eq!(list["total"], json!(2));
    let dossier = list["dossier"].clone();
    assert_eq!(
        dossier["journal_id"],
        json!(Uuid::from_u128(JID).to_string())
    );
    assert!(
        dossier["path"].as_str().unwrap().ends_with("dossier.db"),
        "{dossier}"
    );

    let a_before = s.ok("get_study", json!({ "study_id": study_id(STUDY_A) }));
    assert_eq!(
        a_before["dossier"], dossier,
        "every response names the dossier"
    );
    assert_eq!(a_before["study"]["security_ticker"], json!("NESN.SW"));
    // O1: the computed outputs, from the app's own construction, in neutral codes.
    let computed = &a_before["computed"];
    assert!(computed.is_object(), "{a_before}");
    let state = computed["verdict_state"].as_str().unwrap();
    assert!(["full", "provisional", "withheld"].contains(&state));
    assert_eq!(
        computed["method_version"],
        json!(steadyinvest_core::METHOD_VERSION)
    );
    assert!(computed["inputs_hash"].as_str().unwrap().len() == 64);
    assert!(computed["quality_flags"].is_array());
    assert!(computed["open_gates"].is_array());
    let facts = &computed["verdict_facts"];
    for key in [
        "present_price_zone",
        "ud_at_or_above_target",
        "relative_value_below_ceiling",
        "present_price_in_low_zone",
        "appreciation_at_or_above_double",
        "quality_value_candidate",
    ] {
        assert!(
            facts.get(key).is_some(),
            "verdict fact {key} missing: {facts}"
        );
    }
    let computed_text = computed.to_string();
    for word in ["buy", "sell", "hold"] {
        assert!(
            !computed_text.to_lowercase().contains(word),
            "{word} in the computed outputs: {computed_text}"
        );
    }
    assert!(
        ["ratio", "undefined", "unknown"]
            .contains(&computed["upside_downside_state"].as_str().unwrap())
    );
    let b_before = s.ok("get_study", json!({ "study_id": study_id(STUDY_B) }));
    let hist = s.ok(
        "get_judgment_history",
        json!({ "study_id": study_id(STUDY_A) }),
    );
    assert_eq!(hist["total"], json!(1));
    let notes = s.ok("get_notes", json!({ "study_id": study_id(STUDY_A) }));
    assert_eq!(notes["notes"], json!([]));

    // One draft of each kind.
    let mut ids = Vec::new();
    for (tool, extra) in [
        (
            "submit_draft_study",
            json!({ "security_ticker": "TSLA", "native_currency": "USD", "company_name": "Tesla" }),
        ),
        (
            "submit_draft_note",
            json!({ "study_id": study_id(STUDY_A), "note_text": "Une note proposée." }),
        ),
        (
            "submit_draft_value",
            json!({ "study_id": study_id(STUDY_A), "field": "eps", "fiscal_year": 2025,
                    "proposed_value": "4.3" }),
        ),
        (
            "submit_draft_value",
            json!({ "study_id": study_id(STUDY_A), "field": "estimated_low_eps",
                    "proposed_value": "4.0" }),
        ),
        (
            "submit_draft_value",
            json!({ "study_id": study_id(STUDY_B), "field": "forecast_low_option",
                    "proposed_value": "recent_severe_low" }),
        ),
    ] {
        let body = s.ok(tool, submit_args(&dossier, extra));
        assert_eq!(body["status"], json!("pending"));
        assert_eq!(body["dossier"], dossier);
        ids.push(body["draft_id"].as_str().unwrap().to_string());
    }

    let record = s.ok("get_drafts_record", json!({}));
    assert_eq!(record["total"], json!(5));
    for d in record["drafts"].as_array().unwrap() {
        assert_eq!(d["status"], json!("pending"), "{d}");
        assert!(ids.contains(&d["id"].as_str().unwrap().to_string()));
    }
    let kinds: Vec<&str> = record["drafts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["kind"].as_str().unwrap())
        .collect();
    for k in ["study", "note", "cell", "judgment"] {
        assert!(kinds.contains(&k), "a {k} draft");
    }
    let only_b = s.ok(
        "get_drafts_record",
        json!({ "study_id": study_id(STUDY_B), "status": "pending" }),
    );
    assert_eq!(only_b["total"], json!(1));

    // The studies are unchanged (a pending draft changes nothing).
    let a_after = s.ok("get_study", json!({ "study_id": study_id(STUDY_A) }));
    let b_after = s.ok("get_study", json!({ "study_id": study_id(STUDY_B) }));
    assert_eq!(a_after["study"], a_before["study"]);
    assert_eq!(b_after["study"], b_before["study"]);
    assert_eq!(a_after["computed"], a_before["computed"]);

    // An idempotent retry: the same proposition with the same draft_id is recorded once.
    let retry_id = Uuid::from_u128(0x8499).to_string();
    let args = submit_args(
        &dossier,
        json!({ "study_id": study_id(STUDY_B), "note_text": "Retry", "draft_id": retry_id }),
    );
    let first = s.ok("submit_draft_note", args.clone());
    let second = s.ok("submit_draft_note", args.clone());
    assert_eq!(first["draft_id"], json!(retry_id));
    assert_eq!(second["draft_id"], json!(retry_id));
    assert_eq!(second["status"], json!("pending"));
    assert_eq!(s.ok("get_drafts_record", json!({}))["total"], json!(6));
    // The owner validates it (in the table, as the decision rail's outcome); a late retry answers
    // the STORED status, not « pending » (G3).
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE ai_drafts SET status = 'validated', decided_at = '2026-09-28T12:00:00Z' \
         WHERE id = ?1",
        [&retry_id],
    )
    .unwrap();
    drop(conn);
    let late = s.ok("submit_draft_note", args);
    assert_eq!(late["draft_id"], json!(retry_id));
    assert_eq!(late["status"], json!("validated"));
}

#[test]
fn each_refusal_family_returns_its_code_and_the_spec_message() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture_dossier(dir.path());
    let home = dir.path().join("home");
    let mut s = spawn(&["--dossier", path.to_str().unwrap()], &home);
    let dossier = s.ok("list_studies", json!({}))["dossier"].clone();

    // D10: the dossier the AI read is not this one.
    let mut other = dossier.clone();
    other["journal_id"] = json!(Uuid::from_u128(1).to_string());
    let body = s.refused(
        "submit_draft_note",
        submit_args(
            &other,
            json!({ "study_id": study_id(STUDY_A), "note_text": "x" }),
        ),
    );
    assert_eq!(body["code"], json!("dossier_mismatch"));
    // Same path, another journal: the message names the two journal ids (G3 decision).
    assert_eq!(
        body["message"],
        json!(format!(
            "Le dossier a changé depuis la lecture ({} ≠ {}) ; rien n'a été enregistré.",
            Uuid::from_u128(1),
            Uuid::from_u128(JID)
        ))
    );
    assert_eq!(body["dossier"], dossier, "a refusal names the dossier too");

    // D6: a market fact is not draftable.
    let body = s.refused(
        "submit_draft_value",
        submit_args(
            &dossier,
            json!({ "study_id": study_id(STUDY_A), "field": "current_price", "proposed_value": "1" }),
        ),
    );
    assert_eq!(body["code"], json!("field_not_draftable"));
    assert_eq!(
        body["message"],
        json!(
            "Le champ current_price ne peut pas être proposé (voir la liste des champs du schéma \
             de l'outil) ; rien n'a été enregistré."
        )
    );

    // NFR-A4: a blank comment.
    let mut args = submit_args(
        &dossier,
        json!({ "study_id": study_id(STUDY_A), "note_text": "x" }),
    );
    args["comment"] = json!("   ");
    let body = s.refused("submit_draft_note", args);
    assert_eq!(body["code"], json!("empty_comment"));
    assert_eq!(
        body["message"],
        json!("Le commentaire est obligatoire ; rien n'a été enregistré.")
    );

    // D2: the security is already studied in this currency.
    let body = s.refused(
        "submit_draft_study",
        submit_args(
            &dossier,
            json!({ "security_ticker": "NESN.SW", "native_currency": "CHF" }),
        ),
    );
    assert_eq!(body["code"], json!("study_exists"));
    assert_eq!(
        body["message"],
        json!("L'étude NESN.SW en CHF existe déjà ; rien n'a été enregistré.")
    );

    // A malformed call: an unknown argument, a cell field without its year.
    let body = s.refused("list_studies", json!({ "page": 2 }));
    assert_eq!(body["code"], json!("invalid_call"));
    let body = s.refused(
        "submit_draft_value",
        submit_args(
            &dossier,
            json!({ "study_id": study_id(STUDY_A), "field": "eps", "proposed_value": "1" }),
        ),
    );
    assert_eq!(body["code"], json!("invalid_call"));

    // Nothing was recorded by any refusal.
    assert_eq!(s.ok("get_drafts_record", json!({}))["total"], json!(0));
}

#[test]
fn a_missing_dossier_is_named_touches_nothing_and_the_server_keeps_serving() {
    let dir = tempfile::tempdir().unwrap();
    let later = dir.path().join("later");
    std::fs::create_dir_all(&later).unwrap();
    let path = later.join("dossier.db");
    let home = dir.path().join("home");
    let mut s = spawn(&["--dossier", path.to_str().unwrap()], &home);

    let body = s.refused("list_studies", json!({}));
    assert_eq!(body["code"], json!("no_dossier"));
    assert_eq!(body["dossier"], Value::Null);
    assert!(!path.exists(), "no file created at the resolved path");

    // The dossier appears: the same server now serves it.
    let built = fixture_dossier(&later);
    assert_eq!(built, path);
    let list = s.ok("list_studies", json!({}));
    assert_eq!(list["total"], json!(2));
}

#[test]
fn a_dossier_of_another_schema_is_refused_by_name_and_left_as_it_was() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture_dossier(dir.path());
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.pragma_update(None, "user_version", 7).unwrap();
    drop(conn);
    let home = dir.path().join("home");
    let mut s = spawn(&["--dossier", path.to_str().unwrap()], &home);
    let body = s.refused("list_studies", json!({}));
    assert_eq!(body["code"], json!("schema_mismatch"));
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("ouvrez-le d'abord dans l'application"),
        "{body}"
    );
    let conn = rusqlite::Connection::open(&path).unwrap();
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 7, "never migrated");
}

#[test]
fn the_server_advertises_tools_only() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture_dossier(dir.path());
    let home = dir.path().join("home");
    let mut s = spawn(&["--dossier", path.to_str().unwrap()], &home);
    let r = s.request(
        "initialize",
        json!({ "protocolVersion": "2025-06-18", "capabilities": {},
                "clientInfo": { "name": "t", "version": "0" } }),
    );
    let caps = r["result"]["capabilities"]
        .as_object()
        .unwrap_or_else(|| panic!("{r}"));
    let keys: Vec<&String> = caps.keys().collect();
    assert_eq!(keys, vec!["tools"], "{r}");
}

#[test]
fn bounds_unknown_studies_and_long_values_are_named() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture_dossier(dir.path());
    let home = dir.path().join("home");
    let mut s = spawn(&["--dossier", path.to_str().unwrap()], &home);
    let dossier = s.ok("list_studies", json!({}))["dossier"].clone();
    for (tool, args) in [
        ("list_studies", json!({ "limit": 201 })),
        ("get_drafts_record", json!({ "limit": 0 })),
        (
            "get_judgment_history",
            json!({ "study_id": study_id(STUDY_A), "limit": 51 }),
        ),
    ] {
        assert_eq!(
            s.refused(tool, args)["code"],
            json!("invalid_call"),
            "{tool}"
        );
    }
    s.ok(
        "get_judgment_history",
        json!({ "study_id": study_id(STUDY_A), "limit": 50 }),
    );
    let unknown = Uuid::from_u128(0xdead).to_string();
    for (tool, args) in [
        ("get_judgment_history", json!({ "study_id": unknown })),
        ("get_drafts_record", json!({ "study_id": unknown })),
        ("get_notes", json!({ "study_id": unknown })),
    ] {
        let body = s.refused(tool, args);
        assert_eq!(body["code"], json!("study_not_found"), "{tool}");
        assert_eq!(body["dossier"], dossier, "{tool} names the dossier");
    }
    let body = s.refused(
        "submit_draft_value",
        submit_args(
            &dossier,
            json!({ "study_id": study_id(STUDY_A), "field": "estimated_low_eps",
                    "proposed_value": "9".repeat(101) }),
        ),
    );
    assert_eq!(body["code"], json!("text_too_long"));
    assert_eq!(
        body["message"],
        json!(
            "Le texte de la valeur proposée dépasse 100 caractères (101) ; rien n'a été enregistré."
        )
    );
}

/// G3: AI-written text never reaches the log raw (a newline would forge a line); neither does a
/// client-sent method or tool name (the SDK's own events are kept to errors).
#[test]
fn no_ai_or_client_text_reaches_the_log() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture_dossier(dir.path());
    let home = dir.path().join("home");
    let mut s = spawn(&["--dossier", path.to_str().unwrap()], &home);
    let dossier = s.ok("list_studies", json!({}))["dossier"].clone();
    let forged = "x\nFORGED-LOG-LINE level=ERROR";
    let mut args = submit_args(
        &dossier,
        json!({ "study_id": study_id(STUDY_A), "field": "estimated_low_eps",
                "proposed_value": forged }),
    );
    args["comment"] = json!(forged);
    let refused = s.refused("submit_draft_value", args);
    assert_eq!(refused["code"], json!("value_unparsable"));
    let mut note = submit_args(
        &dossier,
        json!({ "study_id": study_id(STUDY_A), "note_text": forged }),
    );
    note["comment"] = json!(forged);
    note["origin_model"] = json!(forged);
    s.ok("submit_draft_note", note);
    s.send(&json!({ "jsonrpc": "2.0", "method": "notifications/CLIENT-MARKER-METHOD" }));
    let r = s.request(
        "tools/call",
        json!({ "name": "CLIENT-MARKER-TOOL", "arguments": {} }),
    );
    assert!(r["error"].is_object(), "{r}");
    assert!(
        !r.to_string().contains("CLIENT-MARKER"),
        "the name is echoed: {r}"
    );
    s.ok("list_studies", json!({}));
    s.stop();
    let log = log_text(&home);
    assert!(log.contains("tool call"), "the log is written: {log}");
    assert!(!log.contains("FORGED"), "AI text reached the log:\n{log}");
    assert!(
        !log.contains("CLIENT-MARKER"),
        "a client name reached the log:\n{log}"
    );
}

/// G3: a log directory the server cannot write → it runs without a file and still serves.
#[cfg(unix)]
#[test]
fn an_unwritable_log_directory_does_not_stop_the_server() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = fixture_dossier(dir.path());
    let home = dir.path().join("home");
    let logs = home.join("data").join("steadyinvest").join("logs");
    std::fs::create_dir_all(&logs).unwrap();
    std::fs::set_permissions(&logs, std::fs::Permissions::from_mode(0o555)).unwrap();
    let writable = std::fs::File::create(logs.join(".probe")).is_ok();
    if !writable {
        let mut s = spawn(&["--dossier", path.to_str().unwrap()], &home);
        assert_eq!(s.ok("list_studies", json!({}))["total"], json!(2));
    }
    std::fs::set_permissions(&logs, std::fs::Permissions::from_mode(0o755)).unwrap();
}
