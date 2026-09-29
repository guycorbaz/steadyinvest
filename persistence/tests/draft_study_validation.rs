//! Story 8.7 — validating a draft study: `validate_draft_study` creates the study (with its FR51
//! creation snapshot) and records the draft `validated` + `created_study_id` in ONE transaction with
//! ONE logical-version bump, re-runs the duplicate check (D2, archived studies included, identifier
//! compared case-insensitively), refuses a draft that is no longer pending, and writes nothing on a
//! refusal. The created study's deletion takes the draft with it (O7). `list_study_drafts` reads a
//! study's processed drafts, its draft study included, for the history merge (A12).

use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
use steadyinvest_contract::{ForecastLowOption, Judgment, SCHEMA_VERSION, Study, Timestamp};
use steadyinvest_persistence::{DraftStudyValidation, Error, Journal};
use tempfile::TempDir;
use uuid::Uuid;

const JID: u128 = 0x8700;
const DRAFT: u128 = 0xD7;

fn ts(s: &str) -> Timestamp {
    Timestamp(s.to_string())
}

fn empty_judgment() -> Judgment {
    Judgment {
        estimated_high_eps: None,
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
        ai_placed: Default::default(),
    }
}

fn new_study(id: u128, ticker: &str, currency: &str, at: &str) -> Study {
    Study {
        id: Uuid::from_u128(id),
        journal_id: Uuid::from_u128(JID),
        security_ticker: ticker.to_string(),
        native_currency: currency.to_string(),
        years: Vec::new(),
        judgment: empty_judgment(),
        rationale: None,
        company_name: None,
        notes: Vec::new(),
        created_at: ts(at),
        schema_version: SCHEMA_VERSION,
    }
}

fn dossier() -> (TempDir, PathBuf, Journal) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("dossier.db");
    let journal =
        Journal::create(&path, Uuid::from_u128(JID), &ts("2026-09-29T07:00:00Z")).expect("creates");
    (dir, path, journal)
}

fn raw(path: &Path) -> Connection {
    let conn = Connection::open(path).expect("raw open");
    conn.pragma_update(None, "foreign_keys", true)
        .expect("foreign keys on");
    conn
}

/// Plant one draft study (ROG / CHF unless given) in `status`.
fn plant_draft_study(path: &Path, ticker: &str, currency: &str, status: &str) {
    let decided = (status != "pending").then_some("2026-09-29T08:30:00Z");
    raw(path)
        .execute(
            "INSERT INTO ai_drafts
                 (id, kind, study_id, security_ticker, native_currency, status, created_at,
                  decided_at, comment, origin_client, origin_model, stale_at_decision,
                  edited_before_validation, created_study_id, payload)
             VALUES (?1, 'study', NULL, ?2, ?3, ?4, '2026-09-29T08:00:00Z', ?5,
                     'Croissance régulière, bilan solide.', 'claude-code', 'm',
                     NULL, NULL, NULL, '{\"version\":1,\"company_name\":\"Roche Holding\"}')",
            params![
                Uuid::from_u128(DRAFT).to_string(),
                ticker,
                currency,
                status,
                decided
            ],
        )
        .expect("plants");
}

type Facts = (String, Option<String>, Option<String>, Option<i64>);

