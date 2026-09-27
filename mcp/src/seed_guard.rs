//! The dev-safety guard of the `seed` example (Story 8.4 AC 18, arch §Phase 4 A12): seeding the
//! owner's real dossier is impossible by construction — the guard refuses any `<copy>` that is,
//! canonically, one of the dossiers the real app-config points to, or the app's default dossier.

use std::path::{Path, PathBuf};

/// Why a seed target is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeedRefusal {
    /// The path does not exist (the seed never creates a dossier).
    Missing(PathBuf),
    /// The path is a dossier the real app uses.
    RealDossier(PathBuf),
    /// The real app-config could not be read, so the guard cannot tell — refused.
    ConfigUnreadable(String),
}

impl std::fmt::Display for SeedRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SeedRefusal::Missing(p) => write!(f, "{} does not exist", p.display()),
            SeedRefusal::RealDossier(p) => write!(
                f,
                "{} is a dossier of the real app; seed a throw-away copy only",
                p.display()
            ),
            SeedRefusal::ConfigUnreadable(d) => {
                write!(f, "the app-config could not be read ({d}); refused")
            }
        }
    }
}

fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Refuse `copy` if it is missing or equal (canonically) to any of `real` — the pure rule.
pub fn check_seed_target_against(copy: &Path, real: &[PathBuf]) -> Result<PathBuf, SeedRefusal> {
    if !copy.exists() {
        return Err(SeedRefusal::Missing(copy.to_path_buf()));
    }
    let target = canonical(copy);
    if real.iter().any(|r| canonical(r) == target) {
        return Err(SeedRefusal::RealDossier(copy.to_path_buf()));
    }
    Ok(target)
}

/// The dossiers the real app-config points to (`last_opened_path`, `journal_path`) and the app's
/// default dossier — read from the REAL environment (never from the seed's temp dirs).
pub fn real_dossiers() -> Result<Vec<PathBuf>, SeedRefusal> {
    let mut out = Vec::new();
    if let Some(config) = steadyinvest_paths::config_file_path() {
        match steadyinvest_paths::read_dossier_pointers(&config) {
            Ok(Some(p)) => {
                out.extend(p.last_opened_path);
                out.extend(p.journal_path);
            }
            Ok(None) => {}
            Err(e) => return Err(SeedRefusal::ConfigUnreadable(e.to_string())),
        }
    }
    out.extend(steadyinvest_paths::default_journal_path());
    Ok(out)
}

/// The full guard: refuse a missing target or a real dossier.
pub fn check_seed_target(copy: &Path) -> Result<PathBuf, SeedRefusal> {
    check_seed_target_against(copy, &real_dossiers()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_real_dossier_is_refused_even_through_another_spelling() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("journal.db");
        std::fs::write(&real, b"x").unwrap();
        let other_spelling = dir.path().join(".").join("journal.db");
        assert_eq!(
            check_seed_target_against(&other_spelling, std::slice::from_ref(&real)),
            Err(SeedRefusal::RealDossier(other_spelling.clone()))
        );
        #[cfg(unix)]
        {
            let link = dir.path().join("link.db");
            std::os::unix::fs::symlink(&real, &link).unwrap();
            assert!(matches!(
                check_seed_target_against(&link, std::slice::from_ref(&real)),
                Err(SeedRefusal::RealDossier(_))
            ));
        }
    }

    #[test]
    fn a_copy_elsewhere_passes_and_a_missing_one_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("journal.db");
        let copy = dir.path().join("copy.db");
        std::fs::write(&real, b"x").unwrap();
        std::fs::write(&copy, b"x").unwrap();
        assert!(check_seed_target_against(&copy, std::slice::from_ref(&real)).is_ok());
        let missing = dir.path().join("missing.db");
        assert_eq!(
            check_seed_target_against(&missing, &[real]),
            Err(SeedRefusal::Missing(missing))
        );
    }
}
