//! Studies wiring: dashboard + open-form lifecycle — create (Story 2.2), open + fold/regime
//! restore (Stories 2.3/2.5-view), search / sort / filter + archive / un-archive / delete behind
//! the confirm overlay (Story 2.12), the single-study JSON export/import (Story 5.2, FR59) and
//! faithful PDF export (Story 5.6, FR52) with their `exports/`-folder file writers (ADD7/8 —
//! never beside the live journal), the verify-engine replay (Story 2.13, FR9) and the read-only
//! demo study (FR62), plus the `refresh_studies` dashboard re-render. Moved verbatim from
//! `main.rs` — no logic change.

use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use steadyinvest_persistence::StudySummary;
use uuid::Uuid;

use crate::config::StudyViewState;
use crate::regime::Regime;
use crate::state::JournalState;
use crate::wiring::push::{push_form, push_view_state};
use crate::wiring::watchlist::refresh_watchlist;
use crate::wiring::{Session, persist};
use crate::wiring::{list_notice, study_notice};
use crate::{FixtureLine, MainWindow, Prefs, ScenarioCompareState, Studies, StudyRow, Verify};
use crate::{regime, state, viewmodel};

/// Write a study's export envelope to a file (Story 5.2, FR59) and return its path. The file lands in
/// an `exports/` folder under the OS data dir — **never** beside the live journal DB (ADD7/8
/// sync-safety; the native picker + a user-chosen sync target is Story 5.5). Named by the study id
/// (stable, unique). `app` owns the file I/O — `contract` only produced the string.
fn write_study_export(id: Uuid, json: &str) -> std::io::Result<std::path::PathBuf> {
    let dir = directories::ProjectDirs::from("", "", "steadyinvest")
        .map(|d| d.data_dir().join("exports"))
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no OS data directory"))?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("study-{id}.json"));
    std::fs::write(&path, json)?;
    Ok(path)
}

/// The default `exports/` folder under the OS data dir — the native PDF save picker (issue #106)
/// opens here, but the user is free to save anywhere. `None` when the OS exposes no data directory.
fn default_exports_dir() -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("", "", "steadyinvest").map(|d| d.data_dir().join("exports"))
}

/// A filesystem-safe default file stem from a ticker (e.g. `AAPL.US` → `AAPL.US`, `BRK/B` → `BRK_B`).
/// Path separators and control/space characters become `_`; the common `.`/`-` are kept.
fn safe_stem(ticker: &str) -> String {
    let stem: String = ticker
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if stem.is_empty() {
        "etude".to_string()
    } else {
        stem
    }
}

/// G1 final (L12) — the PDF's path as picked, with `.pdf` appended unless its name already ends
/// so (any case, « .pdf » alone included); `true` when the path was changed. rfd does not force
/// the filter's extension everywhere, and a picked name such as « etude-NESN.SW » HAS an extension
/// (« SW ») — so the test is the name's ending, and the suffix is appended, never swapped for the
/// name's own last dot-part. A name ending in a bare dot (« etude. ») takes « pdf », never
/// « ..pdf ».
fn with_pdf_extension(path: std::path::PathBuf) -> (std::path::PathBuf, bool) {
    let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
        return (path, false);
    };
    let lower = name.to_lowercase();
    if lower.ends_with(".pdf") {
        return (path, false);
    }
    let named = match name.strip_suffix('.') {
        Some(stem) => format!("{stem}.pdf"),
        None => format!("{name}.pdf"),
    };
    (path.with_file_name(named), true)
}

/// One pickable study (G1 decision 3, #237): the `id` is the key carried end to end; the `label`
/// is display only — « TICKER », or « TICKER · CUR » when the ticker has several studies (then
/// « · date » and « · n » only if a currency is still shared or unreadable), unique within one
/// list. `ordinal` is that « n » — the one disambiguator the column header (ticker, currency,
/// date) does not already show, so the comparison carries it into the header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StudyChoice {
    pub id: Uuid,
    pub label: String,
    pub ordinal: Option<usize>,
}

/// The facts a choice label is built from. `currency` is read only for an ambiguous ticker;
/// `None` there means the study could not be read (it stays listed: it EXISTS, and compared it
/// reads « indisponible »).
#[derive(Debug, Clone)]
struct ChoiceFacts {
    id: Uuid,
    ticker: String,
    currency: Option<String>,
    date: String,
}

