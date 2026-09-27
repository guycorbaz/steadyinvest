//! Shared test support of the `steadyinvest-mcp` integration tests (Story 8.4): a fixture dossier
//! builder, marker seeding of the portfolio tables, and a stdio JSON-RPC driver of the real binary.
//!
//! Dev-safety (arch §Phase 4 A12): every spawn passes a temp `--dossier` (or none, in the home
//! isolation test) and points `HOME`, `XDG_CONFIG_HOME` and `XDG_DATA_HOME` at a temp directory —
//! the real app-config and the real dossier are never reachable.

#![allow(dead_code)] // each test file uses a subset

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;
use steadyinvest_contract::{
    Cell, Coverage, ForecastLowOption, Freshness, Judgment, Money, Provenance, Review, Source,
    Study, Timestamp, YearData,
};
use uuid::Uuid;

pub const JID: u128 = 0x8400;
pub const STUDY_A: u128 = 0x84a;
pub const STUDY_B: u128 = 0x84b;

pub fn ts(s: &str) -> Timestamp {
    Timestamp(s.to_string())
}

pub fn money(s: &str) -> Money {
    serde_json::from_str(&format!("\"{s}\"")).expect("decimal parses")
}

pub fn cell(v: &str) -> Cell {
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

pub fn study(id: u128, ticker: &str, currency: &str, created: &str) -> Study {
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
    s.years = vec![
        YearData {
            year: 2024,
            sales: cell("900"),
            eps: cell("3.8"),
            high_price: cell("100"),
            low_price: cell("70"),
            dividend_per_share: None,
            pre_tax_profit: None,
            book_value_per_share: None,
        },
        YearData {
            year: 2025,
            sales: cell("1000"),
            eps: cell("4.1"),
            high_price: cell("110"),
            low_price: cell("80"),
            dividend_per_share: None,
            pre_tax_profit: None,
            book_value_per_share: None,
        },
    ];
    s
}

/// A closed WAL dossier at `<dir>/dossier.db` holding studies A (NESN.SW, CHF) and B (AAPL, USD).
#[expect(
    clippy::disallowed_types,
    reason = "test fixture builder, never in the binary"
)]
pub fn fixture_dossier(dir: &Path) -> PathBuf {
    let path = dir.join("dossier.db");
    let mut j = steadyinvest_persistence::Journal::create(
        &path,
        Uuid::from_u128(JID),
        &ts("2026-09-28T07:00:00Z"),
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

/// Unique marker strings in one row of every portfolio / watchlist / cache table (incl. a
/// dividend transaction); returns the markers.
pub fn seed_markers(path: &Path) -> Vec<String> {
    let conn = rusqlite::Connection::open(path).expect("raw");
    conn.pragma_update(None, "foreign_keys", false)
        .expect("fk off for seeding");
    let mut markers = Vec::new();
    for table in [
        "portfolios",
        "holdings",
        "transactions",
        "watchlist_items",
        "fx_rates",
        "price_history",
    ] {
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
    conn.execute("UPDATE transactions SET kind = 'dividend-MARKER-kind'", [])
        .expect("dividend");
    markers.push("dividend-MARKER-kind".to_string());
    markers
}

/// A running `steadyinvest-mcp` driven over stdio, with its captured stdout.
pub struct Server {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
    next_id: u64,
    /// Every stdout line received (the non-exposure suite scans them all).
    pub transcript: Vec<String>,
}

/// Spawn the binary with `args`, `HOME` / `XDG_CONFIG_HOME` / `XDG_DATA_HOME` under `home`.
pub fn spawn(args: &[&str], home: &Path) -> Server {
    let config = home.join("config");
    let data = home.join("data");
    std::fs::create_dir_all(&config).expect("config dir");
    std::fs::create_dir_all(&data).expect("data dir");
    let mut child = Command::new(env!("CARGO_BIN_EXE_steadyinvest-mcp"))
        .args(args)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", &config)
        .env("XDG_DATA_HOME", &data)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn steadyinvest-mcp");
    let stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut server = Server {
        child,
        stdin,
        lines: rx,
        next_id: 1,
        transcript: Vec::new(),
    };
    server.initialize();
    server
}

impl Server {
    fn send(&mut self, message: &Value) {
        let mut line = message.to_string();
        line.push('\n');
        self.stdin.write_all(line.as_bytes()).expect("write");
        self.stdin.flush().expect("flush");
    }

    /// Send a request and wait (20 s at most) for the response with its id.
    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let line = self
                .lines
                .recv_timeout(Duration::from_secs(20))
                .unwrap_or_else(|_| panic!("no response to {method} within 20 s"));
            self.transcript.push(line.clone());
            let message: Value = serde_json::from_str(&line)
                .unwrap_or_else(|e| panic!("stdout line is not JSON-RPC ({e}): {line}"));
            if message["id"] == json!(id) {
                return message;
            }
        }
    }

    fn initialize(&mut self) {
        let response = self.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "steadyinvest-mcp-tests", "version": "0" },
            }),
        );
        assert!(
            response["result"]["serverInfo"]["name"] == json!("steadyinvest-mcp"),
            "{response}"
        );
        self.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
    }

    /// `tools/list` → the tool names.
    pub fn tool_names(&mut self) -> Vec<String> {
        let r = self.request("tools/list", json!({}));
        r["result"]["tools"]
            .as_array()
            .expect("tools")
            .iter()
            .map(|t| t["name"].as_str().expect("name").to_string())
            .collect()
    }

    /// `tools/call` → (`isError`, the JSON body of the first text content).
    pub fn call(&mut self, tool: &str, arguments: Value) -> (bool, Value) {
        let r = self.request(
            "tools/call",
            json!({ "name": tool, "arguments": arguments }),
        );
        let result = &r["result"];
        assert!(result.is_object(), "tools/call {tool} failed: {r}");
        let text = result["content"][0]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("no text content: {r}"));
        let body: Value = serde_json::from_str(text).expect("the text is JSON");
        (result["isError"] == json!(true), body)
    }

    /// A successful call's body (panics with the error body otherwise).
    pub fn ok(&mut self, tool: &str, arguments: Value) -> Value {
        let (is_error, body) = self.call(tool, arguments);
        assert!(!is_error, "{tool} refused: {body}");
        body
    }

    /// A refused call's body (panics with the body if it succeeded).
    pub fn refused(&mut self, tool: &str, arguments: Value) -> Value {
        let (is_error, body) = self.call(tool, arguments);
        assert!(is_error, "{tool} was not refused: {body}");
        body
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The submission's common arguments (comment, origin, the dossier read).
pub fn common(dossier: &Value) -> serde_json::Map<String, Value> {
    let mut m = serde_json::Map::new();
    m.insert("comment".into(), json!("test proposition"));
    m.insert("origin_client".into(), json!("tests"));
    m.insert("origin_model".into(), json!("none"));
    m.insert("dossier_journal_id".into(), dossier["journal_id"].clone());
    m.insert("dossier_path".into(), dossier["path"].clone());
    m
}

/// `common` plus `extra`.
pub fn submit_args(dossier: &Value, extra: Value) -> Value {
    let mut m = common(dossier);
    if let Value::Object(e) = extra {
        m.extend(e);
    }
    Value::Object(m)
}
