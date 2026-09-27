//! Story 8.2a — the `ai_drafts` table (migration v8): the engine's CHECKs and trigger, the typed
//! read, the export/import round-trip, the O7 cascade, the backup, and the computation isolation.
//!
//! There is no writer API before Story 8.3, so drafts are inserted by **raw SQL** on the dossier
//! file — which exercises the CHECKs and the trigger exactly as the MCP draft connection will.

use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
use steadyinvest_contract::{
    DraftKind, DraftStatus, ForecastLowOption, Judgment, Money, SCHEMA_VERSION, Study, Timestamp,
};
use steadyinvest_persistence::{DraftRecord, Error, Journal};
use tempfile::TempDir;
use uuid::Uuid;

fn ts(s: &str) -> Timestamp {
    Timestamp(s.to_string())
}

fn money(s: &str) -> Money {
    serde_json::from_str(&format!("\"{s}\"")).expect("decimal parses")
}

fn study(id: u128, journal_id: Uuid, ticker: &str) -> Study {
    Study {
        id: Uuid::from_u128(id),
        journal_id,
        security_ticker: ticker.to_string(),
        native_currency: "CHF".to_string(),
        years: Vec::new(),
        judgment: Judgment {
            ai_placed: Default::default(),
            estimated_high_eps: Some(money("5.20")),
            estimated_low_eps: None,
            projected_sales_growth_pct: None,
            projected_eps_growth_pct: None,
            judged_avg_high_pe: None,
            judged_avg_low_pe: None,
            forecast_low_option: ForecastLowOption::AvgLowPriceLast5y,
            recent_severe_low: None,
            current_price: None,
            present_full_year_dividend: None,
            ttm_eps: None,
        },
        rationale: None,
        company_name: None,
        notes: Vec::new(),
        created_at: ts("2026-09-27T08:00:00Z"),
        schema_version: SCHEMA_VERSION,
    }
}

const JID: u128 = 0x8200;
const STUDY_A: u128 = 0x8A;
const STUDY_B: u128 = 0x8B;

/// A dossier with two studies (A, B), closed-free: the journal stays open, raw connections write
/// beside it (WAL).
fn dossier() -> (TempDir, PathBuf, Journal) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("dossier.db");
    let jid = Uuid::from_u128(JID);
    let mut journal = Journal::create(&path, jid, &ts("2026-09-27T07:00:00Z")).expect("creates");
    journal.put_study(&study(STUDY_A, jid, "NESN")).expect("A");
    journal.put_study(&study(STUDY_B, jid, "ROG")).expect("B");
    (dir, path, journal)
}

fn raw(path: &Path) -> Connection {
    let conn = Connection::open(path).expect("raw open");
    conn.pragma_update(None, "foreign_keys", true)
        .expect("foreign keys on");
    conn
}

fn uuid_text(n: u128) -> String {
    Uuid::from_u128(n).to_string()
}

/// One row to insert — every column, as text/integers, so each test states exactly what it plants.
#[derive(Clone)]
struct Row {
    id: String,
    kind: &'static str,
    study_id: Option<String>,
    ticker: &'static str,
    currency: Option<&'static str>,
    status: &'static str,
    created_at: &'static str,
    decided_at: Option<&'static str>,
    comment: Option<&'static str>,
    client: Option<&'static str>,
    model: Option<&'static str>,
    stale: Option<i64>,
    edited: Option<i64>,
    created_study_id: Option<String>,
    payload: String,
}

const NOTE_PAYLOAD: &str = r#"{"version":1,"note_text":"Marge en hausse."}"#;
const CELL_PAYLOAD: &str = r#"{"version":1,"target":{"target":"cell","fiscal_year":2024,"field":"eps"},"proposed_value":"3.15"}"#;
const JUDGMENT_PAYLOAD: &str = r#"{"version":1,"target":{"target":"judgment","field":"judged_avg_low_pe"},"proposed_value":"11.5"}"#;
const STUDY_PAYLOAD: &str = r#"{"version":1,"company_name":"ASML Holding"}"#;

fn note_row(n: u128) -> Row {
    Row {
        id: uuid_text(n),
        kind: "note",
        study_id: Some(uuid_text(STUDY_A)),
        ticker: "NESN",
        currency: None,
        status: "pending",
        created_at: "2026-09-27T09:00:00Z",
        decided_at: None,
        comment: Some("La marge progresse depuis trois ans."),
        client: Some("claude-code"),
        model: Some("claude-opus-5-5"),
        stale: None,
        edited: None,
        created_study_id: None,
        payload: NOTE_PAYLOAD.to_string(),
    }
}

