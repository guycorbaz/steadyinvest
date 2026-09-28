//! The eight tools of the MCP server (Story 8.4 AC 7 / 9 / 10) and their dispatch.
//!
//! Every call resolves the dossier afresh (A10), then goes through `McpAccess` only — typed reads,
//! draft inserts, no SQL here and no file of the dossier opened here (an extra descriptor would
//! release SQLite's POSIX locks — 8.3 G3 H1). Every successful result names the dossier it read
//! (O3); every submit tool requires it back (D10). A refused or failed call is a result with
//! `isError: true` and `{ code, message, dossier }` — the French message of Story 8.0 §3.3.
//!
//! The input schemas are built at runtime from the draftable-field registry
//! (`contract::DraftField::ALL`), so the schema can never drift from what `McpAccess` accepts.

use crate::dto;
use crate::messages::{self, Rendered, unit_fr};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use steadyinvest_contract::{
    DraftField, DraftFieldKind, DraftKind, DraftOrigin, DraftStatus, DraftTarget, Timestamp,
};
use steadyinvest_persistence::{
    DossierIdentity, DraftFilter, DraftSubmission, Error, MAX_PAGE, McpAccess, McpRead,
    McpReadRequest, Page, SubmissionRefusal, SubmitError,
};
use uuid::Uuid;

/// The eight tool names, in `tools/list` order.
pub const TOOL_NAMES: [&str; 8] = [
    "list_studies",
    "get_study",
    "get_judgment_history",
    "get_notes",
    "get_drafts_record",
    "submit_draft_study",
    "submit_draft_note",
    "submit_draft_value",
];

/// Default page size of the lists (Story 8.4 decision 5).
pub const DEFAULT_LIST_PAGE: u32 = 50;
/// Default page size of a study's history (each snapshot is a full study).
pub const DEFAULT_HISTORY_PAGE: u32 = 20;
/// The largest history page (G3: each snapshot is a full study — bounded response size).
pub const MAX_HISTORY_PAGE: u32 = 50;

/// The text returned as the server's `instructions` (English, neutral — scanned for banned verbs).
pub const INSTRUCTIONS: &str = "SteadyInvest MCP server. It reads the stock studies of the \
owner's SteadyInvest dossier (data cells with provenance, judgments, rationale, notes, history and \
the computed outputs) and records propositions (a new study, a note, a cell or judgment value). A \
proposition is a draft with a mandatory comment: it changes nothing in the dossier until the owner \
validates it, one by one, in the app. The portfolio, the watchlist, the keys and the configuration \
are never exposed. Every result names the dossier it read; every proposition carries that identity \
back (dossier_journal_id, dossier_path). Search objectives stay in this client session; the \
dossier does not store them. Provider data is fetched by the owner only, never through this server.";

/// The outcome of one tool call.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub is_error: bool,
    /// The JSON body (success: the tool's data with `dossier`; error: `code`, `message`,
    /// `dossier`).
    pub body: Value,
}

// ── Schemas ──────────────────────────────────────────────────────────────────────────────────

