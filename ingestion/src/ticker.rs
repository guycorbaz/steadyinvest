//! Venue knowledge of the stored canonical ticker (EODHD's `TICKER.<code>` convention) — pure,
//! provider-neutral: the Twelve Data adapter pins its venue by MIC from this table (issue #70),
//! and the app names a study's listing venue from it (the comparison's row 30, G1 #237).

/// PURE: the pinned EODHD-venue-suffix → ISO 10383 **MIC** table (issue #70). Only venues whose
/// EODHD code and MIC are both unambiguous are listed — a MIC strictly filters the listing venue on
/// Twelve Data, so a correct entry can never fetch another exchange's price, and an *absent* entry
/// falls back to the verbatim-symbol rail (neutral no-data notice, never a venue guess). Extending
/// the table is a one-line, test-pinned change. `.US` (a virtual exchange) has no MIC.
pub fn venue_mic(suffix: &str) -> Option<&'static str> {
    // EODHD `TICKER.<code>` → operating MIC (ISO 10383).
    Some(match suffix.to_ascii_uppercase().as_str() {
        "SW" => "XSWX",    // SIX Swiss Exchange
        "PA" => "XPAR",    // Euronext Paris
        "L" => "XLON",     // London Stock Exchange
        "XETRA" => "XETR", // Deutsche Börse Xetra
        "F" => "XFRA",     // Börse Frankfurt (floor)
        "AS" => "XAMS",    // Euronext Amsterdam
        "BR" => "XBRU",    // Euronext Brussels
        "LS" => "XLIS",    // Euronext Lisbon
        "IR" => "XDUB",    // Euronext Dublin
        "MC" => "XMAD",    // Bolsa de Madrid
        "MI" => "XMIL",    // Borsa Italiana (Milan)
        "VI" => "XWBO",    // Wiener Börse
        "ST" => "XSTO",    // Nasdaq Stockholm
        "OL" => "XOSL",    // Oslo Børs
        "CO" => "XCSE",    // Nasdaq Copenhagen
        "HE" => "XHEL",    // Nasdaq Helsinki
        "TO" => "XTSE",    // Toronto Stock Exchange
        "V" => "XTSX",     // TSX Venture
        "HK" => "XHKG",    // Hong Kong Stock Exchange
        "AU" => "XASX",    // Australian Securities Exchange
        _ => return None,
    })
}

/// PURE: the listing venue of a stored canonical ticker (`NESN.SW` → `SW`, `BRK.B.US` → `US`) —
/// only a KNOWN venue: `.US` or a suffix of the pinned [`venue_mic`] table. A share-class suffix
/// (`BRK.B`) or an unmapped one is `None` — absent, never passed off as an exchange.
pub fn known_venue(ticker: &str) -> Option<String> {
    let (base, suffix) = ticker.rsplit_once('.')?;
    if base.is_empty() {
        return None;
    }
    (suffix.eq_ignore_ascii_case("US") || venue_mic(suffix).is_some())
        .then(|| suffix.to_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn venue_mic_table_is_case_insensitive_and_absent_for_unknown() {
        assert_eq!(venue_mic("SW"), Some("XSWX"));
        assert_eq!(venue_mic("sw"), Some("XSWX"));
        assert_eq!(venue_mic("XX"), None);
        assert_eq!(venue_mic("US"), None); // `.US` is handled upstream (bare symbol, no MIC)
    }

    #[test]
    fn a_known_venue_is_never_a_share_class() {
        assert_eq!(known_venue("NESN.SW").as_deref(), Some("SW"));
        assert_eq!(known_venue("aapl.us").as_deref(), Some("US"));
        assert_eq!(known_venue("BRK.B.US").as_deref(), Some("US"));
        assert_eq!(known_venue("BRK.B"), None); // a share class, not an exchange
        assert_eq!(known_venue("NESN.XX"), None); // unmapped: absent, never a guess
        assert_eq!(known_venue("NESN"), None);
        assert_eq!(known_venue(".SW"), None);
    }
}