fn study_row(n: u128) -> Row {
    Row {
        id: uuid_text(n),
        kind: "study",
        study_id: None,
        ticker: "ASML",
        currency: Some("EUR"),
        payload: STUDY_PAYLOAD.to_string(),
        ..note_row(n)
    }
}

fn cell_row(n: u128) -> Row {
    Row {
        kind: "cell",
        payload: CELL_PAYLOAD.to_string(),
        ..note_row(n)
    }
}

fn judgment_row(n: u128) -> Row {
    Row {
        kind: "judgment",
        payload: JUDGMENT_PAYLOAD.to_string(),
        ..note_row(n)
    }
}

fn insert(conn: &Connection, r: &Row) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT INTO ai_drafts
             (id, kind, study_id, security_ticker, native_currency, status, created_at, decided_at,
              comment, origin_client, origin_model, stale_at_decision, edited_before_validation,
              created_study_id, payload)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            r.id,
            r.kind,
            r.study_id,
            r.ticker,
            r.currency,
            r.status,
            r.created_at,
            r.decided_at,
            r.comment,
            r.client,
            r.model,
            r.stale,
            r.edited,
            r.created_study_id,
            r.payload
        ],
    )
}

fn draft_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT count(*) FROM ai_drafts", [], |r| r.get(0))
        .expect("count reads")
}

fn is_constraint(e: &rusqlite::Error) -> bool {
    matches!(e, rusqlite::Error::SqliteFailure(f, _)
        if f.code == rusqlite::ErrorCode::ConstraintViolation)
}

// ── AC 3 / 4 — the engine refuses every malformed row; the trigger bumps once per insert ──

#[test]
fn every_check_refuses_its_row_and_writes_nothing() {
    let (_dir, path, _journal) = dossier();
    let conn = raw(&path);
    let decided = |r: Row| Row {
        status: "validated",
        decided_at: Some("2026-09-27T10:00:00Z"),
        ..r
    };
    let cases: Vec<(&str, Row)> = vec![
        (
            "blank comment",
            Row {
                comment: Some("   "),
                ..note_row(1)
            },
        ),
        (
            "NULL comment",
            Row {
                comment: None,
                ..note_row(2)
            },
        ),
        (
            "empty origin_client",
            Row {
                client: Some(""),
                ..note_row(3)
            },
        ),
        (
            "NULL origin_model",
            Row {
                model: None,
                ..note_row(4)
            },
        ),
        (
            "unknown kind",
            Row {
                kind: "proposal",
                ..note_row(5)
            },
        ),
        (
            "unknown status",
            Row {
                status: "stale",
                ..note_row(6)
            },
        ),
        (
            "draft study with a study_id",
            Row {
                study_id: Some(uuid_text(STUDY_A)),
                ..study_row(7)
            },
        ),
        (
            "note without study_id",
            Row {
                study_id: None,
                ..note_row(8)
            },
        ),
        (
            "draft study without currency",
            Row {
                currency: None,
                ..study_row(9)
            },
        ),
        (
            "cell with a currency",
            Row {
                currency: Some("CHF"),
                ..cell_row(10)
            },
        ),
        (
            "pending with decided_at",
            Row {
                decided_at: Some("2026-09-27T10:00:00Z"),
                ..note_row(11)
            },
        ),
        (
            "rejected without decided_at",
            Row {
                status: "rejected",
                ..note_row(12)
            },
        ),
        (
            "created_study_id on a note",
            Row {
                created_study_id: Some(uuid_text(STUDY_B)),
                ..decided(note_row(13))
            },
        ),
        (
            "created_study_id on a pending draft study",
            Row {
                created_study_id: Some(uuid_text(STUDY_B)),
                ..study_row(14)
            },
        ),
        (
            "stale_at_decision = 2",
            Row {
                stale: Some(2),
                ..decided(note_row(15))
            },
        ),
        (
            "edited_before_validation = -1",
            Row {
                edited: Some(-1),
                ..decided(note_row(16))
            },
        ),
        (
            "validated draft study without created_study_id",
            decided(study_row(18)),
        ),
        (
            "validated_undone draft study (not undoable, arch A8)",
            Row {
                status: "validated_undone",
                decided_at: Some("2026-09-27T10:00:00Z"),
                created_study_id: Some(uuid_text(STUDY_B)),
                ..study_row(19)
            },
        ),
        (
            "rejected draft study with created_study_id",
            Row {
                status: "rejected",
                decided_at: Some("2026-09-27T10:00:00Z"),
                created_study_id: Some(uuid_text(STUDY_B)),
                ..study_row(20)
            },
        ),
        (
            "pending with stale_at_decision",
            Row {
                stale: Some(0),
                ..note_row(21)
            },
        ),
        (
            "pending with edited_before_validation",
            Row {
                edited: Some(1),
                ..note_row(22)
            },
        ),
        (
            "rejected with edited_before_validation",
            Row {
                status: "rejected",
                decided_at: Some("2026-09-27T10:00:00Z"),
                edited: Some(0),
                ..note_row(23)
            },
        ),
        (
            "upper-case id",
            Row {
                id: uuid_text(0xABC24).to_uppercase(),
                ..note_row(24)
            },
        ),
        (
            "short id",
            Row {
                id: "draft-25".to_string(),
                ..note_row(25)
            },
        ),
        (
            "upper-case study_id",
            Row {
                study_id: Some(uuid_text(STUDY_A).to_uppercase()),
                ..note_row(26)
            },
        ),
        (
            "study_id of an absent study (FK)",
            Row {
                study_id: Some(uuid_text(0xDEAD)),
                ..note_row(17)
            },
        ),
    ];
    for (what, row) in cases {
        let err = insert(&conn, &row).expect_err(what);
        assert!(
            is_constraint(&err),
            "{what}: expected a constraint error, got {err:?}"
        );
    }
    assert_eq!(draft_count(&conn), 0, "no refused row was written");
}

