//! Story 8.2b — the decision side of `ai_drafts`: `decide_draft` and `step_draft_decision` write the
//! study (upsert + FR51 snapshot) and the draft in ONE transaction with ONE logical-version bump,
//! re-check the draft and the study inside it, obey the 8.2a CHECKs, and leave everything unchanged
//! when a failure is injected between the two writes (NFR-R2).
//!
//! Drafts are planted by raw SQL (the MCP inserter is Story 8.3's), exactly as the 8.2a tests do.

use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
use steadyinvest_contract::{
    DraftStatus, ForecastLowOption, Judgment, Money, SCHEMA_VERSION, Study, Timestamp,
};
use steadyinvest_persistence::{
    DraftDecisionWrite, DraftStep, DraftVerdict, Error, Journal, StudyWrite,
};
use tempfile::TempDir;
use uuid::Uuid;

fn ts(s: &str) -> Timestamp {
    Timestamp(s.to_string())
}

fn money(s: &str) -> Money {
    serde_json::from_str(&format!("\"{s}\"")).expect("decimal parses")
}

const JID: u128 = 0x8B00;
const STUDY_A: u128 = 0xA1;
const DRAFT: u128 = 0xD1;

fn study(low_pe: &str) -> Study {
    Study {
        id: Uuid::from_u128(STUDY_A),
        journal_id: Uuid::from_u128(JID),
        security_ticker: "NESN".to_string(),
        native_currency: "CHF".to_string(),
        years: Vec::new(),
        judgment: Judgment {
            estimated_high_eps: Some(money("5.20")),
            estimated_low_eps: None,
            projected_sales_growth_pct: None,
            projected_eps_growth_pct: None,
            judged_avg_high_pe: None,
            judged_avg_low_pe: Some(money(low_pe)),
            forecast_low_option: ForecastLowOption::AvgLowPriceLast5y,
            recent_severe_low: None,
            current_price: None,
            present_full_year_dividend: None,
            ttm_eps: None,
            ai_placed: Default::default(),
        },
        rationale: None,
        company_name: None,
        notes: Vec::new(),
        frozen_verdict: None,
        created_at: ts("2026-09-27T08:00:00Z"),
        schema_version: SCHEMA_VERSION,
    }
}

fn dossier() -> (TempDir, PathBuf, Journal) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("dossier.db");
    let mut journal =
        Journal::create(&path, Uuid::from_u128(JID), &ts("2026-09-27T07:00:00Z")).expect("creates");
    journal
        .put_study_with_history(&study("12"), &ts("2026-09-27T08:00:00Z"))
        .expect("A");
    (dir, path, journal)
}

fn raw(path: &Path) -> Connection {
    let conn = Connection::open(path).expect("raw open");
    conn.pragma_update(None, "foreign_keys", true)
        .expect("foreign keys on");
    conn
}

const JUDGMENT_PAYLOAD: &str = r#"{"version":1,"target":{"target":"judgment","field":"judged_avg_low_pe"},"proposed_value":"11.5"}"#;

/// Plant one pending judgment draft on study A (or `status` / `decided_at` as given).
fn plant(path: &Path, status: &str, decided_at: Option<&str>) {
    raw(path)
        .execute(
            "INSERT INTO ai_drafts
                 (id, kind, study_id, security_ticker, native_currency, status, created_at,
                  decided_at, comment, origin_client, origin_model, stale_at_decision,
                  edited_before_validation, created_study_id, payload)
             VALUES (?1, 'judgment', ?2, 'NESN', NULL, ?3, '2026-09-27T09:00:00Z', ?4,
                     'Le P/E bas historique est plus proche de 11.', 'claude-code', 'm',
                     NULL, NULL, NULL, ?5)",
            params![
                Uuid::from_u128(DRAFT).to_string(),
                Uuid::from_u128(STUDY_A).to_string(),
                status,
                decided_at,
                JUDGMENT_PAYLOAD
            ],
        )
        .expect("plants");
}

type DraftFacts = (String, Option<String>, Option<i64>, Option<i64>);

