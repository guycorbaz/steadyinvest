//! Story 7.1 — the comparison rail: the five picks → up to five columns (one `build_frame` per
//! study, the same construction the study screen uses), the table's cells, the currency-mix
//! fact; « Retour » closes; « Exporter PDF » through the native `rfd` save picker (landscape);
//! « Ouvrir l'étude » opens the study on top (the comparison stays behind it).
//!
//! G1 decision 3 (#237): the pickers list STUDIES, not tickers. A pick is keyed by its study id
//! from the drop-down to the column and to « Ouvrir l'étude » (the discriminator rule — no
//! ticker lookup anywhere); the label is display only.

use std::rc::Rc;

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use uuid::Uuid;

use crate::state::{self, JournalState};
use crate::viewmodel::comparison::{comparison_column, missing_column, unavailable_column};
use crate::viewmodel::engine::build_frame;
use crate::wiring::Session;
use crate::wiring::studies::study_choices;
use crate::{Comparison, ComparisonHeader, MainWindow, Studies, StudyChoice};

const SLOTS: usize = 5;

/// The five pick labels, as shown.
fn pick_labels(c: &Comparison<'_>) -> [String; SLOTS] {
    [
        c.get_pick1(),
        c.get_pick2(),
        c.get_pick3(),
        c.get_pick4(),
        c.get_pick5(),
    ]
    .map(|s| s.to_string())
}

fn set_pick_label(c: &Comparison<'_>, slot: usize, label: &str) {
    let label: SharedString = label.into();
    match slot {
        0 => c.set_pick1(label),
        1 => c.set_pick2(label),
        2 => c.set_pick3(label),
        3 => c.set_pick4(label),
        _ => c.set_pick5(label),
    }
}

/// The five picked study ids (`""` = an empty slot).
fn pick_ids(c: &Comparison<'_>) -> [String; SLOTS] {
    let model = c.get_pick_ids();
    std::array::from_fn(|i| model.row_data(i).map(|s| s.to_string()).unwrap_or_default())
}

fn set_pick_ids(c: &Comparison<'_>, ids: &[String; SLOTS]) {
    let ids: Vec<SharedString> = ids.iter().map(SharedString::from).collect();
    c.set_pick_ids(ModelRc::new(VecModel::from(ids)));
}

/// The pushed choices as `(id, label)`.
fn choices(c: &Comparison<'_>) -> Vec<(String, String)> {
    c.get_choices()
        .iter()
        .map(|x| (x.id.to_string(), x.label.to_string()))
        .collect()
}

/// PURE: one drop-down's list — every study but those picked in the OTHER slots (spec §2).
fn slot_options(choices: &[(String, String)], ids: &[String; SLOTS], slot: usize) -> Vec<String> {
    choices
        .iter()
        .filter(|(id, _)| {
            !ids.iter()
                .enumerate()
                .any(|(i, picked)| i != slot && picked == id)
        })
        .map(|(_, label)| label.clone())
        .collect()
}

/// PURE: the distinct picked study ids, in slot order — the columns' identities.
fn distinct_ids(ids: &[String; SLOTS]) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = Vec::new();
    for (slot, id) in ids.iter().enumerate() {
        if !id.is_empty() && !out.iter().any(|(_, seen)| seen == id) {
            out.push((slot, id.clone()));
        }
    }
    out
}

/// Re-derive the five lists and the distinct-pick count from the choices and the pick ids.
fn sync_picker(c: &Comparison<'_>) {
    let choices = choices(c);
    let ids = pick_ids(c);
    let lists: [Vec<String>; SLOTS] =
        std::array::from_fn(|slot| slot_options(&choices, &ids, slot));
    let model = |v: &Vec<String>| {
        let v: Vec<SharedString> = v.iter().map(SharedString::from).collect();
        ModelRc::new(VecModel::from(v))
    };
    c.set_options1(model(&lists[0]));
    c.set_options2(model(&lists[1]));
    c.set_options3(model(&lists[2]));
    c.set_options4(model(&lists[3]));
    c.set_options5(model(&lists[4]));
    c.set_distinct_picks(distinct_ids(&ids).len() as i32);
}

/// Push the dossier's studies into the picker (called from `refresh_studies`). A read failure
/// is stated (`choices-unavailable`, #95), never an empty list. A pick whose study is still
/// listed takes its current label (it may gain « · CUR » when a second study of its ticker
/// appears); a pick whose study is gone keeps its id and label — « Comparer » then states it
/// « introuvable » rather than dropping it in silence.
pub(crate) fn push_choices(ui: &MainWindow, state: &JournalState) {
    let c = ui.global::<Comparison>();
    let listed = study_choices(state);
    c.set_choices_unavailable(listed.is_err());
    let listed = listed.unwrap_or_default();
    let rows: Vec<StudyChoice> = listed
        .iter()
        .map(|x| StudyChoice {
            id: x.id.to_string().into(),
            label: x.label.clone().into(),
        })
        .collect();
    c.set_choices(ModelRc::new(VecModel::from(rows)));
    let ids = pick_ids(&c);
    for (slot, id) in ids.iter().enumerate() {
        if let Some(choice) = listed.iter().find(|x| x.id.to_string() == *id) {
            set_pick_label(&c, slot, &choice.label);
        }
    }
    set_pick_ids(&c, &ids);
    sync_picker(&c);
}