#[test]
fn a_valid_draft_of_each_kind_inserts_and_bumps_the_version_by_one() {
    let (_dir, path, journal) = dossier();
    let conn = raw(&path);
    let decided_study = Row {
        status: "validated",
        decided_at: Some("2026-09-27T10:00:00Z"),
        stale: Some(0),
        created_study_id: Some(uuid_text(STUDY_B)),
        ..study_row(0x50)
    };
    for row in [
        study_row(0x10),
        note_row(0x20),
        cell_row(0x30),
        judgment_row(0x40),
        decided_study,
    ] {
        let before = journal.logical_version().expect("reads");
        insert(&conn, &row).expect("a valid draft inserts");
        assert_eq!(
            journal.logical_version().expect("reads"),
            before + 1,
            "trg_ai_drafts_bump_logical_version bumps exactly once for a {} draft",
            row.kind
        );
    }
    assert_eq!(draft_count(&conn), 5);
}

// ── Typed read ──

#[test]
fn list_drafts_reads_every_column_typed() {
    let (_dir, path, journal) = dossier();
    let conn = raw(&path);
    let rejected = Row {
        status: "validated",
        decided_at: Some("2026-09-27T11:00:00Z"),
        stale: Some(1),
        edited: Some(0),
        created_at: "2026-09-27T09:30:00Z",
        ..cell_row(0x21)
    };
    insert(&conn, &note_row(0x20)).expect("inserts");
    insert(&conn, &rejected).expect("inserts");
    let drafts = journal.list_drafts().expect("reads");
    assert_eq!(drafts.len(), 2);
    assert_eq!(
        drafts[1],
        DraftRecord {
            id: Uuid::from_u128(0x21),
            kind: DraftKind::Cell,
            study_id: Some(Uuid::from_u128(STUDY_A)),
            security_ticker: "NESN".to_string(),
            native_currency: None,
            status: DraftStatus::Validated,
            created_at: ts("2026-09-27T09:30:00Z"),
            decided_at: Some(ts("2026-09-27T11:00:00Z")),
            comment: "La marge progresse depuis trois ans.".to_string(),
            origin_client: "claude-code".to_string(),
            origin_model: "claude-opus-5-5".to_string(),
            stale_at_decision: Some(true),
            edited_before_validation: Some(false),
            created_study_id: None,
            payload: CELL_PAYLOAD.to_string(),
        }
    );
    assert_eq!(drafts[0].kind, DraftKind::Note, "ordered by created_at");
}

#[test]
fn a_draft_whose_payload_does_not_fit_fails_the_read_by_name() {
    // The table's CHECKs cannot see inside the JSON: a version-0, wrong-shape or unknown-key payload is
    // caught at the read, loudly — never skipped.
    for payload in [
        r#"{"version":0,"note_text":"x"}"#.to_string(),
        CELL_PAYLOAD.to_string(), // a cell-shaped payload on a note row
        r#"{"version":1,"note_text":"x","extra":1}"#.to_string(), // an unknown key
        "not json".to_string(),
    ] {
        let (_dir, path, journal) = dossier();
        let conn = raw(&path);
        insert(
            &conn,
            &Row {
                payload,
                ..note_row(0x20)
            },
        )
        .expect("the table accepts the text");
        match journal.list_drafts() {
            Err(Error::CorruptPayload { detail }) => {
                assert!(
                    detail.contains("ai_drafts.payload"),
                    "names the column: {detail}"
                )
            }
            other => panic!("expected CorruptPayload, got {other:?}"),
        }
    }
}

