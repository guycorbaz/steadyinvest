//! Story 8.4 AC 15 (arch §Phase 4 A12, dev safety): without `--dossier`, the server resolves the
//! dossier from the app-config under `XDG_CONFIG_HOME` / `HOME` — pointed at a temp directory here,
//! so the real home is never reached: (a) a temp `last_opened_path` is followed; (b) with no
//! config, the default dossier (under the temp `XDG_DATA_HOME`) is named missing and nothing is
//! created there but the server's log.

#![cfg(target_os = "linux")]

mod support;

use serde_json::json;
use support::*;

/// Every absolute path anywhere in `text` (a `/` that starts a path token — not the middle of a
/// word like « hausse/baisse ») starts with `root`; and the real home never appears.
fn every_path_under(text: &str, root: &str) {
    if let Some(real_home) = std::env::var_os("HOME") {
        let real_home = real_home.to_string_lossy().to_string();
        if !root.starts_with(&real_home) {
            assert!(
                !text.contains(&real_home),
                "the real home appeared in the output"
            );
        }
    }
    let bytes = text.as_bytes();
    for (i, _) in text.match_indices('/') {
        let starts_token = i == 0 || !(bytes[i - 1] as char).is_alphanumeric();
        let continues = bytes
            .get(i + 1)
            .is_some_and(|b| (*b as char).is_alphanumeric());
        if starts_token && continues {
            let token: String = text[i..]
                .chars()
                .take_while(|c| !matches!(c, '"' | ' ' | '\\' | ')' | ',' | '\n'))
                .collect();
            assert!(
                token.starts_with(root),
                "a path outside the temp home appeared: {token}"
            );
        }
    }
}

#[test]
fn the_config_pointer_under_a_temp_home_is_followed() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let home = root.join("home");
    let dossier_dir = root.join("dossiers");
    std::fs::create_dir_all(&dossier_dir).unwrap();
    let path = fixture_dossier(&dossier_dir);
    let config_dir = home.join("config").join("steadyinvest");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("config.json"),
        json!({ "journal_path": "/nowhere/refused.db", "last_opened_path": path }).to_string(),
    )
    .unwrap();

    let mut s = spawn(&[], &home);
    let list = s.ok("list_studies", json!({}));
    assert_eq!(list["total"], json!(2));
    assert_eq!(list["dossier"]["path"], json!(path.display().to_string()));
    every_path_under(&s.transcript.join("\n"), root.to_str().unwrap());
}

#[test]
fn with_no_config_the_temp_default_is_named_missing_and_nothing_is_created() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let home = root.join("home");
    let mut s = spawn(&[], &home);
    let body = s.refused("list_studies", json!({}));
    assert_eq!(body["code"], json!("no_dossier"));
    let resolved = body["resolved_path"].as_str().expect("the resolved path");
    assert!(
        resolved.starts_with(root.to_str().unwrap()),
        "resolved outside the temp home: {resolved}"
    );
    every_path_under(&s.transcript.join("\n"), root.to_str().unwrap());
    drop(s);
    // Nothing created in the data dir but the server's own log directory.
    let data = home.join("data").join("steadyinvest");
    let entries: Vec<String> = std::fs::read_dir(&data)
        .map(|rd| {
            rd.map(|e| e.unwrap().file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    assert!(
        entries.iter().all(|e| e == "logs"),
        "created in the data dir: {entries:?}"
    );
    assert!(!home.join("config").join("steadyinvest").exists());
}