/// Build the columns for the picked studies, by id: one frame each. A study that no longer
/// exists is « introuvable »; a read failure or a frame that does not compute « indisponible ».
fn columns(
    state: &JournalState,
    picks: &[(String, String)],
    format: crate::viewmodel::format::NumberFormat,
) -> Vec<(Option<Uuid>, steadyinvest_report::ComparisonColumn)> {
    picks
        .iter()
        .map(|(id, label)| {
            let Ok(uuid) = Uuid::parse_str(id) else {
                return (None, unavailable_column(label));
            };
            match state.try_get_study(uuid) {
                Ok(Some(study)) => match build_frame(&study) {
                    Ok(frame) => (Some(uuid), comparison_column(&study, &frame, format)),
                    Err(_) => (None, unavailable_column(label)),
                },
                Ok(None) => (None, missing_column(label)),
                Err(_) => (None, unavailable_column(label)),
            }
        })
        .collect()
}

/// Push the comparison of the current picks into the `Comparison` global.
pub(crate) fn push_comparison(
    ui: &MainWindow,
    state: &JournalState,
    format: crate::viewmodel::format::NumberFormat,
) {
    let c = ui.global::<Comparison>();
    let labels = pick_labels(&c);
    let picks: Vec<(String, String)> = distinct_ids(&pick_ids(&c))
        .into_iter()
        .map(|(slot, id)| (id, labels[slot].clone()))
        .collect();
    let keyed = columns(state, &picks, format);
    let cols: Vec<&steadyinvest_report::ComparisonColumn> = keyed.iter().map(|(_, x)| x).collect();
    // The notice slot holds only the export outcome of THIS table: a new table clears it.
    c.set_notice(SharedString::new());
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
    // « Ouvrir l'étude » opens the id the column was BUILT from — no second lookup; a column with
    // no readable study behind it offers no button.
    let headers: Vec<ComparisonHeader> = keyed
        .iter()
        .map(|(id, col)| ComparisonHeader {
            ticker: col.ticker.clone().into(),
            study_id: id.map(|id| id.to_string()).unwrap_or_default().into(),
            name: col.name.clone().into(),
            currency: col.currency.clone().into(),
            date: col.date.clone().into(),
            unavailable: col.unavailable,
            missing: col.missing,
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
            missing: h.missing,
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
    // A drop-down pick: its label (unique in the pushed list) → its study id; the lists re-derive.
    {
        let ui_weak = ui.as_weak();
        ui.global::<Comparison>().on_picked(move |slot, label| {
            let ui = ui_weak.unwrap();
            let c = ui.global::<Comparison>();
            let Ok(slot) = usize::try_from(slot) else {
                return;
            };
            if slot >= SLOTS {
                return;
            }
            let mut ids = pick_ids(&c);
            ids[slot] = choices(&c)
                .into_iter()
                .find(|(_, l)| *l == label.as_str())
                .map(|(id, _)| id)
                .unwrap_or_default();
            set_pick_ids(&c, &ids);
            sync_picker(&c);
        });
    }
    {
        let ui_weak = ui.as_weak();
        ui.global::<Comparison>().on_clear_picks(move || {
            let ui = ui_weak.unwrap();
            let c = ui.global::<Comparison>();
            for slot in 0..SLOTS {
                set_pick_label(&c, slot, "");
            }
            set_pick_ids(&c, &std::array::from_fn(|_| String::new()));
            sync_picker(&c);
        });
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(v: [&str; SLOTS]) -> [String; SLOTS] {
        v.map(String::from)
    }

    #[test]
    fn a_picked_study_leaves_the_other_lists_but_stays_in_its_own() {
        let choices: Vec<(String, String)> = [("a", "AAPL.US · USD"), ("b", "AAPL.US · CHF")]
            .iter()
            .chain([("c", "NESN.SW")].iter())
            .map(|(i, l)| (i.to_string(), l.to_string()))
            .collect();
        let picked = ids(["a", "", "c", "", ""]);
        assert_eq!(
            slot_options(&choices, &picked, 0),
            ["AAPL.US · USD", "AAPL.US · CHF"]
        );
        assert_eq!(slot_options(&choices, &picked, 1), ["AAPL.US · CHF"]);
        assert_eq!(
            slot_options(&choices, &picked, 2),
            ["AAPL.US · CHF", "NESN.SW"]
        );
    }

    #[test]
    fn compare_counts_distinct_studies_not_filled_slots() {
        // Two slots on one study: ONE column, and « Comparer » stays disabled (< 2).
        assert_eq!(distinct_ids(&ids(["a", "a", "", "", ""])).len(), 1);
        // Two studies of one ticker are two columns (keyed by id, not by ticker).
        assert_eq!(
            distinct_ids(&ids(["", "b", "a", "b", ""])),
            vec![(1, "b".to_string()), (2, "a".to_string())]
        );
        assert!(distinct_ids(&ids(["", "", "", "", ""])).is_empty());
    }
}