// ── AC 7 — export / import ──

/// A dossier holding drafts in every kind and every status, every nullable column exercised.
fn plant_every_draft(path: &Path) {
    let conn = raw(path);
    let decided = |r: Row, status: &'static str| Row {
        status,
        decided_at: Some("2026-09-27T12:00:00Z"),
        ..r
    };
    let rows = vec![
        study_row(0x101),
        Row {
            created_study_id: Some(uuid_text(STUDY_B)),
            stale: Some(0),
            ..decided(study_row(0x102), "validated")
        },
        Row {
            edited: Some(0),
            ..decided(note_row(0x203), "validated_undone")
        },
        decided(study_row(0x104), "rejected"),
        note_row(0x201),
        Row {
            edited: Some(1),
            ..decided(note_row(0x202), "validated")
        },
        cell_row(0x301),
        Row {
            stale: Some(1),
            edited: Some(0),
            ..decided(cell_row(0x302), "validated_undone")
        },
        judgment_row(0x401),
        Row {
            stale: Some(0),
            ..decided(judgment_row(0x402), "rejected")
        },
    ];
    for (i, mut row) in rows.into_iter().enumerate() {
        // Distinct creation times keep the order deterministic and visible.
        row.created_at = [
            "2026-09-27T09:00:01Z",
            "2026-09-27T09:00:02Z",
            "2026-09-27T09:00:03Z",
            "2026-09-27T09:00:04Z",
            "2026-09-27T09:00:05Z",
            "2026-09-27T09:00:06Z",
            "2026-09-27T09:00:07Z",
            "2026-09-27T09:00:08Z",
            "2026-09-27T09:00:09Z",
            "2026-09-27T09:00:10Z",
        ][i];
        insert(&conn, &row).expect("a valid planted draft");
    }
}

#[test]
fn drafts_round_trip_through_export_and_import_byte_for_byte() {
    let (_dir, path, journal) = dossier();
    plant_every_draft(&path);
    let before = journal.list_drafts().expect("reads");
    assert_eq!(before.len(), 10);
    let exported = journal.export_journal().expect("exports");
    assert!(exported.contains("ai_drafts"), "the array travels");

    // Into a fresh, empty dossier (a foreign seed): the file carries the studies too.
    let dir2 = TempDir::new().expect("tempdir");
    let mut target = Journal::create(
        dir2.path().join("target.db"),
        Uuid::from_u128(0x9999),
        &ts("2026-09-27T07:00:00Z"),
    )
    .expect("creates");
    let summary = target.import_journal(&exported).expect("imports");
    assert_eq!(summary.ai_drafts, 10);
    assert_eq!(
        target.list_drafts().expect("reads"),
        before,
        "every column crossed"
    );

    // Re-import is idempotent: no duplicate, same rows.
    target.import_journal(&exported).expect("re-imports");
    assert_eq!(target.list_drafts().expect("reads"), before);
}

#[test]
fn a_draftless_dossier_exports_without_the_array() {
    let (_dir, _path, journal) = dossier();
    let exported = journal.export_journal().expect("exports");
    assert!(
        !exported.contains("ai_drafts"),
        "an empty draft set exports WITHOUT the key (the #78 rail)"
    );
}

/// Rewrite an export's payload through a JSON edit, re-hashing it — to build files the app would
/// not write.
fn edited_export(exported: &str, edit: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut envelope: serde_json::Value = serde_json::from_str(exported).expect("envelope");
    let mut snapshot: serde_json::Value =
        serde_json::from_str(envelope["payload"].as_str().expect("payload")).expect("snapshot");
    edit(&mut snapshot);
    let payload = serde_json::to_string(&snapshot).expect("serializes");
    envelope["integrity_hash"] =
        serde_json::Value::String(steadyinvest_contract::sha256_hex(payload.as_bytes()));
    envelope["payload"] = serde_json::Value::String(payload);
    serde_json::to_string(&envelope).expect("serializes")
}

