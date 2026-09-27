//! Story 8.3 — the gated MCP access surface (`McpAccess`), through its public API only: per-call
//! and lock-free opens, the version gate, WAL and DELETE dossiers, the sidecars a read leaves, the
//! whole-surface non-exposure markers, every submission refusal with its code, the draft-origin
//! rule, and the race with the app's `delete_study`.
//!
//! The authorizer policies, the rejected-writes suite, the table classification and the restore
//! race live beside the code (`src/mcp_access.rs`), where the gated connections are reachable.

use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};
use steadyinvest_contract::{
    Cell, Coverage, DraftKind, DraftOrigin, DraftStatus, DraftTarget, ForecastLowOption, Freshness,
    Judgment, Money, Provenance, Review, Source, Study, Timestamp, YearData,
};
use steadyinvest_persistence::{
    DossierIdentity, DraftFilter, DraftSubmission, Error, Journal, JournalMode, MAX_PAGE,
    McpAccess, Page, SubmissionRefusal, SubmitError,
};
use tempfile::TempDir;
use uuid::Uuid;

const JID: u128 = 0x8300;
const STUDY_A: u128 = 0x83a;
const STUDY_B: u128 = 0x83b;
const METHOD: &str = "ssg-1.2.0";

fn ts(s: &str) -> Timestamp {
    Timestamp(s.to_string())
}

fn money(s: &str) -> Money {
    serde_json::from_str(&format!("\"{s}\"")).expect("decimal parses")
}

fn cell(v: &str) -> Cell {
    Cell {
        value: Some(money(v)),
        source: Source::Provider,
        freshness: Freshness::Current,
        review: Review::None,
        coverage: Coverage::Present,
        provenance: Provenance {
            ai_origin: None,
            source: Source::Provider,
            logical_version: 1,
            timestamp: ts("2026-09-28T08:00:00Z"),
            hash_of_dependencies: "t".to_string(),
        },
        pending: None,
    }
}

fn study(id: u128, ticker: &str, currency: &str, created: &str) -> Study {
    let mut s = Study::new(
        Uuid::from_u128(id),
        Uuid::from_u128(JID),
        ticker,
        currency,
        Judgment {
            ai_placed: Default::default(),
            estimated_high_eps: Some(money("5.20")),
            estimated_low_eps: None,
            projected_sales_growth_pct: None,
            projected_eps_growth_pct: None,
            judged_avg_high_pe: None,
            judged_avg_low_pe: None,
            forecast_low_option: ForecastLowOption::AvgLowPriceLast5y,
            recent_severe_low: None,
            current_price: Some(money("100")),
            present_full_year_dividend: None,
            ttm_eps: None,
        },
        ts(created),
    );
    s.years = vec![YearData {
        year: 2025,
        sales: cell("1000"),
        eps: cell("4.1"),
        high_price: cell("110"),
        low_price: cell("80"),
        dividend_per_share: None,
        pre_tax_profit: None,
        book_value_per_share: None,
    }];
    s
}

/// A closed dossier holding studies A (NESN.SW, CHF) and B (AAPL, USD).
fn dossier(dir: &TempDir, mode: JournalMode) -> PathBuf {
    let path = dir.path().join("dossier.db");
    let mut j = Journal::create_with_mode(
        &path,
        Uuid::from_u128(JID),
        &ts("2026-09-28T07:00:00Z"),
        mode,
    )
    .expect("create");
    for s in [
        study(STUDY_A, "NESN.SW", "CHF", "2026-09-28T08:00:00Z"),
        study(STUDY_B, "AAPL", "USD", "2026-09-28T08:05:00Z"),
    ] {
        j.put_study_with_history(&s, &ts("2026-09-28T08:10:00Z"))
            .expect("study");
    }
    drop(j);
    path
}

fn identity(path: &Path) -> DossierIdentity {
    DossierIdentity {
        journal_id: Uuid::from_u128(JID),
        path: path.to_path_buf(),
    }
}