fn draft_facts(path: &Path) -> DraftFacts {
    raw(path)
        .query_row(
            "SELECT status, decided_at, stale_at_decision, edited_before_validation
               FROM ai_drafts WHERE id = ?1",
            params![Uuid::from_u128(DRAFT).to_string()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("draft reads")
}

fn snapshots(path: &Path) -> i64 {
    raw(path)
        .query_row("SELECT count(*) FROM judgments", [], |r| r.get(0))
        .expect("count")
}

fn stored_payload(path: &Path) -> String {
    raw(path)
        .query_row(
            "SELECT payload FROM studies WHERE id = ?1",
            params![Uuid::from_u128(STUDY_A).to_string()],
            |r| r.get(0),
        )
        .expect("payload")
}

fn validation<'a>(
    before: &'a Study,
    after: &'a Study,
    now: &'a Timestamp,
    stale: bool,
) -> DraftDecisionWrite<'a> {
    DraftDecisionWrite {
        draft_id: Uuid::from_u128(DRAFT),
        verdict: DraftVerdict::Validated,
        decided_at: now,
        stale_at_decision: Some(stale),
        edited_before_validation: Some(false),
        study: Some(StudyWrite {
            expected_before: before,
            after,
            now,
        }),
    }
}

#[test]
fn a_validation_writes_the_study_its_snapshot_and_the_draft_with_one_bump() {
    let (_dir, path, mut journal) = dossier();
    plant(&path, "pending", None);
    let before = journal
        .get_study(Uuid::from_u128(STUDY_A))
        .unwrap()
        .unwrap();
    let after = study("11.5");
    let version = journal.logical_version().unwrap();
    let snaps = snapshots(&path);
    let now = ts("2026-09-27T10:00:00Z");
    journal
        .decide_draft(validation(&before, &after, &now, true))
        .expect("decides");
    assert_eq!(journal.logical_version().unwrap(), version + 1, "one bump");
    assert_eq!(snapshots(&path), snaps + 1, "one FR51 snapshot");
    assert_eq!(journal.get_study(after.id).unwrap().unwrap(), after);
    assert_eq!(
        draft_facts(&path),
        (
            "validated".to_string(),
            Some("2026-09-27T10:00:00Z".to_string()),
            Some(1),
            Some(0)
        )
    );
    let record = journal.get_draft(Uuid::from_u128(DRAFT)).unwrap().unwrap();
    assert_eq!(record.status, DraftStatus::Validated);
}

#[test]
fn a_rejection_writes_only_the_draft() {
    let (_dir, path, mut journal) = dossier();
    plant(&path, "pending", None);
    let payload = stored_payload(&path);
    let version = journal.logical_version().unwrap();
    let snaps = snapshots(&path);
    let now = ts("2026-09-27T10:00:00Z");
    journal
        .decide_draft(DraftDecisionWrite {
            draft_id: Uuid::from_u128(DRAFT),
            verdict: DraftVerdict::Rejected,
            decided_at: &now,
            stale_at_decision: Some(false),
            edited_before_validation: None,
            study: None,
        })
        .expect("rejects");
    assert_eq!(stored_payload(&path), payload);
    assert_eq!(snapshots(&path), snaps);
    assert_eq!(journal.logical_version().unwrap(), version + 1);
    assert_eq!(draft_facts(&path).0, "rejected");
}

#[test]
fn a_draft_decided_meanwhile_is_refused_and_nothing_is_written() {
    let (_dir, path, mut journal) = dossier();
    plant(&path, "rejected", Some("2026-09-27T09:30:00Z"));
    let before = journal
        .get_study(Uuid::from_u128(STUDY_A))
        .unwrap()
        .unwrap();
    let after = study("11.5");
    let payload = stored_payload(&path);
    let version = journal.logical_version().unwrap();
    let now = ts("2026-09-27T10:00:00Z");
    let err = journal
        .decide_draft(validation(&before, &after, &now, false))
        .expect_err("refused");
    assert!(
        matches!(&err, Error::DraftNotPending { status } if status == "rejected"),
        "{err:?}"
    );
    assert_eq!(stored_payload(&path), payload);
    assert_eq!(journal.logical_version().unwrap(), version);
}

#[test]
fn an_unknown_draft_is_named_missing() {
    let (_dir, _path, mut journal) = dossier();
    let now = ts("2026-09-27T10:00:00Z");
    let err = journal
        .decide_draft(DraftDecisionWrite {
            draft_id: Uuid::from_u128(0xDEAD),
            verdict: DraftVerdict::Rejected,
            decided_at: &now,
            stale_at_decision: None,
            edited_before_validation: None,
            study: None,
        })
        .expect_err("refused");
    assert!(matches!(err, Error::DraftNotFound { .. }), "{err:?}");
    assert_eq!(
        err.kind(),
        steadyinvest_persistence::ErrorKind::Other,
        "a missing draft row is never « fichier introuvable »"
    );
}