#[test]
fn a_draft_about_a_study_neither_in_the_file_nor_in_the_dossier_refuses_the_whole_import() {
    let (_dir, path, journal) = dossier();
    insert(&raw(&path), &note_row(0x20)).expect("plants");
    // Drop the studies from the file: the draft now points at nothing the target knows.
    let exported = edited_export(&journal.export_journal().expect("exports"), |s| {
        s["studies"] = serde_json::json!([]);
    });
    let dir2 = TempDir::new().expect("tempdir");
    let mut target = Journal::create(
        dir2.path().join("target.db"),
        Uuid::from_u128(0x9999),
        &ts("2026-09-27T07:00:00Z"),
    )
    .expect("creates");
    let version = target.logical_version().expect("reads");
    match target.import_journal(&exported) {
        Err(Error::ImportMalformed { detail }) => {
            assert!(detail.contains("draft"), "{detail}")
        }
        other => panic!("expected ImportMalformed, got {other:?}"),
    }
    assert!(
        target.list_drafts().expect("reads").is_empty(),
        "nothing applied"
    );
    assert_eq!(target.logical_version().expect("reads"), version, "no bump");
}

#[test]
fn a_draft_about_a_study_already_in_the_dossier_imports_without_it() {
    // The dossier re-imports its own drafts from a file that carries no study: the referenced
    // studies are already here.
    let (_dir, path, mut journal) = dossier();
    insert(&raw(&path), &note_row(0x20)).expect("plants");
    let exported = edited_export(&journal.export_journal().expect("exports"), |s| {
        s["studies"] = serde_json::json!([]);
        s["ai_drafts"][0]["comment"] = serde_json::json!("Commentaire revu.");
    });
    let summary = journal.import_journal(&exported).expect("imports");
    assert_eq!(summary.ai_drafts, 1);
    assert_eq!(
        journal.list_drafts().expect("reads")[0].comment,
        "Commentaire revu.",
        "upserted by id"
    );
}

/// One edit of an exported snapshot's JSON.
type SnapshotEdit = dyn Fn(&mut serde_json::Value);

#[test]
fn an_imported_draft_that_breaks_a_rule_is_malformed_and_nothing_is_applied() {
    let (_dir, path, journal) = dossier();
    insert(&raw(&path), &note_row(0x20)).expect("plants");
    let exported = journal.export_journal().expect("exports");
    let dir2 = TempDir::new().expect("tempdir");
    let mut target = Journal::create(
        dir2.path().join("target.db"),
        Uuid::from_u128(0x9999),
        &ts("2026-09-27T07:00:00Z"),
    )
    .expect("creates");
    let edits: Vec<Box<SnapshotEdit>> = vec![
        // A payload of an unusable version (0 — corrupt, not newer).
        Box::new(|s| s["ai_drafts"][0]["payload"] = serde_json::json!(r#"{"version":0}"#)),
        // A pending draft with a decision date (a table CHECK).
        Box::new(|s| s["ai_drafts"][0]["decided_at"] = serde_json::json!("2026-09-27T12:00:00Z")),
        // A blank comment (a table CHECK).
        Box::new(|s| s["ai_drafts"][0]["comment"] = serde_json::json!("  ")),
    ];
    for edit in edits {
        let file = edited_export(&exported, |s| edit(s));
        match target.import_journal(&file) {
            Err(Error::ImportMalformed { .. }) => {}
            other => panic!("expected ImportMalformed, got {other:?}"),
        }
        assert!(
            target.list_studies().expect("reads").is_empty(),
            "all-or-nothing"
        );
        assert!(target.list_drafts().expect("reads").is_empty());
    }
}

// ── AC 8 — cascade (O7) ──

#[test]
fn deleting_a_study_deletes_exactly_its_drafts() {
    let (_dir, path, mut journal) = dossier();
    let conn = raw(&path);
    let decided = |r: Row, status: &'static str| Row {
        status,
        decided_at: Some("2026-09-27T12:00:00Z"),
        ..r
    };
    // About A: one of each targeting kind, several statuses.
    insert(&conn, &note_row(0xA1)).expect("plants");
    insert(&conn, &decided(cell_row(0xA2), "validated")).expect("plants");
    insert(&conn, &decided(judgment_row(0xA3), "rejected")).expect("plants");
    // The validated draft study A came from.
    insert(
        &conn,
        &Row {
            created_study_id: Some(uuid_text(STUDY_A)),
            ..decided(study_row(0xA4), "validated")
        },
    )
    .expect("plants");
    // Kept: a pending draft study, and drafts about B.
    insert(&conn, &study_row(0xC1)).expect("plants");
    insert(
        &conn,
        &Row {
            study_id: Some(uuid_text(STUDY_B)),
            ticker: "ROG",
            ..note_row(0xB1)
        },
    )
    .expect("plants");
    insert(
        &conn,
        &Row {
            created_study_id: Some(uuid_text(STUDY_B)),
            ..decided(study_row(0xB2), "validated")
        },
    )
    .expect("plants");

    let before = journal.logical_version().expect("reads");
    journal
        .delete_study(Uuid::from_u128(STUDY_A))
        .expect("deletes");
    let kept: Vec<Uuid> = journal
        .list_drafts()
        .expect("reads")
        .iter()
        .map(|d| d.id)
        .collect();
    let mut kept_sorted = kept.clone();
    kept_sorted.sort();
    assert_eq!(
        kept_sorted,
        vec![
            Uuid::from_u128(0xB1),
            Uuid::from_u128(0xB2),
            Uuid::from_u128(0xC1)
        ],
        "A's drafts (study_id OR created_study_id) went; B's and the pending draft study stayed"
    );
    assert_eq!(
        journal.logical_version().expect("reads"),
        before + 1,
        "one bump"
    );

    // An absent id stays a true no-op.
    let v = journal.logical_version().expect("reads");
    journal
        .delete_study(Uuid::from_u128(0xDEAD))
        .expect("no-op");
    assert_eq!(
        journal.logical_version().expect("reads"),
        v,
        "no phantom bump"
    );
}

// ── AC 6 — backup carries the rows and the trigger ──

#[test]
fn a_backup_carries_the_drafts_and_the_trigger() {
    let (dir, path, journal) = dossier();
    plant_every_draft(&path);
    let dest = dir.path().join("backup.db");
    journal.backup_to(&dest).expect("backs up");
    let before = journal.list_drafts().expect("reads");
    drop(journal);

    let conn = Connection::open(&dest).expect("opens the backup");
    let trigger: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master
             WHERE type = 'trigger' AND name = 'trg_ai_drafts_bump_logical_version'",
            [],
            |r| r.get(0),
        )
        .expect("reads");
    assert_eq!(trigger, 1, "the trigger travels with the backup");
    drop(conn);
    let restored = Journal::open(&dest).expect("the backup opens as a dossier");
    assert_eq!(restored.list_drafts().expect("reads"), before);
}