/// PURE: label the studies (the discriminator rule: the label disambiguates, the id identifies),
/// sorted by label. Every label is unique, so a drop-down value maps back to one study.
fn label_choices(facts: &[ChoiceFacts]) -> Vec<StudyChoice> {
    let count = |pred: &dyn Fn(&ChoiceFacts) -> bool| facts.iter().filter(|f| pred(f)).count();
    let mut out: Vec<StudyChoice> = facts
        .iter()
        .map(|f| {
            let label = if count(&|g| g.ticker == f.ticker) == 1 {
                f.ticker.clone()
            } else {
                match &f.currency {
                    Some(cur)
                        if count(&|g| g.ticker == f.ticker && g.currency == f.currency) == 1 =>
                    {
                        format!("{} · {cur}", f.ticker)
                    }
                    Some(cur) => format!("{} · {cur} · {}", f.ticker, f.date),
                    // Unreadable: no currency to name — the date (then a number) tells it apart.
                    None => format!("{} · {}", f.ticker, f.date),
                }
            };
            StudyChoice {
                id: f.id,
                label,
                ordinal: None,
            }
        })
        .collect();
    // Same ticker, currency and day: number them in (date, id) order — still one label per study.
    let mut totals: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for c in &out {
        *totals.entry(c.label.clone()).or_insert(0) += 1;
    }
    let mut ordered: Vec<usize> = (0..facts.len()).collect();
    ordered.sort_by(|&a, &b| (&facts[a].date, facts[a].id).cmp(&(&facts[b].date, facts[b].id)));
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for i in ordered {
        if totals.get(&out[i].label).is_some_and(|&t| t > 1) {
            let n = seen.entry(out[i].label.clone()).or_insert(0);
            *n += 1;
            out[i].label = format!("{} · {n}", out[i].label);
            out[i].ordinal = Some(*n);
        }
    }
    out.sort_by(|a, b| (&a.label, a.id).cmp(&(&b.label, b.id)));
    out
}

/// PURE: the choice facts of a listing. `currency_of` reads a study's currency — asked only for
/// an ambiguous ticker: `Ok(None)` (gone between the listing and the read) leaves the study out,
/// it no longer exists; `Err` (unreadable) keeps it, currency-less.
fn choice_facts(
    summaries: &[StudySummary],
    currency_of: impl Fn(Uuid) -> Result<Option<String>, String>,
) -> Vec<ChoiceFacts> {
    let mut facts: Vec<ChoiceFacts> = Vec::with_capacity(summaries.len());
    for s in summaries {
        let ticker = s.security_ticker.to_uppercase();
        let ambiguous = summaries
            .iter()
            .filter(|o| o.security_ticker.eq_ignore_ascii_case(&ticker))
            .count()
            > 1;
        let currency = if ambiguous {
            match currency_of(s.id) {
                Ok(Some(cur)) => Some(cur.to_uppercase()),
                Ok(None) => continue,
                Err(_) => None,
            }
        } else {
            None
        };
        facts.push(ChoiceFacts {
            id: s.id,
            ticker,
            currency,
            date: s.created_at.0.chars().take(10).collect(),
        });
    }
    facts
}

/// The dossier's studies as pickable choices (G1 decision 3) — for the comparison's five
/// drop-downs, and for any other study picker. Fallible (#95): `Err` is a failure of the LISTING,
/// which the picker states — never an empty list passed off as « aucune étude ». One unreadable
/// study does not hide the others: it stays listed (see [`choice_facts`]).
pub(crate) fn study_choices(state: &JournalState) -> Result<Vec<StudyChoice>, String> {
    let summaries = state.try_list_studies()?;
    let facts = choice_facts(&summaries, |id| {
        state
            .try_get_study(id)
            .map(|s| s.map(|study| study.native_currency))
    });
    Ok(label_choices(&facts))
}