#[test]
fn a_study_changed_since_the_decision_was_prepared_is_refused() {
    let (_dir, path, mut journal) = dossier();
    plant(&path, "pending", None);
    let before = journal
        .get_study(Uuid::from_u128(STUDY_A))
        .unwrap()
        .unwrap();
    // Another write lands between the read and the decision.
    journal
        .put_study_with_history(&study("13"), &ts("2026-09-27T09:45:00Z"))
        .unwrap();
    let payload = stored_payload(&path);
    let after = study("11.5");
    let now = ts("2026-09-27T10:00:00Z");
    let err = journal
        .decide_draft(validation(&before, &after, &now, false))
        .expect_err("refused");
    assert!(matches!(err, Error::StudyChangedSinceRead), "{err:?}");
    assert_eq!(stored_payload(&path), payload);
    assert_eq!(draft_facts(&path).0, "pending");
}

#[test]
fn a_scale_only_change_since_the_read_is_a_change() {
    // `Study` equality would call "12" and "12.0" equal; the stored string differs — the guard
    // compares re-serialized strings (the 8.1 dedup technique), so the write is refused.
    let (_dir, path, mut journal) = dossier();
    plant(&path, "pending", None);
    let before = journal
        .get_study(Uuid::from_u128(STUDY_A))
        .unwrap()
        .unwrap();
    journal.put_study(&study("12.0")).unwrap();
    let after = study("11.5");
    let now = ts("2026-09-27T10:00:00Z");
    let err = journal
        .decide_draft(validation(&before, &after, &now, false))
        .expect_err("refused");
    assert!(matches!(err, Error::StudyChangedSinceRead), "{err:?}");
}

#[test]
fn undo_and_redo_move_the_draft_and_the_study_together() {
    let (_dir, path, mut journal) = dossier();
    plant(&path, "pending", None);
    let before = journal
        .get_study(Uuid::from_u128(STUDY_A))
        .unwrap()
        .unwrap();
    let after = study("11.5");
    let now = ts("2026-09-27T10:00:00Z");
    journal
        .decide_draft(validation(&before, &after, &now, false))
        .unwrap();
    let draft = Uuid::from_u128(DRAFT);

    let version = journal.logical_version().unwrap();
    let t1 = ts("2026-09-27T10:05:00Z");
    journal
        .step_draft_decision(&before, draft, DraftStep::Undo, &t1)
        .expect("undo");
    assert_eq!(journal.get_study(before.id).unwrap().unwrap(), before);
    assert_eq!(
        draft_facts(&path),
        (
            "validated_undone".to_string(),
            Some("2026-09-27T10:05:00Z".to_string()),
            Some(0),
            Some(0)
        ),
        "decision facts kept, decided_at = the transition"
    );
    assert_eq!(journal.logical_version().unwrap(), version + 1);

    let t2 = ts("2026-09-27T10:06:00Z");
    journal
        .step_draft_decision(&after, draft, DraftStep::Redo, &t2)
        .expect("redo");
    assert_eq!(journal.get_study(after.id).unwrap().unwrap(), after);
    assert_eq!(draft_facts(&path).0, "validated");

    // A second redo finds the draft already validated.
    let err = journal
        .step_draft_decision(&after, draft, DraftStep::Redo, &t2)
        .expect_err("mismatch");
    assert!(
        matches!(&err, Error::DraftStatusMismatch { expected, found }
            if expected == "validated_undone" && found == "validated"),
        "{err:?}"
    );
}

#[test]
fn a_decision_that_breaks_a_check_is_refused_by_the_engine() {
    // `edited_before_validation` only exists on a validation (8.2a CHECK).
    let (_dir, path, mut journal) = dossier();
    plant(&path, "pending", None);
    let now = ts("2026-09-27T10:00:00Z");
    let err = journal
        .decide_draft(DraftDecisionWrite {
            draft_id: Uuid::from_u128(DRAFT),
            verdict: DraftVerdict::Rejected,
            decided_at: &now,
            stale_at_decision: None,
            edited_before_validation: Some(true),
            study: None,
        })
        .expect_err("CHECK");
    assert!(matches!(err, Error::Sqlite(_)), "{err:?}");
    assert_eq!(draft_facts(&path).0, "pending");
}