fn base(path: &Path, n: u128, kind: DraftKind) -> DraftSubmission {
    DraftSubmission {
        id: Uuid::from_u128(0x83_d000 + n),
        created_at: ts("2026-09-28T09:00:00Z"),
        kind,
        study_id: Some(Uuid::from_u128(STUDY_A)),
        security_ticker: None,
        native_currency: None,
        company_name: None,
        target: None,
        proposed_value: None,
        note_text: None,
        comment: "vu dans le rapport annuel".to_string(),
        origin: DraftOrigin {
            client: "claude-code".to_string(),
            model: "test-model".to_string(),
        },
        dossier: identity(path),
        method_version: METHOD.to_string(),
    }
}

fn note(path: &Path, n: u128) -> DraftSubmission {
    DraftSubmission {
        note_text: Some("Marge en hausse".to_string()),
        ..base(path, n, DraftKind::Note)
    }
}

fn cell_draft(path: &Path, n: u128, year: i32, field: &str, value: &str) -> DraftSubmission {
    DraftSubmission {
        target: Some(DraftTarget::Cell {
            fiscal_year: year,
            field: field.to_string(),
        }),
        proposed_value: Some(value.to_string()),
        ..base(path, n, DraftKind::Cell)
    }
}

fn judgment_draft(path: &Path, n: u128, field: &str, value: &str) -> DraftSubmission {
    DraftSubmission {
        target: Some(DraftTarget::Judgment {
            field: field.to_string(),
        }),
        proposed_value: Some(value.to_string()),
        ..base(path, n, DraftKind::Judgment)
    }
}

fn study_draft(path: &Path, n: u128, ticker: &str, currency: &str) -> DraftSubmission {
    DraftSubmission {
        study_id: None,
        security_ticker: Some(ticker.to_string()),
        native_currency: Some(currency.to_string()),
        company_name: Some("Roche Holding".to_string()),
        ..base(path, n, DraftKind::Study)
    }
}