// ── AC 9 — no computation path reads the drafts ──

fn read_tree(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
    for entry in std::fs::read_dir(dir).expect("dir reads") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            read_tree(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let text = std::fs::read_to_string(&path).expect("source reads");
            out.push((path, text));
        }
    }
}

#[test]
fn no_computation_path_reads_the_drafts() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    for krate in ["core", "report"] {
        let manifest =
            std::fs::read_to_string(root.join(krate).join("Cargo.toml")).expect("manifest reads");
        // The package name covers a renamed dependency too (`x = { package = "steadyinvest-
        // persistence" }`); the path covers a dependency declared by path under any name.
        assert!(
            !manifest.contains("steadyinvest-persistence") && !manifest.contains("../persistence"),
            "{krate} must not depend on persistence — a pending draft changes no computed output"
        );
    }
    let mut sources = Vec::new();
    read_tree(&root.join("core/src"), &mut sources);
    read_tree(&root.join("report/src"), &mut sources);
    for entry in std::fs::read_dir(root.join("app/src/viewmodel")).expect("viewmodel dir") {
        let path = entry.expect("entry").path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with("engine") {
            if path.is_dir() {
                read_tree(&path, &mut sources);
            } else {
                sources.push((path.clone(), std::fs::read_to_string(&path).expect("reads")));
            }
        }
    }
    assert!(
        !sources.is_empty(),
        "the scan found the computation sources"
    );
    for (path, text) in sources {
        for needle in ["ai_drafts", "list_drafts", "DraftRecord"] {
            assert!(
                !text.contains(needle),
                "{} names {needle} — a computation path must never read drafts (FR72)",
                path.display()
            );
        }
    }
}

// ── G3 follow-ups ──

#[test]
fn the_table_columns_and_indexes_are_pinned() {
    let (_dir, path, _journal) = dossier();
    let conn = raw(&path);
    let columns: Vec<String> = conn
        .prepare("PRAGMA table_info(ai_drafts)")
        .expect("table_info")
        .query_map([], |r| r.get::<_, String>(1))
        .expect("reads")
        .collect::<Result<_, _>>()
        .expect("collects");
    assert_eq!(
        columns,
        [
            "id",
            "kind",
            "study_id",
            "security_ticker",
            "native_currency",
            "status",
            "created_at",
            "decided_at",
            "comment",
            "origin_client",
            "origin_model",
            "stale_at_decision",
            "edited_before_validation",
            "created_study_id",
            "payload",
        ],
        "the ai_drafts column set (arch A4) drifted"
    );
    let indexes: Vec<String> = conn
        .prepare(
            "SELECT name FROM sqlite_master
             WHERE type = 'index' AND tbl_name = 'ai_drafts' AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )
        .expect("prepares")
        .query_map([], |r| r.get::<_, String>(0))
        .expect("reads")
        .collect::<Result<_, _>>()
        .expect("collects");
    assert_eq!(
        indexes,
        [
            "idx_ai_drafts_created_study_id",
            "idx_ai_drafts_status",
            "idx_ai_drafts_study_id",
        ]
    );
}

