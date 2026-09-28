//! `just mcp-seed <copy>` — seed a THROW-AWAY dossier copy with propositions of every kind, by
//! driving the real `steadyinvest-mcp` binary over stdio (Story 8.4 AC 18). For the checks of the
//! Epic 8 UI stories without any AI session.
//!
//! Dev safety (arch §Phase 4 A12): the target must exist and must NOT be a dossier the real app
//! uses (its `last_opened_path`, `journal_path` or default dossier, compared canonically) — the
//! guard refuses it. The server runs with `--dossier <copy>` and temp `HOME` / `XDG_*` dirs.
//!
//! Usage: `cargo build -p steadyinvest-mcp && cargo run -p steadyinvest-mcp --example seed -- <copy>`
//! (the binary is taken beside this example's target dir, or from `STEADYINVEST_MCP_BIN`).

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};
use steadyinvest_mcp::seed_guard::check_seed_target;

struct Client {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    next: u64,
}

impl Client {
    fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next;
        self.next += 1;
        let line = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        writeln!(self.stdin, "{line}").map_err(|e| e.to_string())?;
        self.stdin.flush().map_err(|e| e.to_string())?;
        loop {
            let mut buf = String::new();
            if self.stdout.read_line(&mut buf).map_err(|e| e.to_string())? == 0 {
                return Err(format!("the server closed stdout during {method}"));
            }
            let msg: Value = serde_json::from_str(&buf).map_err(|e| e.to_string())?;
            if msg["id"] == json!(id) {
                return Ok(msg);
            }
        }
    }

    fn call(&mut self, tool: &str, arguments: Value) -> Result<(bool, Value), String> {
        let r = self.request(
            "tools/call",
            json!({ "name": tool, "arguments": arguments }),
        )?;
        let text = r["result"]["content"][0]["text"]
            .as_str()
            .ok_or_else(|| format!("{tool}: no result: {r}"))?;
        let body: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        Ok((r["result"]["isError"] == json!(true), body))
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn binary() -> Result<PathBuf, String> {
    if let Some(p) = std::env::var_os("STEADYINVEST_MCP_BIN") {
        return Ok(PathBuf::from(p));
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe
        .parent()
        .and_then(|examples| examples.parent())
        .ok_or("no target dir")?;
    let bin = dir.join(format!("steadyinvest-mcp{}", std::env::consts::EXE_SUFFIX));
    if bin.exists() {
        Ok(bin)
    } else {
        Err(format!(
            "{} not built — run `cargo build -p steadyinvest-mcp` first",
            bin.display()
        ))
    }
}

fn run(copy: &str) -> Result<(), String> {
    let target = check_seed_target(std::path::Path::new(copy)).map_err(|e| e.to_string())?;
    let home = tempfile::tempdir().map_err(|e| e.to_string())?;
    let (config, data) = (home.path().join("config"), home.path().join("data"));
    std::fs::create_dir_all(&config).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&data).map_err(|e| e.to_string())?;
    let mut child = Command::new(binary()?)
        .arg("--dossier")
        .arg(&target)
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", &config)
        .env("XDG_DATA_HOME", &data)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("steadyinvest-mcp not started: {e}"))?;
    let stdin = child.stdin.take().ok_or("no stdin")?;
    let stdout = BufReader::new(child.stdout.take().ok_or("no stdout")?);
    let mut c = Client {
        child,
        stdin,
        stdout,
        next: 1,
    };
    c.request(
        "initialize",
        json!({ "protocolVersion": "2025-06-18", "capabilities": {},
                "clientInfo": { "name": "mcp-seed", "version": "0" } }),
    )?;
    writeln!(
        c.stdin,
        "{}",
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })
    )
    .map_err(|e| e.to_string())?;

    let (err, list) = c.call("list_studies", json!({}))?;
    if err {
        return Err(format!("list_studies refused: {list}"));
    }
    let dossier = list["dossier"].clone();
    let base = |extra: Value| {
        let mut m = json!({
            "comment": "Proposition de test (mcp-seed).",
            "origin_client": "mcp-seed",
            "origin_model": "none",
            "dossier_journal_id": dossier["journal_id"],
            "dossier_path": dossier["path"],
        });
        if let (Value::Object(m), Value::Object(e)) = (&mut m, extra) {
            m.extend(e);
        }
        m
    };
    let mut submissions = vec![(
        "submit_draft_study",
        base(
            json!({ "security_ticker": "SEED.TEST", "native_currency": "USD",
                     "company_name": "Seed Test SA" }),
        ),
    )];
    if let Some(first) = list["studies"].as_array().and_then(|s| s.first()) {
        let id = first["id"].as_str().unwrap_or_default().to_string();
        let (err, study) = c.call("get_study", json!({ "study_id": id }))?;
        if err {
            return Err(format!("get_study refused: {study}"));
        }
        let year = study["study"]["years"]
            .as_array()
            .and_then(|y| y.last())
            .and_then(|y| y["year"].as_i64());
        submissions.push((
            "submit_draft_note",
            base(json!({ "study_id": id, "note_text": "Note proposée par mcp-seed." })),
        ));
        if let Some(year) = year {
            submissions.push((
                "submit_draft_value",
                base(json!({ "study_id": id, "field": "eps", "fiscal_year": year,
                             "proposed_value": "1.23" })),
            ));
        }
        submissions.push((
            "submit_draft_value",
            base(json!({ "study_id": id, "field": "estimated_low_eps",
                         "proposed_value": "2.5" })),
        ));
        let current = study["study"]["judgment"]["forecast_low_option"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let option = if current == "recent_severe_low" {
            "avg_low_pe_times_eps"
        } else {
            "recent_severe_low"
        };
        submissions.push((
            "submit_draft_value",
            base(json!({ "study_id": id, "field": "forecast_low_option",
                         "proposed_value": option })),
        ));
    } else {
        println!("no study in the copy: only a draft study is seeded");
    }
    for (tool, args) in submissions {
        let (err, body) = c.call(tool, args)?;
        if err {
            println!("{tool}: refused — {} ({})", body["message"], body["code"]);
        } else {
            println!("{tool}: recorded {}", body["draft_id"]);
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [copy] = args.as_slice() else {
        eprintln!("usage: mcp-seed <copy of a dossier>");
        return ExitCode::from(2);
    };
    match run(copy) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("mcp-seed: {e}");
            ExitCode::FAILURE
        }
    }
}