fn refused(r: Result<Uuid, SubmitError>) -> SubmissionRefusal {
    match r {
        Err(SubmitError::Refused(refusal)) => refusal,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn draft_count(path: &Path) -> u64 {
    McpAccess::at(path)
        .list_drafts(DraftFilter::default(), Page::first(MAX_PAGE))
        .expect("drafts")
        .total
}

#[test]
fn each_call_opens_and_closes_without_the_instance_lock_even_beside_the_open_app() {
    let dir = TempDir::new().expect("tempdir");
    let path = dossier(&dir, JournalMode::Wal);
    let access = McpAccess::at(&path);
    let lock = dir.path().join("dossier.db-lock");

    // App closed: a read and a submission take no instance lock.
    assert_eq!(access.list_studies(Page::first(10)).expect("list").total, 2);
    access.submit_draft(&note(&path, 1)).expect("submitted");
    assert!(!lock.exists(), "the MCP access never takes the app's lock");

    // App open on the same dossier: the MCP access still reads and submits, and the app keeps
    // writing (its lock stays its own).
    let mut app = Journal::open(&path).expect("the app opens");
    assert!(lock.exists(), "the app holds its lock");
    let read = access
        .read_study(Uuid::from_u128(STUDY_A))
        .expect("read")
        .expect("present");
    assert_eq!(read.study.security_ticker, "NESN.SW");
    assert_eq!(read.status, "active");
    access
        .submit_draft(&note(&path, 2))
        .expect("submitted beside the app");
    let mut edited = read.study.clone();
    edited.rationale = Some("écrit par l'app".to_string());
    app.put_study_with_history(&edited, &ts("2026-09-28T10:00:00Z"))
        .expect("the app writes after the MCP calls");
    assert_eq!(app.list_drafts().expect("drafts").len(), 2);
    drop(app);
    assert!(!lock.exists(), "the app released its own lock");
}

#[test]
fn a_delete_mode_dossier_is_read_and_written_the_same_way() {
    let dir = TempDir::new().expect("tempdir");
    let path = dossier(&dir, JournalMode::Delete);
    let access = McpAccess::at(&path);
    assert_eq!(access.list_studies(Page::first(10)).expect("list").total, 2);
    access.submit_draft(&note(&path, 1)).expect("submitted");
    assert_eq!(draft_count(&path), 1);
}

#[test]
fn a_dossier_of_another_schema_version_is_refused_both_ways_and_left_untouched() {
    for (label, version) in [("newer", 9_i64), ("older", 7)] {
        let dir = TempDir::new().expect("tempdir");
        let path = dossier(&dir, JournalMode::Delete);
        Connection::open(&path)
            .expect("raw")
            .pragma_update(None, "user_version", version)
            .expect("set version");
        let before = std::fs::read(&path).expect("bytes");
        let access = McpAccess::at(&path);
        match access.list_studies(Page::first(10)) {
            Err(Error::McpSchemaMismatch {
                file_user_version,
                supported,
            }) => {
                assert_eq!(file_user_version, version, "{label}");
                assert_eq!(supported, 8, "{label}");
            }
            other => panic!("{label}: expected a schema mismatch, got {other:?}"),
        }
        match access.submit_draft(&note(&path, 1)) {
            Err(SubmitError::Failed(Error::McpSchemaMismatch { .. })) => {}
            other => panic!("{label}: expected a schema mismatch, got {other:?}"),
        }
        assert_eq!(
            std::fs::read(&path).expect("bytes"),
            before,
            "{label}: never migrated"
        );
    }
}

#[test]
fn the_sidecars_a_read_leaves_beside_a_closed_wal_dossier_are_accepted_by_the_app() {
    let dir = TempDir::new().expect("tempdir");
    let path = dossier(&dir, JournalMode::Wal);
    // The app closed and checkpointed (its drop): nothing beside the file, or empty sidecars.
    McpAccess::at(&path)
        .list_studies(Page::first(10))
        .expect("read");
    let app = Journal::open(&path).expect("the app opens after an MCP read");
    assert!(
        !app.is_read_only(),
        "an MCP read never leaves the dossier read-only (no SidecarNotWritable)"
    );
}

/// Unique marker strings in every portfolio / watchlist / cache row: no MCP read returns any.
#[test]
fn no_read_returns_portfolio_watchlist_or_cache_data() {
    let dir = TempDir::new().expect("tempdir");
    let path = dossier(&dir, JournalMode::Delete);
    let conn = Connection::open(&path).expect("raw");
    // Marker rows, not a coherent portfolio: the references between them are not the point.
    conn.pragma_update(None, "foreign_keys", false)
        .expect("fk off for seeding");
    let denied = [
        "portfolios",
        "holdings",
        "transactions",
        "watchlist_items",
        "fx_rates",
        "price_history",
    ];
    let mut markers = Vec::new();
    for table in denied {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("columns");
        let columns: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get(1)?, r.get(2)?)))
            .expect("info")
            .collect::<rusqlite::Result<_>>()
            .expect("collect");
        let mut names = Vec::new();
        let mut values = Vec::new();
        for (name, kind) in columns {
            names.push(name.clone());
            if kind.eq_ignore_ascii_case("INTEGER") {
                values.push("1".to_string());
            } else {
                let marker = format!("MARKER-{table}-{name}");
                markers.push(marker.clone());
                values.push(format!("'{marker}'"));
            }
        }
        conn.execute(
            &format!(
                "INSERT INTO {table} ({}) VALUES ({})",
                names.join(", "),
                values.join(", ")
            ),
            [],
        )
        .expect("seed a marker row");
    }
    // A dividend transaction too (the kind column is a transaction's type).
    conn.execute(
        "UPDATE transactions SET kind = 'dividend-MARKER-kind'",
        params![],
    )
    .expect("dividend");
    markers.push("dividend-MARKER-kind".to_string());
    drop(conn);

    let access = McpAccess::at(&path);
    access
        .submit_draft(&note(&path, 1))
        .expect("a draft to list");
    let mut surface = String::new();
    surface.push_str(&format!("{:?}", access.identity().expect("identity")));
    surface.push_str(&format!(
        "{:?}",
        access.list_studies(Page::first(MAX_PAGE)).expect("studies")
    ));
    for id in [STUDY_A, STUDY_B] {
        let id = Uuid::from_u128(id);
        surface.push_str(&format!("{:?}", access.read_study(id).expect("study")));
        surface.push_str(&format!(
            "{:?}",
            access
                .read_history(id, Page::first(MAX_PAGE))
                .expect("history")
        ));
    }
    surface.push_str(&format!(
        "{:?}",
        access
            .list_drafts(DraftFilter::default(), Page::first(MAX_PAGE))
            .expect("drafts")
    ));
    assert!(
        surface.contains("NESN.SW"),
        "the surface does read the studies"
    );
    for marker in &markers {
        assert!(
            !surface.contains(marker),
            "{marker} leaked through the MCP surface"
        );
    }
    assert!(markers.len() >= 20, "every denied table carried markers");
}