fn fresh_target() -> (TempDir, Journal) {
    let dir = TempDir::new().expect("tempdir");
    let target = Journal::create(
        dir.path().join("target.db"),
        Uuid::from_u128(0x9999),
        &ts("2026-09-27T07:00:00Z"),
    )
    .expect("creates");
    (dir, target)
}

#[test]
fn a_dangling_created_study_id_refuses_the_whole_import_naming_both_ids() {
    let (_dir, path, journal) = dossier();
    insert(
        &raw(&path),
        &Row {
            status: "validated",
            decided_at: Some("2026-09-27T10:00:00Z"),
            created_study_id: Some(uuid_text(STUDY_B)),
            ..study_row(0x50)
        },
    )
    .expect("plants");
    // Keep only study A in the file: B (the created study) is in neither the file nor the target.
    let exported = edited_export(&journal.export_journal().expect("exports"), |s| {
        let a = s["studies"]
            .as_array()
            .expect("studies")
            .iter()
            .find(|r| r["study"]["id"] == serde_json::json!(uuid_text(STUDY_A)))
            .cloned()
            .expect("A is exported");
        s["studies"] = serde_json::json!([a]);
    });
    let (_d, mut target) = fresh_target();
    let version = target.logical_version().expect("reads");
    match target.import_journal(&exported) {
        Err(Error::ImportMalformed { detail }) => {
            assert!(
                detail.contains(&uuid_text(0x50)),
                "names the draft: {detail}"
            );
            assert!(
                detail.contains(&uuid_text(STUDY_B)),
                "names the study: {detail}"
            );
        }
        other => panic!("expected ImportMalformed, got {other:?}"),
    }
    assert!(
        target.list_studies().expect("reads").is_empty(),
        "nothing applied"
    );
    assert_eq!(target.logical_version().expect("reads"), version);
}

#[test]
fn an_import_bumps_once_plus_once_per_new_draft_and_a_reimport_once() {
    let (_dir, path, journal) = dossier();
    plant_every_draft(&path);
    let exported = journal.export_journal().expect("exports");
    let (_d, mut target) = fresh_target();
    let v0 = target.logical_version().expect("reads");
    target.import_journal(&exported).expect("imports");
    assert_eq!(
        target.logical_version().expect("reads"),
        v0 + 1 + 10,
        "the import's own bump + one trigger bump per inserted draft (util.rs)"
    );
    let v1 = target.logical_version().expect("reads");
    target.import_journal(&exported).expect("re-imports");
    assert_eq!(
        target.logical_version().expect("reads"),
        v1 + 1,
        "an update-in-place fires no insert trigger"
    );
}

#[test]
fn an_older_export_never_moves_a_decided_draft_back() {
    // An export taken while the draft was pending, re-imported after the owner validated it:
    // the decision and the created study stay (keep-existing, never backwards).
    let (_dir, path, mut journal) = dossier();
    let conn = raw(&path);
    insert(&conn, &study_row(0x50)).expect("plants the pending draft study");
    insert(&conn, &note_row(0x51)).expect("plants a pending note draft");
    let older = journal.export_journal().expect("exports while pending");
    conn.execute(
        "UPDATE ai_drafts SET status = 'validated', decided_at = '2026-09-27T12:00:00Z',
             created_study_id = ?1 WHERE id = ?2",
        params![uuid_text(STUDY_B), uuid_text(0x50)],
    )
    .expect("the owner validates the draft study");
    conn.execute(
        "UPDATE ai_drafts SET status = 'rejected', decided_at = '2026-09-27T12:00:00Z'
             WHERE id = ?1",
        params![uuid_text(0x51)],
    )
    .expect("the owner rejects the note draft");
    let decided = journal.list_drafts().expect("reads");

    journal
        .import_journal(&older)
        .expect("the older export imports");
    assert_eq!(
        journal.list_drafts().expect("reads"),
        decided,
        "no decided draft went back to pending, no created_study_id was erased"
    );
}