fn string_prop(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

fn uuid_prop(description: &str) -> Value {
    json!({ "type": "string", "format": "uuid", "description": description })
}

fn page_props(props: &mut Map<String, Value>, default_limit: u32, max_limit: u32) {
    props.insert(
        "offset".into(),
        json!({ "type": "integer", "minimum": 0, "description": "Rows skipped (default 0)." }),
    );
    props.insert(
        "limit".into(),
        json!({
            "type": "integer",
            "minimum": 1,
            "maximum": max_limit,
            "description": format!("Rows returned (default {default_limit}, at most {max_limit})."),
        }),
    );
}

fn submit_common(props: &mut Map<String, Value>, required: &mut Vec<&'static str>) {
    props.insert(
        "comment".into(),
        string_prop("Why this proposition: mandatory, shown to the owner beside it."),
    );
    props.insert(
        "origin_client".into(),
        string_prop("The AI client submitting (e.g. \"claude-code\"). Mandatory."),
    );
    props.insert(
        "origin_model".into(),
        string_prop("The model that produced the proposition. Mandatory."),
    );
    props.insert(
        "dossier_journal_id".into(),
        uuid_prop("The dossier.journal_id of the result this proposition is based on."),
    );
    props.insert(
        "dossier_path".into(),
        string_prop("The dossier.path of the result this proposition is based on."),
    );
    props.insert(
        "draft_id".into(),
        uuid_prop(
            "Optional id of this proposition, for a safe retry: the same proposition sent again \
             with the same id is recorded once.",
        ),
    );
    required.extend([
        "comment",
        "origin_client",
        "origin_model",
        "dossier_journal_id",
        "dossier_path",
    ]);
}

fn schema(props: Map<String, Value>, required: Vec<&'static str>) -> Map<String, Value> {
    let mut s = Map::new();
    s.insert("type".into(), json!("object"));
    s.insert("properties".into(), Value::Object(props));
    s.insert("required".into(), json!(required));
    s.insert("additionalProperties".into(), json!(false));
    s
}

/// The per-field lines of `submit_draft_value`'s description (AC 9): key, kind, unit, options.
pub fn field_lines() -> String {
    DraftField::ALL
        .iter()
        .map(|f| {
            let kind = match f.kind() {
                DraftFieldKind::Cell => "cell, fiscal_year required",
                DraftFieldKind::Judgment => "judgment, no fiscal_year",
            };
            let unit = match f.unit() {
                steadyinvest_contract::DraftUnit::Amount => {
                    "absolute amount in the study currency (not in millions)".to_string()
                }
                steadyinvest_contract::DraftUnit::PerShare => {
                    "per-share amount in the study currency".to_string()
                }
                steadyinvest_contract::DraftUnit::Price => {
                    "share price in the study currency".to_string()
                }
                steadyinvest_contract::DraftUnit::Percent => {
                    "percent written as percent (12 = 12 %)".to_string()
                }
                steadyinvest_contract::DraftUnit::Ratio => "plain multiple".to_string(),
                steadyinvest_contract::DraftUnit::Option => {
                    format!("one of: {}", messages::options_list(*f))
                }
            };
            format!("- {} ({kind}): {unit}", f.key())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The eight tools, their descriptions and input schemas (`tools/list`).
pub fn tool_specs() -> Vec<(&'static str, String, Map<String, Value>)> {
    let mut out = Vec::new();

    let mut p = Map::new();
    page_props(&mut p, DEFAULT_LIST_PAGE, MAX_PAGE);
    out.push((
        "list_studies",
        "List the studies of the dossier (id, ticker, creation date, status), one page at a time."
            .to_string(),
        schema(p, vec![]),
    ));

    let mut p = Map::new();
    p.insert(
        "study_id".into(),
        uuid_prop("The study id (from list_studies)."),
    );
    out.push((
        "get_study",
        "Read one study: data cells with provenance, judgments, rationale, notes, status, and its \
         computed outputs (zones as neutral codes low/middle/high, upside/downside ratio, 5-year \
         potential, verdict facts and state). Numbers are exact decimals as strings."
            .to_string(),
        schema(p, vec!["study_id"]),
    ));

    let mut p = Map::new();
    p.insert("study_id".into(), uuid_prop("The study id."));
    page_props(&mut p, DEFAULT_HISTORY_PAGE, MAX_HISTORY_PAGE);
    out.push((
        "get_judgment_history",
        "Read a study's history: each entry is the full study at a past save, newest first."
            .to_string(),
        schema(p, vec!["study_id"]),
    ));

    let mut p = Map::new();
    p.insert("study_id".into(), uuid_prop("The study id."));
    out.push((
        "get_notes",
        "Read the notes of a study.".to_string(),
        schema(p, vec!["study_id"]),
    ));

    let mut p = Map::new();
    p.insert(
        "study_id".into(),
        uuid_prop("Optional: only the propositions about this study."),
    );
    p.insert(
        "status".into(),
        json!({
            "type": "string",
            "enum": DraftStatus::ALL.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            "description": "Optional: only the propositions in this status.",
        }),
    );
    page_props(&mut p, DEFAULT_LIST_PAGE, MAX_PAGE);
    out.push((
        "get_drafts_record",
        "Read the record of every proposition (pending, validated, validated then undone, \
         rejected) with its comment, origin, dates and outcome."
            .to_string(),
        schema(p, vec![]),
    ));

    let mut p = Map::new();
    let mut req = vec!["security_ticker", "native_currency"];
    p.insert(
        "security_ticker".into(),
        string_prop("The security symbol: 1 to 20 of A-Z, 0-9, '.', '-' (e.g. NVDA, ROG.SW)."),
    );
    p.insert(
        "native_currency".into(),
        string_prop("The study currency: three uppercase letters (e.g. USD, CHF)."),
    );
    p.insert(
        "company_name".into(),
        string_prop("Optional company name (at most 200 characters)."),
    );
    submit_common(&mut p, &mut req);
    out.push((
        "submit_draft_study",
        "Propose a new study for a security. It creates nothing until the owner validates it; a \
         security already studied, or already proposed, in the same currency is refused."
            .to_string(),
        schema(p, req),
    ));

    let mut p = Map::new();
    let mut req = vec!["study_id", "note_text"];
    p.insert("study_id".into(), uuid_prop("The study the note is about."));
    p.insert(
        "note_text".into(),
        string_prop("The proposed note text (at most 10000 characters)."),
    );
    submit_common(&mut p, &mut req);
    out.push((
        "submit_draft_note",
        "Propose a note on a study. It is added only when the owner validates it.".to_string(),
        schema(p, req),
    ));

    let mut p = Map::new();
    let mut req = vec!["study_id", "field", "proposed_value"];
    p.insert(
        "study_id".into(),
        uuid_prop("The study the value is about."),
    );
    p.insert(
        "field".into(),
        json!({
            "type": "string",
            "enum": DraftField::ALL.iter().map(|f| f.key()).collect::<Vec<_>>(),
            "description": "The field proposed (see the list in the tool description).",
        }),
    );
    p.insert(
        "fiscal_year".into(),
        json!({
            "type": "integer",
            "description": "The fiscal year of a cell field (a year of the study); absent for a \
                            judgment field.",
        }),
    );
    p.insert(
        "proposed_value".into(),
        string_prop(
            "The proposed value as plain decimal text in the field's unit (e.g. \"12.5\"; no \
             exponent, no thousands separator), or an option name for forecast_low_option.",
        ),
    );
    submit_common(&mut p, &mut req);
    out.push((
        "submit_draft_value",
        format!(
            "Propose a value for one data cell or judgment of a study. It changes no value, zone \
             or verdict until the owner validates it; one pending proposition per target. \
             Current price and TTM EPS are market facts and cannot be proposed. Fields:\n{}",
            field_lines()
        ),
        schema(p, req),
    ));
    out
}

// ── Arguments ────────────────────────────────────────────────────────────────────────────────

struct Args {
    map: Map<String, Value>,
}

type ArgResult<T> = Result<T, Rendered>;

impl Args {
    fn new(map: Map<String, Value>, allowed: &[&str]) -> ArgResult<Self> {
        if let Some(extra) = map.keys().find(|k| !allowed.contains(&k.as_str())) {
            return Err(messages::invalid_call(&format!(
                "unknown argument {}",
                messages::echo(extra)
            )));
        }
        Ok(Args { map })
    }

    fn opt_str(&self, name: &str) -> ArgResult<Option<String>> {
        match self.map.get(name) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.clone())),
            Some(_) => Err(messages::invalid_call(&format!("{name} must be a string"))),
        }
    }

    fn str(&self, name: &str) -> ArgResult<String> {
        self.opt_str(name)?
            .ok_or_else(|| messages::invalid_call(&format!("{name} is required")))
    }

    fn opt_uuid(&self, name: &str) -> ArgResult<Option<Uuid>> {
        match self.opt_str(name)? {
            None => Ok(None),
            Some(s) => Uuid::parse_str(s.trim())
                .map(Some)
                .map_err(|_| messages::invalid_call(&format!("{name} is not a uuid"))),
        }
    }

    fn uuid(&self, name: &str) -> ArgResult<Uuid> {
        self.opt_uuid(name)?
            .ok_or_else(|| messages::invalid_call(&format!("{name} is required")))
    }

    fn opt_int(&self, name: &str) -> ArgResult<Option<i64>> {
        match self.map.get(name) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Number(n)) => n
                .as_i64()
                .map(Some)
                .ok_or_else(|| messages::invalid_call(&format!("{name} must be an integer"))),
            Some(_) => Err(messages::invalid_call(&format!(
                "{name} must be an integer"
            ))),
        }
    }

    fn page(&self, default_limit: u32, max_limit: u32) -> ArgResult<Page> {
        let offset = match self.opt_int("offset")? {
            None => 0,
            Some(o) if o >= 0 => o as u64,
            Some(_) => return Err(messages::invalid_call("offset must be 0 or more")),
        };
        let limit = match self.opt_int("limit")? {
            None => default_limit,
            Some(l) if (1..=i64::from(max_limit)).contains(&l) => l as u32,
            Some(_) => {
                return Err(messages::invalid_call(&format!(
                    "limit must be between 1 and {max_limit}"
                )));
            }
        };
        Ok(Page { offset, limit })
    }
}