#[test]
fn the_identity_names_the_dossier_read() {
    let dir = TempDir::new().expect("tempdir");
    let path = dossier(&dir, JournalMode::Wal);
    let id = McpAccess::at(&path).identity().expect("identity");
    assert_eq!(id.journal_id, Uuid::from_u128(JID));
    assert_eq!(id.path, std::fs::canonicalize(&path).expect("resolved"));
}

#[test]
fn lists_are_paged_and_bounded() {
    let dir = TempDir::new().expect("tempdir");
    let path = dossier(&dir, JournalMode::Wal);
    let access = McpAccess::at(&path);
    let first = access
        .list_studies(Page {
            offset: 0,
            limit: 1,
        })
        .expect("page 1");
    assert_eq!((first.items.len(), first.total), (1, 2));
    assert_eq!(first.items[0].security_ticker, "NESN.SW");
    let second = access
        .list_studies(Page {
            offset: 1,
            limit: 1,
        })
        .expect("page 2");
    assert_eq!(second.items[0].security_ticker, "AAPL");
    let zero = access
        .list_studies(Page {
            offset: 0,
            limit: 0,
        })
        .expect("0");
    assert_eq!(zero.items.len(), 1, "a zero limit reads one row");
    for n in 0..3 {
        access
            .submit_draft(&DraftSubmission {
                note_text: Some(format!("note {n}")),
                ..note(&path, n)
            })
            .expect("draft");
    }
    let drafts = access
        .list_drafts(
            DraftFilter::default(),
            Page {
                offset: 0,
                limit: 2,
            },
        )
        .expect("drafts");
    assert_eq!((drafts.items.len(), drafts.total), (2, 3));
    let huge = access
        .list_drafts(
            DraftFilter::default(),
            Page {
                offset: 0,
                limit: 10_000,
            },
        )
        .expect("huge");
    assert_eq!(huge.items.len(), 3, "clamped, all three fit");
}