/// NFR-R2: a failure injected at either write rolls everything back — study payload, snapshots,
/// draft row and logical version all unchanged.
#[test]
fn an_injected_failure_leaves_everything_unchanged() {
    for trigger in [
        "CREATE TRIGGER inject BEFORE UPDATE ON ai_drafts BEGIN SELECT RAISE(ABORT, 'injected'); END",
        "CREATE TRIGGER inject BEFORE INSERT ON judgments BEGIN SELECT RAISE(ABORT, 'injected'); END",
    ] {
        let (_dir, path, mut journal) = dossier();
        plant(&path, "pending", None);
        let before = journal
            .get_study(Uuid::from_u128(STUDY_A))
            .unwrap()
            .unwrap();
        let after = study("11.5");
        let payload = stored_payload(&path);
        let snaps = snapshots(&path);
        let version = journal.logical_version().unwrap();
        let facts = draft_facts(&path);
        raw(&path)
            .execute_batch(trigger)
            .expect("plants the failure");
        let now = ts("2026-09-27T10:00:00Z");
        let err = journal
            .decide_draft(validation(&before, &after, &now, false))
            .expect_err("injected");
        assert!(matches!(err, Error::Sqlite(_)), "{err:?}");
        assert_eq!(stored_payload(&path), payload, "{trigger}");
        assert_eq!(snapshots(&path), snaps, "{trigger}");
        assert_eq!(draft_facts(&path), facts, "{trigger}");
        assert_eq!(journal.logical_version().unwrap(), version, "{trigger}");
        raw(&path).execute_batch("DROP TRIGGER inject").unwrap();
    }
}

#[test]
fn an_injected_failure_in_an_undo_step_leaves_everything_unchanged() {
    let (_dir, path, mut journal) = dossier();
    plant(&path, "pending", None);
    let before = journal
        .get_study(Uuid::from_u128(STUDY_A))
        .unwrap()
        .unwrap();
    let after = study("11.5");
    let now = ts("2026-09-27T10:00:00Z");
    journal
        .decide_draft(validation(&before, &after, &now, false))
        .unwrap();
    let payload = stored_payload(&path);
    let version = journal.logical_version().unwrap();
    raw(&path)
        .execute_batch(
            "CREATE TRIGGER inject BEFORE UPDATE ON ai_drafts BEGIN SELECT RAISE(ABORT, 'x'); END",
        )
        .unwrap();
    let t1 = ts("2026-09-27T10:05:00Z");
    journal
        .step_draft_decision(&before, Uuid::from_u128(DRAFT), DraftStep::Undo, &t1)
        .expect_err("injected");
    assert_eq!(stored_payload(&path), payload);
    assert_eq!(draft_facts(&path).0, "validated");
    assert_eq!(journal.logical_version().unwrap(), version);
}

#[test]
fn study_status_names_an_archived_study_and_a_gone_one() {
    let (_dir, _path, mut journal) = dossier();
    let id = Uuid::from_u128(STUDY_A);
    assert_eq!(journal.study_status(id).unwrap().as_deref(), Some("active"));
    journal.set_study_status(id, "archived").unwrap();
    assert_eq!(
        journal.study_status(id).unwrap().as_deref(),
        Some("archived")
    );
    assert_eq!(journal.study_status(Uuid::from_u128(0xBAD)).unwrap(), None);
    assert!(journal.get_draft(Uuid::from_u128(0xBAD)).unwrap().is_none());
}

#[test]
fn a_decision_writing_another_study_than_the_draft_s_is_refused() {
    // G3 F9: the study written must be the draft's own.
    let (_dir, path, mut journal) = dossier();
    plant(&path, "pending", None);
    let mut other = study("12");
    other.id = Uuid::from_u128(0xB2);
    journal.put_study(&other).unwrap();
    let before = journal.get_study(other.id).unwrap().unwrap();
    let mut after = before.clone();
    after.judgment.judged_avg_low_pe = Some(money("11.5"));
    let now = ts("2026-09-27T10:00:00Z");
    let err = journal
        .decide_draft(validation(&before, &after, &now, false))
        .expect_err("refused");
    assert!(matches!(err, Error::DraftStudyMismatch { .. }), "{err:?}");
    assert_eq!(draft_facts(&path).0, "pending");
    let err = journal
        .step_draft_decision(&after, Uuid::from_u128(DRAFT), DraftStep::Undo, &now)
        .expect_err("refused");
    assert!(matches!(err, Error::DraftStudyMismatch { .. }), "{err:?}");
}