const PAGE_ARGS: [&str; 2] = ["offset", "limit"];
const SUBMIT_ARGS: [&str; 6] = [
    "comment",
    "origin_client",
    "origin_model",
    "dossier_journal_id",
    "dossier_path",
    "draft_id",
];

fn allowed(tool_args: &[&'static str], extra: &[&'static str]) -> Vec<&'static str> {
    tool_args.iter().chain(extra.iter()).copied().collect()
}

// ── Dispatch ─────────────────────────────────────────────────────────────────────────────────

/// What a call needs from its environment: the explicit `--dossier`, the clock, the id source (the
/// server is the caller of `McpAccess` — ADD15: identity and time come from it), and whether the
/// client cancelled the call (checked before the insert).
pub struct CallEnv<'a> {
    pub explicit: Option<&'a Path>,
    pub now: &'a dyn Fn() -> Timestamp,
    pub new_id: &'a dyn Fn() -> Uuid,
    pub cancelled: &'a dyn Fn() -> bool,
}

fn error_body(r: &Rendered, dossier: Option<&DossierIdentity>, resolved: Option<&Path>) -> Value {
    json!({
        "code": r.code,
        "message": r.message,
        "dossier": dossier.map(dto::dossier),
        "resolved_path": resolved.map(|p| p.display().to_string()),
    })
}