#[test]
fn the_history_reads_newest_first_and_the_drafts_filter() {
    let dir = TempDir::new().expect("tempdir");
    let path = dossier(&dir, JournalMode::Wal);
    let mut app = Journal::open(&path).expect("open");
    let mut s = app
        .get_study(Uuid::from_u128(STUDY_A))
        .expect("get")
        .expect("present");
    s.rationale = Some("seconde version".to_string());
    app.put_study_with_history(&s, &ts("2026-09-28T11:00:00Z"))
        .expect("second snapshot");
    drop(app);
    let access = McpAccess::at(&path);
    let history = access
        .read_history(Uuid::from_u128(STUDY_A), Page::first(10))
        .expect("history");
    assert_eq!(history.total, 2);
    assert_eq!(
        history.items[0].study.rationale.as_deref(),
        Some("seconde version"),
        "newest first"
    );
    access.submit_draft(&note(&path, 1)).expect("A");
    access
        .submit_draft(&DraftSubmission {
            study_id: Some(Uuid::from_u128(STUDY_B)),
            ..note(&path, 2)
        })
        .expect("B");
    let only_b = access
        .list_drafts(
            DraftFilter {
                study_id: Some(Uuid::from_u128(STUDY_B)),
                status: None,
            },
            Page::first(10),
        )
        .expect("filter");
    assert_eq!(only_b.total, 1);
    let pending = access
        .list_drafts(
            DraftFilter {
                study_id: None,
                status: Some(DraftStatus::Pending),
            },
            Page::first(10),
        )
        .expect("pending");
    assert_eq!(pending.total, 2);
}

#[test]
fn accepted_drafts_of_every_kind_are_pending_with_their_origin_and_fingerprint() {
    let dir = TempDir::new().expect("tempdir");
    let path = dossier(&dir, JournalMode::Wal);
    let access = McpAccess::at(&path);
    access.submit_draft(&note(&path, 1)).expect("note");
    access
        .submit_draft(&cell_draft(&path, 2, 2025, "eps", "4.35"))
        .expect("cell");
    access
        .submit_draft(&judgment_draft(&path, 3, "judged_avg_high_pe", "22.5"))
        .expect("judgment");
    access
        .submit_draft(&judgment_draft(
            &path,
            4,
            "forecast_low_option",
            "recent_severe_low",
        ))
        .expect("option");
    access
        .submit_draft(&study_draft(&path, 5, "ROG.SW", "CHF"))
        .expect("study");
    let drafts = access
        .list_drafts(DraftFilter::default(), Page::first(MAX_PAGE))
        .expect("drafts")
        .items;
    assert_eq!(drafts.len(), 5);
    for d in &drafts {
        // Draft origin (NFR-A4): 100 % carry a non-blank comment, client, model and a timestamp.
        assert_eq!(d.status, DraftStatus::Pending);
        assert!(!d.comment.trim().is_empty());
        assert!(!d.origin_client.trim().is_empty() && !d.origin_model.trim().is_empty());
        assert_eq!(d.created_at, ts("2026-09-28T09:00:00Z"));
        assert!(d.decided_at.is_none());
    }
    let cell = drafts
        .iter()
        .find(|d| d.kind == DraftKind::Cell)
        .expect("cell");
    assert_eq!(
        cell.security_ticker, "NESN.SW",
        "the ticker is the stored study's"
    );
    assert!(cell.payload.contains("\"base_fingerprint\":\"fp1:"));
    let study = drafts
        .iter()
        .find(|d| d.kind == DraftKind::Study)
        .expect("study");
    assert_eq!(
        (
            study.security_ticker.as_str(),
            study.native_currency.as_deref()
        ),
        ("ROG.SW", Some("CHF"))
    );
    assert!(study.payload.contains("Roche Holding"));
}

