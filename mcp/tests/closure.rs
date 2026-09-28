//! Story 8.4 AC 1 (arch §Phase 4 A1, FR15 / FR21 / FR76, NFR-A3, NFR-S1): the resolved
//! normal-dependency closure of `steadyinvest-mcp` holds no provider / network crate, no HTTP
//! client, no keychain and no GUI — so no provider call and no key read is even linkable.

use serde_json::Value;
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::process::Command;

const FORBIDDEN: [&str; 17] = [
    "steadyinvest-ingestion",
    "steadyinvest-app",
    "reqwest",
    "hyper",
    "hyper-util",
    "h2",
    "ureq",
    "isahc",
    "curl",
    "surf",
    "attohttpc",
    "keyring",
    "secret-service",
    "slint",
    "rfd",
    "axum",
    "tower-http",
];

/// The resolved normal-dependency closure of `package` (every platform), by package name.
fn closure_of(package: &str) -> BTreeSet<String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--locked"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo metadata runs");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let meta: Value = serde_json::from_slice(&output.stdout).expect("metadata JSON");
    let names: HashMap<&str, &str> = meta["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .map(|p| (p["id"].as_str().unwrap(), p["name"].as_str().unwrap()))
        .collect();
    let nodes: HashMap<&str, &Value> = meta["resolve"]["nodes"]
        .as_array()
        .expect("resolve nodes")
        .iter()
        .map(|n| (n["id"].as_str().unwrap(), n))
        .collect();
    let root = names
        .iter()
        .find(|(_, n)| **n == package)
        .map(|(id, _)| *id)
        .expect("the package");
    // Breadth-first over NORMAL edges only (a `dep_kinds` entry with `kind: null`), every platform.
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut queue = VecDeque::from([root]);
    while let Some(id) = queue.pop_front() {
        if !seen.insert(id) {
            continue;
        }
        for dep in nodes[id]["deps"].as_array().expect("deps") {
            let normal = dep["dep_kinds"]
                .as_array()
                .expect("dep_kinds")
                .iter()
                .any(|k| k["kind"].is_null());
            if normal {
                queue.push_back(dep["pkg"].as_str().unwrap());
            }
        }
    }
    seen.iter().map(|id| names[id].to_string()).collect()
}

#[test]
fn the_dependency_closure_excludes_every_network_keychain_and_gui_crate() {
    let closure = closure_of("steadyinvest-mcp");
    assert!(closure.contains("rmcp"), "the walk reaches the SDK");
    assert!(
        closure.contains("steadyinvest-persistence"),
        "the walk reaches the access layer"
    );
    let leaked: Vec<&&str> = FORBIDDEN.iter().filter(|f| closure.contains(**f)).collect();
    assert!(
        leaked.is_empty(),
        "forbidden crates in the steadyinvest-mcp closure: {leaked:?}"
    );
}

/// Negative control (G3): the same walk from the app DOES find the network and keychain crates —
/// the test above can fail.
#[test]
fn the_walk_finds_the_network_and_keychain_crates_of_the_app() {
    let closure = closure_of("steadyinvest-app");
    for name in ["reqwest", "keyring", "steadyinvest-ingestion", "slint"] {
        assert!(closure.contains(name), "{name} not found from the app");
    }
}