/// A refused or failed call. The LOG gets the tool and the code only — never the message, which
/// can echo AI-written text (G3: a newline in it would forge a log line).
fn fail(
    tool: &'static str,
    r: Rendered,
    dossier: Option<&DossierIdentity>,
    resolved: Option<&Path>,
) -> Outcome {
    tracing::warn!(tool, code = r.code, "tool call refused or failed");
    Outcome {
        is_error: true,
        body: error_body(&r, dossier, resolved),
    }
}

/// The log line of a dossier-layer failure: its kind and MCP code — plus, for a denied write, each
/// denied action and object (engine-produced SQL names, never AI text).
fn log_failure(tool: &'static str, e: &Error) {
    if let Error::McpDenied { denials } = e {
        for d in denials {
            tracing::warn!(tool, action = %d.action, object = %d.object, "write denied");
        }
    }
    tracing::warn!(
        tool,
        kind = ?e.kind(),
        code = e.mcp_code().unwrap_or("dossier_error"),
        "dossier call failed"
    );
}

fn ok(dossier: &DossierIdentity, mut data: Value) -> Outcome {
    if let Value::Object(map) = &mut data {
        map.insert("dossier".into(), dto::dossier(dossier));
    }
    Outcome {
        is_error: false,
        body: data,
    }
}

/// The tool's own `'static` name (so the log never carries a client-sent string), or `None`.
fn known_tool(name: &str) -> Option<&'static str> {
    TOOL_NAMES.iter().copied().find(|t| *t == name)
}