#[test]
fn each_refusal_carries_its_code_and_writes_nothing() {
    let dir = TempDir::new().expect("tempdir");
    let path = dossier(&dir, JournalMode::Wal);
    let access = McpAccess::at(&path);
    access
        .submit_draft(&cell_draft(&path, 90, 2025, "sales", "1200"))
        .expect("a pending cell draft for target_has_pending");
    access
        .submit_draft(&study_draft(&path, 91, "ROG.SW", "CHF"))
        .expect("a pending draft study for draft_study_pending");
    let before = draft_count(&path);

    let cases: Vec<(&str, DraftSubmission)> = vec![
        (
            "dossier_mismatch",
            DraftSubmission {
                dossier: DossierIdentity {
                    journal_id: Uuid::from_u128(0xdead),
                    path: path.clone(),
                },
                ..note(&path, 1)
            },
        ),
        (
            "dossier_mismatch",
            DraftSubmission {
                dossier: DossierIdentity {
                    journal_id: Uuid::from_u128(JID),
                    path: dir.path().join("copy.db"),
                },
                ..note(&path, 2)
            },
        ),
        (
            "study_not_found",
            DraftSubmission {
                study_id: Some(Uuid::from_u128(0x404)),
                ..note(&path, 3)
            },
        ),
        (
            "empty_comment",
            DraftSubmission {
                comment: " \u{200B}\t".to_string(),
                ..note(&path, 4)
            },
        ),
        (
            "missing_origin",
            DraftSubmission {
                origin: DraftOrigin {
                    client: "claude-code".to_string(),
                    model: "\u{00A0}".to_string(),
                },
                ..note(&path, 5)
            },
        ),
        (
            "empty_note_text",
            DraftSubmission {
                note_text: Some("\u{FEFF} ".to_string()),
                ..note(&path, 6)
            },
        ),
        ("identifier_invalid", study_draft(&path, 7, "rog.sw", "CHF")),
        ("identifier_invalid", study_draft(&path, 8, "ROG.SW", "chf")),
        (
            "identifier_invalid",
            study_draft(&path, 9, "A-VERY-LONG-TICKER-NAME", "CHF"),
        ),
        (
            "field_not_draftable",
            judgment_draft(&path, 10, "current_price", "101"),
        ),
        (
            "field_not_draftable",
            judgment_draft(&path, 11, "ttm_eps", "4"),
        ),
        (
            "field_not_draftable",
            cell_draft(&path, 12, 2025, "judged_avg_high_pe", "20"),
        ),
        (
            "field_not_draftable",
            cell_draft(&path, 13, 2025, "no_such_field", "1"),
        ),
        ("year_not_in_study", cell_draft(&path, 14, 2019, "eps", "3")),
        (
            "value_unparsable",
            cell_draft(&path, 15, 2025, "eps", "4,35"),
        ),
        (
            "value_unparsable",
            judgment_draft(&path, 16, "judged_avg_low_pe", "1e3"),
        ),
        (
            "value_not_an_option",
            judgment_draft(&path, 17, "forecast_low_option", "lowest"),
        ),
        (
            "value_out_of_range",
            cell_draft(&path, 18, 2025, "sales", "1000000000000000"),
        ),
        (
            "target_has_pending",
            cell_draft(&path, 19, 2025, "sales", "1300"),
        ),
        ("study_exists", study_draft(&path, 20, "NESN.SW", "CHF")),
        ("study_exists", study_draft(&path, 21, "AAPL", "USD")),
        (
            "draft_study_pending",
            study_draft(&path, 22, "ROG.SW", "CHF"),
        ),
    ];
    for (code, sub) in cases {
        let refusal = refused(access.submit_draft(&sub));
        assert_eq!(refusal.code(), code, "{sub:?}");
    }
    assert_eq!(draft_count(&path), before, "no refused draft was written");

    // The same security in ANOTHER currency is not a duplicate (D2).
    access
        .submit_draft(&study_draft(&path, 30, "AAPL", "CHF"))
        .expect("AAPL in CHF is another study");
    // Several note drafts on one study are allowed (D4 is per cell / judgment target).
    access.submit_draft(&note(&path, 31)).expect("note 1");
    access.submit_draft(&note(&path, 32)).expect("note 2");
}

#[test]
fn an_archived_study_still_counts_as_existing_for_a_draft_study() {
    let dir = TempDir::new().expect("tempdir");
    let path = dossier(&dir, JournalMode::Wal);
    Journal::open(&path)
        .expect("open")
        .set_study_status(Uuid::from_u128(STUDY_A), "archived")
        .expect("archive");
    let access = McpAccess::at(&path);
    let refusal = refused(access.submit_draft(&study_draft(&path, 1, "nesn.sw", "CHF")));
    assert_eq!(
        refusal.code(),
        "identifier_invalid",
        "lower-case is not the rule's form"
    );
    let refusal = refused(access.submit_draft(&study_draft(&path, 2, "NESN.SW", "CHF")));
    match refusal {
        SubmissionRefusal::StudyExists { study_id, .. } => {
            assert_eq!(study_id, Uuid::from_u128(STUDY_A))
        }
        other => panic!("expected study_exists, got {other:?}"),
    }
}