/// Rebuild the dashboard list from the journal and mirror the read-only flag into the `Studies`
/// global. Called on startup and after every create.
pub(crate) fn refresh_studies(ui: &MainWindow, state: &JournalState) {
    let studies = ui.global::<Studies>();
    // Story 7.1 / G1 decision 3: the comparison's five pickers list STUDIES (ids), re-pushed here
    // so the list screen never shows a stale drop-down.
    crate::wiring::comparison::push_choices(ui, state);
    // The add-position dialog offers the dossier's study tickers.
    {
        let mut tickers: Vec<String> = state
            .list_studies()
            .into_iter()
            .map(|s| s.security_ticker.to_uppercase())
            .collect();
        tickers.sort();
        tickers.dedup();
        let tickers: Vec<SharedString> = tickers.into_iter().map(SharedString::from).collect();
        ui.global::<crate::Holdings>()
            .set_study_tickers(ModelRc::new(VecModel::from(tickers)));
    }
    // The dashboard view state (search/sort/filter) lives on the `Studies` global — read it back and
    // curate the persistence summaries (Story 2.12). Deterministic, pure (`viewmodel::studies::curate`).
    let summaries = state.list_studies();
    // Issue #107: the estimated potential per study (§5 projected total annualized return). Computed
    // app-side via `build_snapshot` (Cardinal Rule kept: `curate` only READS this) and formatted with
    // the user's number format (mirrored on `Prefs`, as `replacement.rs` already reads it). A study
    // that does not compute / withholds the return contributes an absent value → sorts last, "—" shown.
    // Personal scale: one snapshot per study per refresh is sub-ms (the holdings register already does
    // the same per row).
    let format =
        crate::viewmodel::format::NumberFormat::parse(&ui.global::<Prefs>().get_number_format())
            .unwrap_or_default();
    let mut returns: std::collections::HashMap<uuid::Uuid, viewmodel::studies::StudyReturn> =
        std::collections::HashMap::new();
    for summary in &summaries {
        let Some(study) = state.get_study(summary.id) else {
            continue;
        };
        // ONE snapshot per study feeds both the §5 potential (issue #107) and the issue #148
        // "à compléter" flag — no second engine pass. A study that fails to normalize is "unknown":
        // absent potential ("—") and NOT flagged incomplete (a broken normalize must not shout).
        let snapshot = crate::viewmodel::engine::build_snapshot(&study).ok();
        // Issue #189: when ONLY the dividend history is missing (EPS chain judged) the total is
        // withheld by the core (absence ≠ 0) — the column then shows and sorts on the annualised
        // appreciation alone, marked « (hors div.) » by `fmt_total_return`.
        let return_outputs = snapshot.as_ref().map(|snap| &snap.outputs().returns);
        let value = return_outputs.and_then(|r| {
            r.projected_total_annualized_return_pct
                .or_else(|| r.appreciation_only_potential())
        });
        let display = match return_outputs {
            Some(r) => crate::viewmodel::engine::fmt_total_return(r, format),
            None => crate::viewmodel::form::EMPTY_SLOT.to_string(),
        };
        let incomplete = snapshot
            .as_ref()
            .is_some_and(crate::viewmodel::engine::study_incomplete);
        // 2026-07-12: the present-price zone/position off the SAME snapshot — the list noun.
        // buy/neutral/sell inside the band; below/above outside it (an honest distinct state, not
        // a blank and not a forced buy/sell); "" when there is no band or no price.
        let zone = match &snapshot {
            Some(snap) => crate::viewmodel::engine::zone_position_key(
                &snap.outputs().risk_reward,
                study.judgment.current_price.map(|m| m.as_decimal()),
            ),
            None => "",
        };
        returns.insert(
            summary.id,
            viewmodel::studies::StudyReturn {
                value,
                display,
                incomplete,
                zone,
                // The user-entered company name shown after the ticker on the list row (2026-07-12).
                company_name: study.company_name.clone().unwrap_or_default(),
            },
        );
    }
    let rows: Vec<StudyRow> = viewmodel::studies::curate(
        &summaries,
        studies.get_search_query().as_str(),
        viewmodel::studies::SortKey::from_wire(studies.get_sort_key().as_str()),
        studies.get_sort_descending(),
        viewmodel::studies::StatusFilter::from_wire(studies.get_status_filter().as_str()),
        &returns,
    );
    studies.set_study_count(summaries.len() as i32);
    studies.set_rows(ModelRc::new(VecModel::from(rows)));
    studies.set_read_only(state.is_read_only());
}