/// Run one tool call (blocking — the server runs it on a blocking thread).
pub fn call(name: &str, arguments: Map<String, Value>, env: &CallEnv<'_>) -> Outcome {
    let Some(tool) = known_tool(name) else {
        tracing::warn!("call of an unknown tool");
        return fail(
            "unknown",
            messages::invalid_call("unknown tool"),
            None,
            None,
        );
    };
    tracing::info!(tool, "tool call");
    let path = match steadyinvest_paths::resolve_dossier(env.explicit) {
        Ok(p) => p,
        Err(e) => {
            // The config path and parse detail go to the log only, never to the client (G3).
            tracing::warn!(tool, error = %e, "no dossier resolved");
            return fail(tool, messages::resolve_error(&e), None, None);
        }
    };
    if path.to_str().is_none() {
        return fail(tool, messages::path_unusable(), None, None);
    }
    let access = McpAccess::at(&path);
    let resolved = access.path().to_path_buf();
    match call_on(tool, arguments, env, &access) {
        Ok(outcome) => outcome,
        Err(Failure::Rendered(r, dossier)) => fail(tool, r, dossier.as_ref(), Some(&resolved)),
        Err(Failure::Dossier(e, dossier)) => {
            log_failure(tool, &e);
            fail(
                tool,
                messages::failure(&e, &resolved),
                dossier.as_ref(),
                Some(&resolved),
            )
        }
        Err(Failure::Submit(e, dossier)) => {
            match &e {
                SubmitError::Failed(f) => log_failure(tool, f),
                SubmitError::Refused(r) => {
                    tracing::warn!(tool, code = r.code(), "proposition refused")
                }
                SubmitError::Cancelled => tracing::info!(tool, "call cancelled"),
            }
            fail(
                tool,
                messages::submit_error(&e, &resolved),
                dossier.as_ref(),
                Some(&resolved),
            )
        }
    }
}

enum Failure {
    Rendered(Rendered, Option<DossierIdentity>),
    Dossier(Error, Option<DossierIdentity>),
    Submit(SubmitError, Option<DossierIdentity>),
}

/// One identified read (the identity and the data in the same read transaction — G3).
fn read(
    access: &McpAccess,
    request: McpReadRequest,
) -> Result<(DossierIdentity, McpRead), Failure> {
    access
        .read_identified(request)
        .map_err(|e| Failure::Dossier(e, None))
}

fn study_missing(id: DossierIdentity, study_id: Uuid) -> Failure {
    Failure::Rendered(
        messages::refusal(&SubmissionRefusal::StudyNotFound { study_id }),
        Some(id),
    )
}

fn unexpected(id: DossierIdentity) -> Failure {
    Failure::Rendered(messages::invalid_call("unexpected read result"), Some(id))
}

fn call_on(
    tool: &'static str,
    arguments: Map<String, Value>,
    env: &CallEnv<'_>,
    access: &McpAccess,
) -> Result<Outcome, Failure> {
    let rendered = |r: Rendered| Failure::Rendered(r, None);
    match tool {
        "list_studies" => {
            let args = Args::new(arguments, &PAGE_ARGS).map_err(rendered)?;
            let page = args.page(DEFAULT_LIST_PAGE, MAX_PAGE).map_err(rendered)?;
            match read(access, McpReadRequest::Studies(page))? {
                (id, McpRead::Studies(list)) => Ok(ok(&id, dto::study_list(&list))),
                (id, _) => Err(unexpected(id)),
            }
        }
        "get_study" | "get_notes" => {
            let args = Args::new(arguments, &["study_id"]).map_err(rendered)?;
            let study_id = args.uuid("study_id").map_err(rendered)?;
            match read(access, McpReadRequest::Study(study_id))? {
                (id, McpRead::Study(read)) => Ok(ok(
                    &id,
                    if tool == "get_study" {
                        dto::study_read(&read)
                    } else {
                        dto::notes(&read)
                    },
                )),
                (id, McpRead::StudyMissing(s)) => Err(study_missing(id, s)),
                (id, _) => Err(unexpected(id)),
            }
        }
        "get_judgment_history" => {
            let args =
                Args::new(arguments, &allowed(&["study_id"], &PAGE_ARGS)).map_err(rendered)?;
            let study_id = args.uuid("study_id").map_err(rendered)?;
            let page = args
                .page(DEFAULT_HISTORY_PAGE, MAX_HISTORY_PAGE)
                .map_err(rendered)?;
            match read(access, McpReadRequest::History(study_id, page))? {
                (id, McpRead::History(hist)) => Ok(ok(&id, dto::history(study_id, &hist))),
                (id, McpRead::StudyMissing(s)) => Err(study_missing(id, s)),
                (id, _) => Err(unexpected(id)),
            }
        }
        "get_drafts_record" => {
            let args = Args::new(arguments, &allowed(&["study_id", "status"], &PAGE_ARGS))
                .map_err(rendered)?;
            let study_id = args.opt_uuid("study_id").map_err(rendered)?;
            let status = match args.opt_str("status").map_err(rendered)? {
                None => None,
                Some(s) => Some(s.parse::<DraftStatus>().map_err(|_| {
                    rendered(messages::invalid_call("status is not a draft status"))
                })?),
            };
            let page = args.page(DEFAULT_LIST_PAGE, MAX_PAGE).map_err(rendered)?;
            match read(
                access,
                McpReadRequest::Drafts(DraftFilter { study_id, status }, page),
            )? {
                (id, McpRead::Drafts(list)) => Ok(ok(&id, dto::draft_list(&list))),
                (id, McpRead::StudyMissing(s)) => Err(study_missing(id, s)),
                (id, _) => Err(unexpected(id)),
            }
        }
        _ => submit(tool, arguments, env, access),
    }
}