fn facts(path: &Path) -> Facts {
    raw(path)
        .query_row(
            "SELECT status, decided_at, created_study_id, edited_before_validation
               FROM ai_drafts WHERE id = ?1",
            params![Uuid::from_u128(DRAFT).to_string()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("draft reads")
}

fn count(path: &Path, sql: &str) -> i64 {
    raw(path).query_row(sql, [], |r| r.get(0)).expect("count")
}

fn validation<'a>(study: &'a Study, now: &'a Timestamp, edited: bool) -> DraftStudyValidation<'a> {
    DraftStudyValidation {
        draft_id: Uuid::from_u128(DRAFT),
        study,
        edited,
        now,
    }
}

#[test]
fn validating_creates_the_study_and_records_the_draft_in_one_bump() {
    let (_dir, path, mut journal) = dossier();
    plant_draft_study(&path, "ROG", "CHF", "pending");
    let before = journal.logical_version().unwrap();
    let now = ts("2026-09-29T09:00:00Z");
    let study = new_study(0x51, "ROG", "CHF", "2026-09-29T09:00:00Z");
    journal
        .validate_draft_study(validation(&study, &now, false))
        .expect("validates");
    assert_eq!(journal.logical_version().unwrap(), before + 1, "one bump");
    assert_eq!(journal.get_study(study.id).unwrap().as_ref(), Some(&study));
    assert_eq!(
        count(&path, "SELECT count(*) FROM judgments"),
        1,
        "the FR51 creation snapshot"
    );
    assert_eq!(
        facts(&path),
        (
            "validated".to_string(),
            Some(now.0.clone()),
            Some(study.id.to_string()),
            Some(0)
        )
    );
}

#[test]
fn an_edited_validation_is_marked() {
    let (_dir, path, mut journal) = dossier();
    plant_draft_study(&path, "ROG", "CHF", "pending");
    let now = ts("2026-09-29T09:00:00Z");
    let study = new_study(0x51, "ROG.SW", "CHF", "2026-09-29T09:00:00Z");
    journal
        .validate_draft_study(validation(&study, &now, true))
        .expect("validates");
    assert_eq!(facts(&path).3, Some(1));
}

#[test]
fn a_duplicate_is_refused_whatever_its_case_or_status_and_nothing_is_written() {
    for (existing_ticker, archived) in [("rog", false), (" ROG ", true)] {
        let (_dir, path, mut journal) = dossier();
        let existing = new_study(0x50, existing_ticker, "chf", "2026-09-29T07:30:00Z");
        journal
            .put_study_with_history(&existing, &ts("2026-09-29T07:30:00Z"))
            .unwrap();
        if archived {
            journal.set_study_status(existing.id, "archived").unwrap();
        }
        plant_draft_study(&path, "ROG", "CHF", "pending");
        let before = journal.logical_version().unwrap();
        let now = ts("2026-09-29T09:00:00Z");
        let study = new_study(0x51, "ROG", "CHF", "2026-09-29T09:00:00Z");
        match journal.validate_draft_study(validation(&study, &now, false)) {
            Err(Error::DraftStudyExists {
                ticker,
                currency,
                study_id,
            }) => {
                assert_eq!(
                    (ticker.as_str(), currency.as_str()),
                    (existing_ticker, "chf"),
                    "named in the EXISTING study's spelling"
                );
                assert_eq!(study_id, existing.id);
            }
            other => panic!("expected DraftStudyExists, got {other:?}"),
        }
        assert_eq!(
            journal.logical_version().unwrap(),
            before,
            "nothing written"
        );
        assert!(journal.get_study(study.id).unwrap().is_none());
        assert_eq!(facts(&path).0, "pending", "the draft stays pending");
    }
}

#[test]
fn another_currency_is_not_a_duplicate() {
    let (_dir, path, mut journal) = dossier();
    let existing = new_study(0x50, "ROG", "USD", "2026-09-29T07:30:00Z");
    journal
        .put_study_with_history(&existing, &ts("2026-09-29T07:30:00Z"))
        .unwrap();
    plant_draft_study(&path, "ROG", "CHF", "pending");
    let now = ts("2026-09-29T09:00:00Z");
    let study = new_study(0x51, "ROG", "CHF", "2026-09-29T09:00:00Z");
    journal
        .validate_draft_study(validation(&study, &now, false))
        .expect("another currency is another study");
}

#[test]
fn a_draft_decided_meanwhile_is_refused_and_nothing_is_created() {
    let (_dir, path, mut journal) = dossier();
    plant_draft_study(&path, "ROG", "CHF", "rejected");
    let now = ts("2026-09-29T09:00:00Z");
    let study = new_study(0x51, "ROG", "CHF", "2026-09-29T09:00:00Z");
    assert!(matches!(
        journal.validate_draft_study(validation(&study, &now, false)),
        Err(Error::DraftNotPending { .. })
    ));
    assert!(journal.get_study(study.id).unwrap().is_none());
    assert!(matches!(
        journal.validate_draft_study(DraftStudyValidation {
            draft_id: Uuid::from_u128(0xFFFF),
            ..validation(&study, &now, false)
        }),
        Err(Error::DraftNotFound { .. })
    ));
}

#[test]
fn deleting_the_created_study_deletes_the_draft_study_record() {
    let (_dir, path, mut journal) = dossier();
    plant_draft_study(&path, "ROG", "CHF", "pending");
    let now = ts("2026-09-29T09:00:00Z");
    let study = new_study(0x51, "ROG", "CHF", "2026-09-29T09:00:00Z");
    journal
        .validate_draft_study(validation(&study, &now, false))
        .unwrap();
    let processed = journal.list_study_drafts(study.id).unwrap();
    assert_eq!(processed.len(), 1, "the draft study it was created from");
    assert_eq!(processed[0].created_study_id, Some(study.id));
    journal.delete_study(study.id).unwrap();
    assert_eq!(count(&path, "SELECT count(*) FROM ai_drafts"), 0, "O7");
}

#[test]
fn a_study_s_processed_drafts_exclude_pending_ones_and_other_studies() {
    let (_dir, path, mut journal) = dossier();
    let a = new_study(0x50, "NESN", "CHF", "2026-09-29T07:30:00Z");
    journal
        .put_study_with_history(&a, &ts("2026-09-29T07:30:00Z"))
        .unwrap();
    let conn = raw(&path);
    for (n, status, decided, study) in [
        (1u128, "rejected", Some("2026-09-29T10:00:00Z"), a.id),
        (2, "pending", None, a.id),
        (3, "validated_undone", Some("2026-09-29T09:00:00Z"), a.id),
    ] {
        conn.execute(
            "INSERT INTO ai_drafts
                 (id, kind, study_id, security_ticker, native_currency, status, created_at,
                  decided_at, comment, origin_client, origin_model, stale_at_decision,
                  edited_before_validation, created_study_id, payload)
             VALUES (?1, 'note', ?2, 'NESN', NULL, ?3, '2026-09-29T08:00:00Z', ?4,
                     'Une note.', 'c', 'm', NULL, ?5, NULL,
                     '{\"version\":1,\"note_text\":\"Texte\"}')",
            params![
                Uuid::from_u128(n).to_string(),
                study.to_string(),
                status,
                decided,
                (status == "validated_undone").then_some(0i64)
            ],
        )
        .expect("plants");
    }
    let got: Vec<Uuid> = journal
        .list_study_drafts(a.id)
        .unwrap()
        .iter()
        .map(|d| d.id)
        .collect();
    assert_eq!(
        got,
        vec![Uuid::from_u128(3), Uuid::from_u128(1)],
        "processed only, by decided_at"
    );
}

#[test]
fn a_study_id_already_in_the_dossier_is_never_overwritten() {
    let (_dir, path, mut journal) = dossier();
    let existing = new_study(0x51, "NESN", "CHF", "2026-09-29T07:30:00Z");
    journal
        .put_study_with_history(&existing, &ts("2026-09-29T07:30:00Z"))
        .unwrap();
    plant_draft_study(&path, "ROG", "CHF", "pending");
    let now = ts("2026-09-29T09:00:00Z");
    let colliding = new_study(0x51, "ROG", "CHF", "2026-09-29T09:00:00Z");
    assert!(matches!(
        journal.validate_draft_study(validation(&colliding, &now, false)),
        Err(Error::DraftStudyMismatch { .. })
    ));
    assert_eq!(journal.get_study(existing.id).unwrap(), Some(existing));
    assert_eq!(facts(&path).0, "pending");
}

// Story 8.7 AC 17 (Decision 4): MCP `get_drafts_record` filtered on a study returns its own drafts
// AND the draft study it was created from.
#[test]
fn the_mcp_record_of_a_study_includes_the_draft_study_it_came_from() {
    use steadyinvest_persistence::{DraftFilter, McpAccess, Page};
    let (_dir, path, mut journal) = dossier();
    plant_draft_study(&path, "ROG", "CHF", "pending");
    let now = ts("2026-09-29T09:00:00Z");
    let study = new_study(0x51, "ROG", "CHF", "2026-09-29T09:00:00Z");
    journal
        .validate_draft_study(validation(&study, &now, false))
        .unwrap();
    raw(&path)
        .execute(
            "INSERT INTO ai_drafts
                 (id, kind, study_id, security_ticker, native_currency, status, created_at,
                  decided_at, comment, origin_client, origin_model, stale_at_decision,
                  edited_before_validation, created_study_id, payload)
             VALUES (?1, 'note', ?2, 'ROG', NULL, 'pending', '2026-09-29T10:00:00Z', NULL,
                     'Une note.', 'c', 'm', NULL, NULL, NULL,
                     '{\"version\":1,\"note_text\":\"Texte\"}')",
            params![Uuid::from_u128(0xE1).to_string(), study.id.to_string()],
        )
        .unwrap();
    drop(journal);
    let record = McpAccess::at(&path)
        .list_drafts(
            DraftFilter {
                study_id: Some(study.id),
                status: None,
            },
            Page::first(50),
        )
        .expect("record");
    let mut ids: Vec<Uuid> = record.items.iter().map(|d| d.id).collect();
    ids.sort();
    assert_eq!(ids, vec![Uuid::from_u128(DRAFT), Uuid::from_u128(0xE1)]);
}