/// Wire the studies domain: create / open (with per-study view-state restore) / fold / regime,
/// the dashboard search / sort / filter + lifecycle actions behind their confirm overlay, the
/// study JSON export/import + PDF export, and the verify / demo surfaces.
pub(crate) fn wire_studies(ui: &MainWindow, s: &Session) {
    let Session {
        journal_state,
        config,
        config_path,
        current_study,
        compare_study,
        pending_study_action,
        ..
    } = s;
    // Create-study intent: validate + persist via the injected sources, then refresh the list.
    // A refused create (blank input / read-only / save failure) surfaces a neutral banner; a
    // successful one clears it.
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        ui.global::<Studies>()
            .on_create_study(move |ticker, currency, company_name| {
                let ui = ui_weak.unwrap();
                // G1 review (decision 8): the optional company name rides the same first write.
                let result = journal_state.borrow_mut().create_study_named(
                    &ticker,
                    &currency,
                    &company_name,
                );
                let studies = ui.global::<Studies>();
                let written = result.is_ok();
                match result {
                    Ok(_id) => studies.set_notice(SharedString::new()),
                    Err(message) => crate::wiring::dialog::refuse(&ui, &message),
                }
                refresh_studies(&ui, &journal_state.borrow());
                // Report whether a study was written so the dialog closes only then.
                written
            });
    }

    // 2026-07-12: the in-form « Retour aux études » button flips `study-open` in pure Slint and
    // never passes through `screen-activated` — this explicit intent re-derives the list rows
    // (per-row §5 potential / « à compléter » / zone) from the edits made in the form.
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        ui.global::<Studies>().on_list_refresh_requested(move || {
            let ui = ui_weak.unwrap();
            refresh_studies(&ui, &journal_state.borrow());
        });
    }

    // ── Issue #34 (FR51, PR 2) — the durable « Historique » timeline of the open study: open /
    // close the read-only panel, expand one entry to its avant → après detail. The rows rebuild
    // on open and, while open, after every mutation (via `push_form` — the panel never shows a
    // stale timeline beside a fresh form). ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
        ui.global::<Studies>().on_open_history(move || {
            let ui = ui_weak.unwrap();
            let Some(id) = current_study.borrow().clone() else {
                return;
            };
            let Ok(id) = Uuid::parse_str(&id) else {
                return;
            };
            ui.global::<Studies>().set_history_open(true);
            crate::wiring::push::push_history(
                &ui,
                &journal_state.borrow(),
                id,
                config.borrow().number_format,
            );
        });
    }
    {
        let ui_weak = ui.as_weak();
        ui.global::<Studies>().on_close_history(move || {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            studies.set_history_open(false);
            studies.set_history_rows(ModelRc::new(VecModel::from(
                Vec::<crate::HistoryEntryRow>::new(),
            )));
            studies.set_history_detail_id(SharedString::new());
            studies
                .set_history_detail_lines(ModelRc::new(VecModel::from(Vec::<SharedString>::new())));
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
        ui.global::<Studies>().on_toggle_history_entry(move |id| {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            // A second click on the expanded entry collapses it.
            if studies.get_history_detail_id() == id {
                studies.set_history_detail_id(SharedString::new());
                studies.set_history_detail_lines(ModelRc::new(VecModel::from(
                    Vec::<SharedString>::new(),
                )));
                return;
            }
            let Some(study_id) = current_study.borrow().clone() else {
                return;
            };
            let (Ok(study_id), Ok(snapshot_id)) =
                (Uuid::parse_str(&study_id), Uuid::parse_str(&id))
            else {
                return;
            };
            let state = journal_state.borrow();
            let format = config.borrow().number_format;
            // The detail diffs THIS snapshot against its predecessor — located by the listing
            // order (oldest first). Any read failure marks the whole panel « indisponible »
            // (the #95 discipline), never a silently empty detail.
            let detail = (|| -> Result<Vec<String>, String> {
                let summaries = state.try_list_study_history(study_id)?;
                let index = summaries
                    .iter()
                    .position(|s| s.id == snapshot_id)
                    .ok_or_else(String::new)?; // vanished between renders — treat as unavailable
                let next = state
                    .try_get_history_snapshot(snapshot_id)?
                    .ok_or_else(String::new)?;
                let prev = match index.checked_sub(1) {
                    Some(i) => Some(
                        state
                            .try_get_history_snapshot(summaries[i].id)?
                            .ok_or_else(String::new)?,
                    ),
                    None => None,
                };
                Ok(viewmodel::history::history_detail(
                    prev.as_ref(),
                    &next,
                    format,
                ))
            })();
            match detail {
                Ok(lines) => {
                    studies.set_history_detail_id(id);
                    studies.set_history_detail_lines(ModelRc::new(VecModel::from(
                        lines
                            .into_iter()
                            .map(SharedString::from)
                            .collect::<Vec<_>>(),
                    )));
                }
                Err(_) => {
                    studies.set_history_unavailable(true);
                    studies.set_history_detail_id(SharedString::new());
                    studies
                        .set_history_detail_lines(ModelRc::new(VecModel::from(
                            Vec::<SharedString>::new(),
                        )));
                }
            }
        });
    }

    // ── Story 5.2 (FR59) — export / import a single study as a portable file. The envelope is the
    // serialized data contract + schema_version + integrity hash (NOT a raw .db); `contract` owns the
    // envelope, `app` owns the file I/O. Import is picker-fed (below) but stays path-based. ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        ui.global::<Studies>().on_export_study(move |id| {
            let ui = ui_weak.unwrap();
            let Ok(uuid) = Uuid::parse_str(&id) else {
                return;
            };
            let outcome = match journal_state.borrow().export_study(uuid) {
                Ok(json) => match write_study_export(uuid, &json) {
                    Ok(path) => Ok(format!("{} {}", state::MSG_STUDY_EXPORTED, path.display())),
                    // G1 final (M4): the write failure is named in French, the OS cause logged.
                    Err(error) => {
                        tracing::warn!(study_id = %uuid, %error, "study export write failed");
                        Err(state::MSG_EXPORT_WRITE_FAILED.to_string())
                    }
                },
                Err(message) => Err(message),
            };
            match outcome {
                // F4: an export outcome never overwrites another source's notice.
                Ok(notice) => list_notice::show(&ui, list_notice::Source::Export, &notice),
                Err(message) => crate::wiring::dialog::refuse(&ui, &message),
            }
        });
    }
    {
        // Story 5.6 (FR52) + issue #106: export a study's faithful, neutral, greyscale PDF via the
        // `report` crate (UI-independent, from `core`/`contract`). A native `rfd` save picker lets the
        // user choose the destination directory + filename (defaulting into exports/ with a ticker-
        // named file); cancel is a silent no-op. Read-only — rendering writes no journal.
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        ui.global::<Studies>().on_export_study_pdf(move |id| {
            let ui = ui_weak.unwrap();
            let Ok(uuid) = Uuid::parse_str(&id) else {
                return;
            };
            // Fetch + render BEFORE opening a dialog, so a study that does not compute never prompts
            // for a destination it can't fill. G1 final (M3): each refusal names its own cause —
            // a read failure (« illisible »), a vanished study (« introuvable »), a study whose data
            // do not prepare — never « L'enregistrement a échoué », which nothing here attempted.
            let read = journal_state.borrow().try_get_study(uuid);
            let study = match read {
                Ok(Some(study)) => study,
                Ok(None) => {
                    crate::wiring::dialog::refuse(&ui, state::MSG_EXPORT_MISSING);
                    return;
                }
                Err(_) => {
                    // The cause is logged by `try_get_study`.
                    crate::wiring::dialog::refuse(&ui, state::MSG_EXPORT_UNREADABLE);
                    return;
                }
            };
            // G1 I: the PDF's figures in the user's number format, as on the screen.
            let numbers = journal_state.borrow().number_format().report_style();
            let bytes = match steadyinvest_report::render_study_pdf(&study, numbers) {
                Ok(bytes) => bytes,
                Err(error) => {
                    // The study does not compute as entered — a named refusal, no panic, no leak.
                    tracing::warn!(study_id = %uuid, %error, "study PDF not rendered");
                    crate::wiring::dialog::refuse(&ui, state::MSG_STUDY_PDF_UNRENDERABLE);
                    return;
                }
            };
            // Native save picker on the UI thread (modal — the established `rfd` pattern, cf. the
            // journal export/create rails). Cancel → no notice, nothing written.
            let mut dialog = rfd::FileDialog::new()
                .set_title("Exporter l'étude en PDF")
                .add_filter("PDF", &["pdf"])
                .set_file_name(format!("etude-{}.pdf", safe_stem(&study.security_ticker)));
            if let Some(dir) = default_exports_dir() {
                let _ = std::fs::create_dir_all(&dir); // best-effort so the dialog opens there
                dialog = dialog.set_directory(dir);
            }
            let Some(path) = dialog.save_file() else {
                return;
            };
            // rfd does not force the filter extension on every platform — ensure `.pdf` (L12).
            let (path, renamed) = with_pdf_extension(path);
            // The picker asked about overwriting the name it returned, not the one completed
            // here: an existing file under the completed name is never overwritten in silence.
            if renamed && path.exists() {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                crate::wiring::dialog::refuse(&ui, &state::export_name_taken_message(&name));
                return;
            }
            match std::fs::write(&path, &bytes) {
                // F4: an export outcome never overwrites another source's notice (M4).
                Ok(()) => list_notice::show(
                    &ui,
                    list_notice::Source::Export,
                    &format!("{} {}", state::MSG_STUDY_EXPORTED, path.display()),
                ),
                // A write failure is a refusal, like its neighbours — named in French, the OS
                // cause logged (M4).
                Err(error) => {
                    tracing::warn!(study_id = %uuid, %error, "study PDF write failed");
                    crate::wiring::dialog::refuse(&ui, state::MSG_EXPORT_WRITE_FAILED);
                }
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        ui.global::<Studies>().on_import_study(move |path| {
            let ui = ui_weak.unwrap();
            let outcome = match std::fs::read_to_string(path.as_str()) {
                Ok(json) => match journal_state.borrow_mut().import_study(&json) {
                    // Surface an overwrite of a pre-existing study distinctly from a fresh import.
                    Ok((_id, true)) => Ok(state::MSG_STUDY_UPDATED),
                    Ok((_id, false)) => Ok(state::MSG_STUDY_IMPORTED),
                    Err(message) => Err(message),
                },
                // An unreadable path is the malformed/unreadable case — a neutral refusal, no panic.
                Err(_) => Err(state::MSG_IMPORT_MALFORMED.to_string()),
            };
            match outcome {
                // F4: an import outcome never overwrites another source's failure.
                Ok(notice) => list_notice::show(&ui, list_notice::Source::Import, notice),
                Err(message) => crate::wiring::dialog::refuse(&ui, &message),
            }
            refresh_studies(&ui, &journal_state.borrow());
        });
    }

    {
        // Native `rfd` open picker on the UI thread (modal — the established rail, cf. the journal
        // open/create dialogs). It feeds the SAME path-based `import-study` callback, so the verify-
        // before-write logic has one code path (headless-tested); cancel → no notice, nothing read.
        let ui_weak = ui.as_weak();
        ui.global::<Studies>().on_pick_and_import_study(move || {
            let ui = ui_weak.unwrap();
            let mut dialog = rfd::FileDialog::new()
                .set_title("Importer une étude")
                .add_filter("Étude exportée (JSON)", &["json"]);
            if let Some(dir) = default_exports_dir().filter(|d| d.is_dir()) {
                dialog = dialog.set_directory(dir);
            }
            let Some(path) = dialog.pick_file() else {
                return; // the user cancelled the dialog
            };
            ui.global::<Studies>()
                .invoke_import_study(path.to_string_lossy().as_ref().into());
        });
    }

    // G1 J review — the ONE close path of an open study (in-form « Retour », the nav rail, the
    // Portefeuille « go to studies »): forget the open study's id, so a late fetch result is routed
    // to the list and no edit rail can write a study that is no longer shown.
    {
        let ui_weak = ui.as_weak();
        let current_study = Rc::clone(current_study);
        ui.global::<Studies>().on_close_study(move || {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            *current_study.borrow_mut() = None;
            studies.set_study_open(false);
            studies.set_demo_active(false);
        });
    }

    // Money surfaces as formatted strings via the form adapter (the only float→string boundary), and
    // the persisted per-study fold/regime view-state is restored (default = Entry + all open).
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
        let compare_study = Rc::clone(compare_study);
        ui.global::<Studies>().on_open_study(move |id_text| {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            let Ok(id) = Uuid::parse_str(&id_text) else {
                return;
            };
            let Some(study) = journal_state.borrow().get_study(id) else {
                return;
            };
            // Story 2.9 — a freshly-opened study starts with an empty undo/redo history (the edit
            // history is per open study, in-memory, never carried across reopen). Reset BEFORE
            // push_form so the mirrored can-undo/can-redo flags read empty.
            journal_state.borrow_mut().reset_undo();
            // Also discard any scenario-compare state from a previous study (review P3) — its overlay
            // and cached baseline must never survive into a different study.
            *compare_study.borrow_mut() = None;
            studies.set_scenario_compare(ScenarioCompareState::default());
            // G1 J: the study slot starts empty — nothing said of the previous study (a fetch
            // result, a refusal) may read as this one's. Before the render, so a normalize failure
            // of THIS study still shows.
            study_notice::reset(&ui);
            let format = config.borrow().number_format;
            push_form(&ui, &journal_state.borrow(), &study, format);
            // A freshly-opened form has no active entry cell (the cursor appears on first focus).
            studies.set_active_year(-1);
            studies.set_active_field(SharedString::new());
            studies.set_active_source(SharedString::new());
            studies.set_active_warning(SharedString::new());
            // Defensively clear any stuck drag state (review P5): if a previous study was closed
            // mid-drag the `up`/`cancel` may never have fired, which would leave the form's scroll
            // disabled. Opening a study always starts from a clean, scrollable state.
            studies.set_judgment_dragging(false);
            let view_state = config
                .borrow()
                .study_view_state
                .get(id_text.as_str())
                .cloned()
                .unwrap_or_default();
            push_view_state(&ui, &view_state);
            *current_study.borrow_mut() = Some(id_text.to_string());
            studies.set_demo_active(false); // opening a real study leaves the demo (Story 2.13)
            studies.set_study_open(true);
        });
    }

    // Fold a section: mutate this study's `study_view_state` (validate index → mutate → persist), then
    // push the new state back (one source of truth). Mirrors the 2.2 Prefs persistence shape — no
    // silent `.ok()`: a save failure surfaces via `persist`.
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(config);
        let path = config_path.clone();
        let current_study = Rc::clone(current_study);
        ui.global::<Studies>().on_toggle_fold(move |index, open| {
            let Some(id) = current_study.borrow().clone() else {
                return;
            };
            if !(0..regime::SECTION_COUNT as i32).contains(&index) {
                return;
            }
            let ui = ui_weak.unwrap();
            let new_state = {
                let mut cfg = config.borrow_mut();
                let entry = cfg.study_view_state.entry(id).or_default();
                entry.folds[index as usize] = open;
                entry.clone()
            };
            persist(path.as_ref(), &config.borrow());
            push_view_state(&ui, &new_state);
        });
    }

    // Switch regime: apply the regime's fold preset + swap the regime token snapshot, persist, push.
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(config);
        let path = config_path.clone();
        let current_study = Rc::clone(current_study);
        ui.global::<Studies>().on_set_regime(move |value| {
            let Some(new_regime) = Regime::parse(&value) else {
                return;
            };
            let Some(id) = current_study.borrow().clone() else {
                return;
            };
            // Selecting the already-active regime is a no-op (AC3: only an actual *switch* applies
            // the fold preset). Re-applying the preset here would silently clobber the user's manual
            // fold edits made within this regime.
            let current_regime = config
                .borrow()
                .study_view_state
                .get(&id)
                .map(|state| state.regime)
                .unwrap_or_default();
            if current_regime == new_regime {
                return;
            }
            let ui = ui_weak.unwrap();
            let new_state = {
                let mut cfg = config.borrow_mut();
                let entry = cfg.study_view_state.entry(id).or_default();
                entry.regime = new_regime;
                entry.folds = new_regime.fold_preset();
                entry.clone()
            };
            persist(path.as_ref(), &config.borrow());
            push_view_state(&ui, &new_state);
        });
    }

    // ── Story 2.12 — dashboard search / sort / filter. The view state lives on the `Studies` global;
    //    each control updates it then re-lists + re-curates (pure `viewmodel::studies::curate`). ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        ui.global::<Studies>().on_set_search(move |text| {
            let ui = ui_weak.unwrap();
            ui.global::<Studies>().set_search_query(text);
            refresh_studies(&ui, &journal_state.borrow());
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        ui.global::<Studies>().on_set_sort(move |key, descending| {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            studies.set_sort_key(key);
            studies.set_sort_descending(descending);
            refresh_studies(&ui, &journal_state.borrow());
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        ui.global::<Studies>().on_set_status_filter(move |filter| {
            let ui = ui_weak.unwrap();
            ui.global::<Studies>().set_status_filter(filter);
            refresh_studies(&ui, &journal_state.borrow());
        });
    }

    // ── Story 2.12 — archive (soft) / un-archive / delete (hard), each behind the confirm overlay
    //    (mirrors the unlock request→confirm→cancel pattern; a SEPARATE `study-action` channel). ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let pending_study_action = Rc::clone(pending_study_action);
        ui.global::<Studies>()
            .on_request_study_action(move |action, id_text| {
                let ui = ui_weak.unwrap();
                let studies = ui.global::<Studies>();
                let Ok(id) = Uuid::parse_str(&id_text) else {
                    return;
                };
                // The ticker for the fact-stating prompt (user data, not scanned); absent study → bail.
                let Some(study) = journal_state.borrow().get_study(id) else {
                    return;
                };
                let action = action.to_string();
                let message = state::study_action_confirm_message(&action, &study.security_ticker);
                let destructive = action == "delete";
                *pending_study_action.borrow_mut() = Some((action, id));
                // The UX pass: the prompt is a modal confirm (the overlay derives the title and
                // the verb from `study-action-destructive`).
                studies.set_study_action_destructive(destructive);
                crate::wiring::dialog::confirm(&ui, "study-action", &message);
            });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let current_study = Rc::clone(current_study);
        let pending_study_action = Rc::clone(pending_study_action);
        ui.global::<Studies>().on_confirm_study_action(move || {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            let Some((action, id)) = pending_study_action.borrow_mut().take() else {
                return;
            };
            // The ticker for the completion notice, captured before a delete removes the row.
            let ticker = journal_state
                .borrow()
                .get_study(id)
                .map(|s| s.security_ticker)
                .unwrap_or_default();
            let result = {
                let mut st = journal_state.borrow_mut();
                match action.as_str() {
                    "archive" => st.archive_study(id),
                    "unarchive" => st.unarchive_study(id),
                    _ => st.delete_study(id),
                }
            };
            match result {
                Ok(()) => {
                    // F4: the outcome never overwrites another source's failure.
                    list_notice::show(
                        &ui,
                        list_notice::Source::StudyAction,
                        &state::study_action_done_message(&action, &ticker),
                    );
                    // If the affected study is the one currently open, close it back to the dashboard
                    // (a hidden/removed study must not stay mounted).
                    let is_open =
                        current_study.borrow().as_deref() == Some(id.to_string().as_str());
                    if is_open {
                        *current_study.borrow_mut() = None;
                        studies.set_study_open(false);
                    }
                    refresh_studies(&ui, &journal_state.borrow());
                    // A delete clears any watchlist soft link to this study (Story 4.1) — re-render
                    // the watchlist so a linked row drops its (now-cleared) study link.
                    refresh_watchlist(&ui, &journal_state.borrow());
                }
                Err(message) => crate::wiring::dialog::refuse(&ui, &message),
            }
        });
    }
    {
        let pending_study_action = Rc::clone(pending_study_action);
        ui.global::<Studies>().on_cancel_study_action(move || {
            *pending_study_action.borrow_mut() = None;
        });
    }

    // ── Story 2.13 — verify engine (FR9): replay the bundled golden fixtures through core and push the
    //    method identity + per-fixture pass/deviation report into the `Verify` global (Réglages hub). ──
    {
        let ui_weak = ui.as_weak();
        ui.global::<Verify>().on_run(move || {
            let ui = ui_weak.unwrap();
            let report = viewmodel::verify::run();
            let verify = ui.global::<Verify>();
            verify.set_method_version(report.method_version.as_str().into());
            verify.set_method_fingerprint(report.method_fingerprint.as_str().into());
            verify.set_summary(state::verify_summary(report.passed_count, report.total).into());
            verify.set_all_passed(report.all_passed());
            let lines: Vec<FixtureLine> = report
                .results
                .iter()
                .map(|r| FixtureLine {
                    id: r.id.as_str().into(),
                    passed: r.passed,
                    detail: r.deviations.join(" ; ").into(),
                })
                .collect();
            verify.set_results(ModelRc::new(VecModel::from(lines)));
            verify.set_ran(true);
        });
    }

    // ── Story 2.13 — load the read-only demonstration study (FR62): build the bundled worked-example
    //    in memory and render it via `push_form`. `current_study` stays None, so every edit rail
    //    no-ops and nothing reaches the journal. `demo-active` drives the "lecture seule" banner. ──
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(config);
        let journal_state = Rc::clone(journal_state);
        let current_study = Rc::clone(current_study);
        ui.global::<Studies>().on_load_demo(move || {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            let format = config.borrow().number_format;
            match viewmodel::verify::demo_study() {
                Ok(study) => {
                    // The demo has no persisted view-state or undo history of its own: render it with
                    // the default (entry regime, all sections open) and an empty undo stack, so it never
                    // inherits the previously-open study's folds/regime or shows enabled undo/redo.
                    journal_state.borrow_mut().reset_undo();
                    study_notice::reset(&ui); // G1 J: the demo inherits no study's notice
                    // G1 J review: `current_study` is None HERE, by construction — not merely
                    // "stays" None: a study opened earlier would otherwise receive the demo's
                    // edit rails and a late fetch result would render over the demo.
                    *current_study.borrow_mut() = None;
                    push_form(&ui, &journal_state.borrow(), &study, format);
                    push_view_state(&ui, &StudyViewState::default());
                    studies.set_notice(SharedString::new());
                    studies.set_demo_active(true);
                    studies.set_study_open(true);
                }
                Err(_) => crate::wiring::dialog::refuse(&ui, state::MSG_DEMO_UNAVAILABLE),
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picked_pdf_name_always_ends_in_pdf() {
        // G1 final (L12): « etude-NESN.SW » has an extension (« SW ») — `.pdf` is appended.
        let p = |s: &str| {
            let (path, renamed) = with_pdf_extension(std::path::PathBuf::from(s));
            (path.to_string_lossy().to_string(), renamed)
        };
        let changed = |s: &str| (s.to_string(), true);
        let kept = |s: &str| (s.to_string(), false);
        assert_eq!(p("/x/etude-NESN.SW"), changed("/x/etude-NESN.SW.pdf"));
        assert_eq!(p("/x/etude"), changed("/x/etude.pdf"));
        assert_eq!(p("/x/notes.txt"), changed("/x/notes.txt.pdf"));
        // A bare trailing dot takes « pdf » — never « ..pdf ».
        assert_eq!(p("/x/etude."), changed("/x/etude.pdf"));
        // Already a PDF name, any case — « .pdf » alone too (never « .pdf.pdf »).
        assert_eq!(p("/x/etude.pdf"), kept("/x/etude.pdf"));
        assert_eq!(p("/x/etude.PDF"), kept("/x/etude.PDF"));
        assert_eq!(p("/x/.pdf"), kept("/x/.pdf"));
    }

    fn facts(n: u128, ticker: &str, currency: &str, date: &str) -> ChoiceFacts {
        ChoiceFacts {
            id: Uuid::from_u128(n),
            ticker: ticker.into(),
            currency: (!currency.is_empty()).then(|| currency.to_string()),
            date: date.into(),
        }
    }

    fn summary(n: u128, ticker: &str, date: &str) -> StudySummary {
        StudySummary {
            id: Uuid::from_u128(n),
            security_ticker: ticker.into(),
            created_at: steadyinvest_contract::Timestamp(format!("{date}T00:00:00Z")),
            status: "active".into(),
        }
    }

    #[test]
    fn an_unreadable_study_stays_listed_and_a_gone_one_is_left_out() {
        let summaries = [
            summary(1, "nesn.sw", "2026-01-01"),
            summary(2, "NESN.SW", "2026-02-01"),
            summary(3, "NESN.SW", "2026-03-01"),
            summary(4, "ROG.SW", "2026-04-01"),
        ];
        let listed = choice_facts(&summaries, |id| match id.as_u128() {
            1 => Ok(Some("chf".into())),
            2 => Err("unreadable".into()),
            3 => Ok(None), // deleted between the listing and the read
            _ => panic!("an unambiguous ticker is never read"),
        });
        let c = label_choices(&listed);
        assert_eq!(
            c.len(),
            3,
            "the gone study is left out, the unreadable one kept"
        );
        assert_eq!(label_of(&c, 1), "NESN.SW · CHF");
        assert_eq!(label_of(&c, 2), "NESN.SW · 2026-02-01");
        assert_eq!(label_of(&c, 4), "ROG.SW");
        // Two unreadable on one day: numbered, and the number is carried for the header.
        let c = label_choices(&[
            facts(5, "X.SW", "", "2026-01-01"),
            facts(6, "X.SW", "", "2026-01-01"),
        ]);
        assert_eq!(label_of(&c, 5), "X.SW · 2026-01-01 · 1");
        assert_eq!(
            c.iter()
                .find(|x| x.id == Uuid::from_u128(6))
                .unwrap()
                .ordinal,
            Some(2)
        );
    }

    fn label_of(choices: &[StudyChoice], n: u128) -> String {
        choices
            .iter()
            .find(|c| c.id == Uuid::from_u128(n))
            .map(|c| c.label.clone())
            .unwrap()
    }

    #[test]
    fn a_choice_names_its_currency_only_when_the_ticker_is_ambiguous() {
        let c = label_choices(&[
            facts(1, "NESN.SW", "", "2026-01-01"),
            facts(2, "AAPL.US", "USD", "2026-02-01"),
            facts(3, "AAPL.US", "CHF", "2026-03-01"),
        ]);
        assert_eq!(label_of(&c, 1), "NESN.SW");
        assert_eq!(label_of(&c, 2), "AAPL.US · USD");
        assert_eq!(label_of(&c, 3), "AAPL.US · CHF");
        // Sorted by label, one entry per STUDY (never deduplicated by ticker).
        let labels: Vec<&str> = c.iter().map(|x| x.label.as_str()).collect();
        assert_eq!(labels, ["AAPL.US · CHF", "AAPL.US · USD", "NESN.SW"]);
    }

    #[test]
    fn a_shared_ticker_and_currency_falls_back_to_the_date_then_a_number() {
        let c = label_choices(&[
            facts(1, "ROG.SW", "CHF", "2026-01-01"),
            facts(2, "ROG.SW", "CHF", "2026-05-01"),
            facts(4, "NOVN.SW", "CHF", "2026-06-01"),
            facts(3, "NOVN.SW", "CHF", "2026-06-01"),
        ]);
        assert_eq!(label_of(&c, 1), "ROG.SW · CHF · 2026-01-01");
        assert_eq!(label_of(&c, 2), "ROG.SW · CHF · 2026-05-01");
        // Same day: numbered in id order — every label still names one study.
        assert_eq!(label_of(&c, 3), "NOVN.SW · CHF · 2026-06-01 · 1");
        assert_eq!(label_of(&c, 4), "NOVN.SW · CHF · 2026-06-01 · 2");
        let ord = |n: u128| {
            c.iter()
                .find(|x| x.id == Uuid::from_u128(n))
                .unwrap()
                .ordinal
        };
        assert_eq!((ord(1), ord(3), ord(4)), (None, Some(1), Some(2)));
        let mut labels: Vec<&str> = c.iter().map(|x| x.label.as_str()).collect();
        labels.dedup();
        assert_eq!(labels.len(), 4, "unique labels");
    }
}