fn submit(
    tool: &'static str,
    arguments: Map<String, Value>,
    env: &CallEnv<'_>,
    access: &McpAccess,
) -> Result<Outcome, Failure> {
    let rendered = |r: Rendered| Failure::Rendered(r, None);
    let own: &[&'static str] = match tool {
        "submit_draft_study" => &["security_ticker", "native_currency", "company_name"],
        "submit_draft_note" => &["study_id", "note_text"],
        _ => &["study_id", "field", "fiscal_year", "proposed_value"],
    };
    let args = Args::new(arguments, &allowed(own, &SUBMIT_ARGS)).map_err(rendered)?;
    let comment = args.str("comment").map_err(rendered)?;
    let origin = DraftOrigin {
        client: args.str("origin_client").map_err(rendered)?,
        model: args.str("origin_model").map_err(rendered)?,
    };
    let dossier = DossierIdentity {
        journal_id: args.uuid("dossier_journal_id").map_err(rendered)?,
        path: PathBuf::from(args.str("dossier_path").map_err(rendered)?),
    };
    let draft_id = args.opt_uuid("draft_id").map_err(rendered)?;

    let mut sub = DraftSubmission {
        id: draft_id.unwrap_or_else(|| (env.new_id)()),
        // A retry of the same draft_id keeps the STORED creation time (McpAccess, in the
        // insert's transaction) — this one is used only for a new draft.
        created_at: (env.now)(),
        kind: DraftKind::Study,
        study_id: None,
        security_ticker: None,
        native_currency: None,
        company_name: None,
        target: None,
        proposed_value: None,
        note_text: None,
        comment,
        origin,
        dossier,
        method_version: steadyinvest_core::METHOD_VERSION.to_string(),
    };
    match tool {
        "submit_draft_study" => {
            sub.kind = DraftKind::Study;
            sub.security_ticker = Some(args.str("security_ticker").map_err(rendered)?);
            sub.native_currency = Some(args.str("native_currency").map_err(rendered)?);
            sub.company_name = args.opt_str("company_name").map_err(rendered)?;
        }
        "submit_draft_note" => {
            sub.kind = DraftKind::Note;
            sub.study_id = Some(args.uuid("study_id").map_err(rendered)?);
            sub.note_text = Some(args.str("note_text").map_err(rendered)?);
        }
        _ => {
            sub.study_id = Some(args.uuid("study_id").map_err(rendered)?);
            let field = args.str("field").map_err(rendered)?;
            let year = args.opt_int("fiscal_year").map_err(rendered)?;
            let year = match year {
                None => None,
                Some(y) => Some(i32::try_from(y).map_err(|_| {
                    rendered(messages::invalid_call("fiscal_year is out of range"))
                })?),
            };
            let (kind, target) = match (DraftField::from_key(&field).map(|f| f.kind()), year) {
                (Some(DraftFieldKind::Cell), None) => {
                    return Err(rendered(messages::invalid_call(&format!(
                        "the cell field {} needs a fiscal_year",
                        messages::echo(&field)
                    ))));
                }
                (Some(DraftFieldKind::Judgment), Some(_)) => {
                    return Err(rendered(messages::invalid_call(&format!(
                        "the judgment field {} takes no fiscal_year",
                        messages::echo(&field)
                    ))));
                }
                (Some(DraftFieldKind::Cell), Some(fiscal_year)) | (None, Some(fiscal_year)) => (
                    DraftKind::Cell,
                    DraftTarget::Cell {
                        fiscal_year,
                        field: field.clone(),
                    },
                ),
                (Some(DraftFieldKind::Judgment), None) | (None, None) => (
                    DraftKind::Judgment,
                    DraftTarget::Judgment {
                        field: field.clone(),
                    },
                ),
            };
            sub.kind = kind;
            sub.target = Some(target);
            sub.proposed_value = Some(args.str("proposed_value").map_err(rendered)?);
        }
    }

    let id = access.identity().map_err(|e| Failure::Dossier(e, None))?;
    let recorded = access
        .submit_draft_recorded(&sub, env.cancelled)
        .map_err(|e| Failure::Submit(e, Some(id.clone())))?;
    tracing::info!(tool, draft = %recorded.id, status = recorded.status.as_str(), "proposition recorded");
    Ok(ok(
        &id,
        json!({ "draft_id": recorded.id.to_string(), "status": recorded.status.as_str() }),
    ))
}

