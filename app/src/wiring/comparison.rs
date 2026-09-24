//! Story 7.1 — the comparison rail: the five picks → up to five columns (one `build_frame` per
//! study, the same construction the study screen uses), the table's cells, the currency-mix
//! fact; « Retour » closes; « Exporter PDF » through the native `rfd` save picker (landscape);
//! « Ouvrir l'étude » opens the study on top (the comparison stays behind it).

use std::rc::Rc;

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use crate::state::{self, JournalState};
use crate::viewmodel::comparison::{comparison_column, unavailable_column};
use crate::viewmodel::engine::build_frame;
use crate::wiring::Session;
use crate::{Comparison, ComparisonHeader, MainWindow, Studies};

/// The picks, non-empty, deduplicated, in order, at most five.
fn picks(ui: &MainWindow) -> Vec<String> {
    let c = ui.global::<Comparison>();
    let mut out: Vec<String> = Vec::new();
    for pick in [
        c.get_pick1(),
        c.get_pick2(),
        c.get_pick3(),
        c.get_pick4(),
        c.get_pick5(),
    ] {
        let t = pick.trim().to_uppercase();
        if !t.is_empty() && !out.contains(&t) {
            out.push(t);
        }
    }
    out.truncate(5);
    out
}

/// Build the columns for the picked tickers: the ticker's study (any currency — a comparison is
/// across studies, not positions), one frame each; a read failure is its own column state.
fn columns(
    state: &JournalState,
    tickers: &[String],
    format: crate::viewmodel::format::NumberFormat,
) -> Vec<steadyinvest_report::ComparisonColumn> {
    tickers
        .iter()
        .map(|ticker| match state.try_study_id_for_ticker(ticker) {
            Ok(Some(id)) => match state.try_get_study(id) {
                Ok(Some(study)) => match build_frame(&study) {
                    Ok(frame) => comparison_column(&study, &frame, format),
                    Err(_) => unavailable_column(ticker),
                },
                _ => unavailable_column(ticker),
            },
            _ => unavailable_column(ticker),
        })
        .collect()
}

/// Push the comparison of the current picks into the `Comparison` global.
pub(crate) fn push_comparison(
    ui: &MainWindow,
    state: &JournalState,
    format: crate::viewmodel::format::NumberFormat,
) {
    let tickers = picks(ui);
    let cols = columns(state, &tickers, format);
    let c = ui.global::<Comparison>();
    let today: String = state.now().0.chars().take(10).collect();
    c.set_date(today.into());
    let currencies: std::collections::BTreeSet<&str> = cols
        .iter()
        .filter(|x| !x.unavailable)
        .map(|x| x.currency.as_str())
        .collect();
    c.set_currency_mix(currencies.len() > 1);
    let n = cols.len();
    c.set_column_count(n as i32);
    // Row-major cells: row 1's n cells, then row 2's, …
    let mut cells: Vec<SharedString> = Vec::with_capacity(30 * n);
    for row in 0..30 {
        for col in &cols {
            cells.push(col.rows.get(row).cloned().unwrap_or_default().into());
        }
    }
    c.set_cells(ModelRc::new(VecModel::from(cells)));
    let headers: Vec<ComparisonHeader> = cols
        .iter()
        .map(|col| ComparisonHeader {
            ticker: col.ticker.clone().into(),
            study_id: state
                .study_id_for_ticker(&col.ticker)
                .map(|id| id.to_string())
                .unwrap_or_default()
                .into(),
            name: col.name.clone().into(),
            currency: col.currency.clone().into(),
            date: col.date.clone().into(),
            unavailable: col.unavailable,
            zone: col.zone.clone().into(),
            state: col.state.clone().into(),
            low_confidence: col.low_confidence,
        })
        .collect();
    c.set_columns(ModelRc::new(VecModel::from(headers)));
}

/// The report's value from the pushed global (one formatting path, no drift).
fn report_value(ui: &MainWindow) -> steadyinvest_report::Comparison {
    let c = ui.global::<Comparison>();
    let n = c.get_column_count().max(0) as usize;
    let cells = c.get_cells();
    let headers = c.get_columns();
    let columns = (0..headers.row_count())
        .filter_map(|i| headers.row_data(i).map(|h| (i, h)))
        .map(|(i, h)| steadyinvest_report::ComparisonColumn {
            ticker: h.ticker.to_string(),
            name: h.name.to_string(),
            currency: h.currency.to_string(),
            date: h.date.to_string(),
            unavailable: h.unavailable,
            rows: (0..30)
                .map(|row| {
                    cells
                        .row_data(row * n + i)
                        .map(|s| s.to_string())
                        .unwrap_or_default()
                })
                .collect(),
            zone: h.zone.to_string(),
            state: h.state.to_string(),
            low_confidence: h.low_confidence,
        })
        .collect();
    steadyinvest_report::Comparison {
        date: c.get_date().to_string(),
        currency_mix: c.get_currency_mix(),
        columns,
    }
}

pub(crate) fn wire_comparison(ui: &MainWindow, s: &Session) {
    let Session {
        journal_state,
        config,
        ..
    } = s;
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        ui.global::<Comparison>().on_compare(move || {
            let ui = ui_weak.unwrap();
            let format = config.borrow().number_format;
            push_comparison(&ui, &journal_state.borrow(), format);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        ui.global::<Comparison>().on_close(move || {
            let ui = ui_weak.unwrap();
            ui.global::<Studies>().set_compare_open(false);
            crate::wiring::studies::refresh_studies(&ui, &journal_state.borrow());
        });
    }
    {
        let ui_weak = ui.as_weak();
        ui.global::<Comparison>().on_open_study(move |id| {
            let ui = ui_weak.unwrap();
            ui.global::<Studies>().invoke_open_study(id);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        ui.global::<Comparison>().on_export_pdf(move || {
            let ui = ui_weak.unwrap();
            let value = report_value(&ui);
            let bytes = steadyinvest_report::render_comparison(&value);
            let mut dialog = rfd::FileDialog::new()
                .set_title("Exporter la comparaison en PDF")
                .add_filter("PDF", &["pdf"])
                .set_file_name(format!("comparaison-{}.pdf", value.date));
            if let Some(dir) = journal_state
                .borrow()
                .backups_dir()
                .and_then(|d| d.parent().map(|p| p.to_path_buf()))
                .filter(|d| d.is_dir())
            {
                dialog = dialog.set_directory(dir);
            }
            let Some(path) = dialog.save_file() else {
                return;
            };
            match std::fs::write(&path, bytes) {
                Ok(()) => ui.global::<Comparison>().set_notice(
                    format!("{} {}", state::MSG_COMPARISON_EXPORTED, path.display()).into(),
                ),
                Err(e) => {
                    crate::wiring::dialog::refuse(&ui, &format!("{} {e}", state::MSG_SAVE_FAILED))
                }
            }
        });
    }
}