#[test]
fn a_newer_payload_version_is_named_newer_on_read_and_on_import() {
    let newer = r#"{"version":2,"note_text":"x","future":true}"#;
    let (_dir, path, journal) = dossier();
    insert(
        &raw(&path),
        &Row {
            payload: newer.to_string(),
            ..note_row(0x20)
        },
    )
    .expect("the table accepts the text");
    match journal.list_drafts() {
        Err(Error::NewerRowSchema {
            row_schema_version: 2,
            supported: 1,
        }) => {}
        other => panic!("expected NewerRowSchema, got {other:?}"),
    }

    let (_dir2, path2, journal2) = dossier();
    insert(&raw(&path2), &note_row(0x20)).expect("plants");
    let file = edited_export(&journal2.export_journal().expect("exports"), |s| {
        s["ai_drafts"][0]["payload"] = serde_json::json!(newer);
    });
    let (_d, mut target) = fresh_target();
    match target.import_journal(&file) {
        Err(Error::ImportVersion {
            found: 2,
            supported: 1,
        }) => {}
        other => panic!("expected ImportVersion, got {other:?}"),
    }
    // Version 0 is corrupt, not newer.
    let (_dir3, path3, journal3) = dossier();
    insert(
        &raw(&path3),
        &Row {
            payload: r#"{"version":0,"note_text":"x"}"#.to_string(),
            ..note_row(0x20)
        },
    )
    .expect("plants");
    assert!(matches!(
        journal3.list_drafts(),
        Err(Error::CorruptPayload { .. })
    ));
}

#[test]
fn an_imported_draft_with_invalid_fields_is_malformed_naming_it() {
    let (_dir, path, journal) = dossier();
    insert(&raw(&path), &study_row(0x50)).expect("plants");
    let exported = journal.export_journal().expect("exports");
    let edits: Vec<(&str, Box<SnapshotEdit>)> = vec![
        (
            "comment",
            Box::new(|s| s["ai_drafts"][0]["comment"] = serde_json::json!("\u{00A0}\u{2003}")),
        ),
        (
            "origin_model",
            Box::new(|s| s["ai_drafts"][0]["origin_model"] = serde_json::json!("\u{3000}")),
        ),
        (
            "security_ticker",
            Box::new(|s| s["ai_drafts"][0]["security_ticker"] = serde_json::json!(" ")),
        ),
        (
            "native_currency",
            Box::new(|s| s["ai_drafts"][0]["native_currency"] = serde_json::json!("euro")),
        ),
        (
            "created_at",
            Box::new(|s| s["ai_drafts"][0]["created_at"] = serde_json::json!("hier")),
        ),
    ];
    for (column, edit) in edits {
        let file = edited_export(&exported, |s| edit(s));
        let (_d, mut target) = fresh_target();
        match target.import_journal(&file) {
            Err(Error::ImportMalformed { detail }) => {
                assert!(detail.contains(column), "{column}: {detail}");
                assert!(
                    detail.contains(&uuid_text(0x50)),
                    "names the draft: {detail}"
                );
            }
            other => panic!("{column}: expected ImportMalformed, got {other:?}"),
        }
        assert!(
            target.list_studies().expect("reads").is_empty(),
            "all-or-nothing"
        );
    }
}

/// One damage done to a planted row.
type RowDamage = dyn Fn(Row) -> Row;

#[test]
fn a_planted_corrupt_column_fails_the_read_naming_it() {
    // Rows the CHECKs would refuse, planted with the checks off (a damaged file): the read names
    // the column, never skips the row.
    let cases: Vec<(&str, Box<RowDamage>)> = vec![
        (
            "ai_drafts.id",
            Box::new(|r| Row {
                id: "not-a-uuid".to_string(),
                ..r
            }),
        ),
        (
            "ai_drafts.kind",
            Box::new(|r| Row {
                kind: "proposal",
                ..r
            }),
        ),
        (
            "ai_drafts.status",
            Box::new(|r| Row {
                status: "stale",
                ..r
            }),
        ),
        (
            "ai_drafts.stale_at_decision",
            Box::new(|r| Row {
                stale: Some(7),
                ..r
            }),
        ),
    ];
    for (column, damage) in cases {
        let (_dir, path, journal) = dossier();
        let conn = raw(&path);
        conn.pragma_update(None, "ignore_check_constraints", true)
            .expect("checks off");
        insert(&conn, &damage(note_row(0x20))).expect("planted despite the checks");
        match journal.list_drafts() {
            Err(Error::CorruptPayload { detail }) => {
                assert!(detail.contains(column), "{column}: {detail}")
            }
            other => panic!("{column}: expected CorruptPayload, got {other:?}"),
        }
    }
}