/// The French unit of a field (re-exported for the seed example and tests).
pub fn unit_of(field: DraftField) -> String {
    unit_fr(field)
}

#[cfg(test)]
mod tests {
    use super::*;
    use steadyinvest_core::method::{BANNED_VERBS_EN, BANNED_VERBS_FR, contains_word};

    #[test]
    fn the_value_schema_enumerates_exactly_the_registry() {
        let specs = tool_specs();
        let (_, description, schema) = specs
            .iter()
            .find(|(n, _, _)| *n == "submit_draft_value")
            .expect("the tool");
        let keys: Vec<String> = schema["properties"]["field"]["enum"]
            .as_array()
            .expect("an enum")
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let registry: Vec<String> = DraftField::ALL
            .iter()
            .map(|f| f.key().to_string())
            .collect();
        assert_eq!(keys, registry);
        assert!(!keys.iter().any(|k| k == "current_price" || k == "ttm_eps"));
        for f in DraftField::ALL {
            assert!(description.contains(f.key()), "{} not described", f.key());
        }
        assert!(description.contains("avg_low_pe_times_eps"));
        assert!(description.contains("not in millions"));
    }

    #[test]
    fn exactly_eight_tools_and_every_submit_requires_comment_origin_and_dossier() {
        let specs = tool_specs();
        let names: Vec<&str> = specs.iter().map(|(n, _, _)| *n).collect();
        assert_eq!(names, TOOL_NAMES);
        for (name, _, schema) in specs.iter().filter(|(n, _, _)| n.starts_with("submit_")) {
            let required: Vec<&str> = schema["required"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect();
            for r in [
                "comment",
                "origin_client",
                "origin_model",
                "dossier_journal_id",
                "dossier_path",
            ] {
                assert!(required.contains(&r), "{name} does not require {r}");
            }
        }
        let study = &specs[5].2;
        assert!(
            study["required"]
                .as_array()
                .unwrap()
                .contains(&json!("native_currency")),
            "D2: the currency is required"
        );
    }

    #[test]
    fn no_tool_text_uses_a_banned_verb() {
        let mut texts = vec![INSTRUCTIONS.to_string()];
        for (name, description, schema) in tool_specs() {
            texts.push(name.to_string());
            texts.push(description);
            texts.push(Value::Object(schema).to_string());
        }
        for text in texts {
            for verb in BANNED_VERBS_EN.iter().chain(BANNED_VERBS_FR.iter()) {
                assert!(!contains_word(&text, verb), "« {verb} » in {text}");
            }
        }
    }
}