#[test]
fn a_call_that_is_not_well_formed_is_a_failure_not_a_refusal() {
    let dir = TempDir::new().expect("tempdir");
    let path = dossier(&dir, JournalMode::Wal);
    let access = McpAccess::at(&path);
    for sub in [
        DraftSubmission {
            study_id: None,
            ..note(&path, 1)
        },
        DraftSubmission {
            note_text: None,
            ..note(&path, 2)
        },
        DraftSubmission {
            created_at: ts("yesterday"),
            ..note(&path, 3)
        },
        DraftSubmission {
            target: None,
            ..cell_draft(&path, 4, 2025, "eps", "4")
        },
    ] {
        match access.submit_draft(&sub) {
            Err(SubmitError::Failed(Error::McpInvalidCall { .. })) => {}
            other => panic!("expected an invalid call, got {other:?}"),
        }
    }
    assert_eq!(draft_count(&path), 0);
}

#[test]
fn a_missing_dossier_is_a_failure_and_creates_nothing() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("absent.db");
    let access = McpAccess::at(&path);
    assert!(matches!(
        access.list_studies(Page::first(1)),
        Err(Error::Sqlite(_))
    ));
    assert!(matches!(
        access.submit_draft(&note(&path, 1)),
        Err(SubmitError::Failed(Error::Sqlite(_)))
    ));
    assert!(!path.exists(), "the MCP access never creates a dossier");
}

/// AC 8 — a submission races the app's `delete_study` on the same study, many times: the check and
/// the insert are one `IMMEDIATE` transaction (and `foreign_keys = ON` backs it), so no pending
/// draft ever outlives its study.
#[test]
fn a_submission_racing_the_deletion_of_its_study_never_leaves_an_orphan() {
    let dir = TempDir::new().expect("tempdir");
    let path = dossier(&dir, JournalMode::Wal);
    const ROUNDS: u128 = 40;
    for round in 0..ROUNDS {
        let study_id = 0x9000 + round;
        {
            let mut app = Journal::open(&path).expect("open");
            app.put_study(&study(study_id, "RACE", "EUR", "2026-09-28T12:00:00Z"))
                .expect("study");
        }
        let barrier = Arc::new(Barrier::new(2));
        let (b1, b2) = (Arc::clone(&barrier), Arc::clone(&barrier));
        let (p1, p2) = (path.clone(), path.clone());
        let submit = std::thread::spawn(move || {
            let sub = DraftSubmission {
                study_id: Some(Uuid::from_u128(study_id)),
                ..note(&p1, 0x5000 + round)
            };
            b1.wait();
            McpAccess::at(&p1).submit_draft(&sub)
        });
        let delete = std::thread::spawn(move || {
            let mut app = Journal::open(&p2).expect("open");
            b2.wait();
            app.delete_study(Uuid::from_u128(study_id))
        });
        let submitted = submit.join().expect("submit thread");
        delete
            .join()
            .expect("delete thread")
            .expect("the delete succeeds");
        match submitted {
            Ok(_) => {}
            Err(SubmitError::Refused(SubmissionRefusal::StudyNotFound { .. })) => {}
            other => panic!("round {round}: unexpected outcome {other:?}"),
        }
    }
    let conn = Connection::open(&path).expect("raw");
    let orphans: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM ai_drafts d WHERE d.study_id IS NOT NULL
               AND NOT EXISTS (SELECT 1 FROM studies s WHERE s.id = d.study_id)",
            [],
            |r| r.get(0),
        )
        .expect("count");
    assert_eq!(orphans, 0, "no draft outlived its study");
}
