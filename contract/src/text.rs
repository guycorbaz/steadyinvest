//! The one « blank text » rule (Story 8.1, moved here by Story 8.3): a text made only of whitespace
//! and invisible Unicode format characters (category Cf — zero-width spaces, bidi marks, BOM…) is
//! **blank**. The owner's note rail (app) and the MCP submission checks (persistence) both use it,
//! so a note, a comment or an origin that shows nothing is refused the same way everywhere.

/// Whether `text` shows nothing: every character is whitespace or a Unicode format character.
/// The empty string is blank.
pub fn is_blank(text: &str) -> bool {
    text.chars().all(|c| c.is_whitespace() || is_format_char(c))
}

/// Unicode general category Cf (format characters), the ranges that can appear in typed or pasted
/// text — std has no category table.
pub fn is_format_char(c: char) -> bool {
    matches!(
        c as u32,
        0x00AD
            | 0x0600..=0x0605
            | 0x061C
            | 0x06DD
            | 0x070F
            | 0x0890..=0x0891
            | 0x08E2
            | 0x180E
            | 0x200B..=0x200F
            | 0x202A..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x206F
            | 0xFEFF
            | 0xFFF9..=0xFFFB
            | 0x110BD
            | 0x110CD
            | 0x13430..=0x1343F
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0001
            | 0xE0020..=0xE007F
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_and_format_characters_alone_are_blank() {
        for blank in [
            "",
            " ",
            "\t\n",
            "\u{00A0}",
            "\u{3000}",
            "\u{200B}",
            "\u{FEFF} \u{200D}",
        ] {
            assert!(is_blank(blank), "{blank:?} shows nothing");
        }
    }

    #[test]
    fn any_visible_character_is_not_blank() {
        for text in ["a", " x ", "\u{200B}é", "0", "—"] {
            assert!(!is_blank(text), "{text:?} shows something");
        }
    }
}
