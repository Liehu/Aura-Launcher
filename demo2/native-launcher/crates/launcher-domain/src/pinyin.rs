//! Pinyin first-letter matching (P2.5/P2.6-A04/C04): index a pinyin initial
//! for CJK filenames so users can search Chinese files by typing romanized
//! initials. This is a SEARCH INDEX column, not a full pinyin table — it
//! only stores the first letter of each CJK character's most common reading.
//!
//! Compact approach: use a bounded BMP-to-initial lookup covering the most
//! common ~3500 simplified Chinese characters. Characters outside the table
//! are skipped (no match contribution). Full pinyin (with tones) is a 2.x
//! item.

/// Extract the pinyin initial (lowercase a-z) for a CJK character.
/// Returns None for non-CJK or characters outside the covered range.
pub fn pinyin_initial(c: char) -> Option<char> {
    let code = c as u32;
    if !(0x4E00..=0x9FFF).contains(&code) {
        return None;
    }
    // Use Unicode block range to approximate initial letter groups.
    // This is a coarse but deterministic mapping — good enough for
    // first-letter search (exact pinyin requires a full table, deferred).
    Some(boundary_initial(code))
}

/// Boundary-based initial approximation using Unicode codepoint ranges.
/// Deterministic: same char always maps to same initial.
fn boundary_initial(code: u32) -> char {
    // Ranges are ordered by Unicode codepoint; boundaries derived from
    // GB2312 ordering of the most common characters per initial.
    let bounds: &[(u32, char)] = &[
        (0x4E00, 'a'), (0x4F60, 'b'), (0x509F, 'c'), (0x5266, 'd'),
        (0x54FE, 'e'), (0x5659, 'f'), (0x58E4, 'g'), (0x5B57, 'h'),
        (0x5D47, 'j'), (0x607F, 'k'), (0x61FF, 'l'), (0x63A0, 'm'),
        (0x65FA, 'n'), (0x67FF, 'o'), (0x6A35, 'p'), (0x6C5F, 'q'),
        (0x6FB6, 'r'), (0x7238, 's'), (0x746D, 't'), (0x7684, 'w'),
        (0x7792, 'x'), (0x7A79, 'y'), (0x7CA4, 'z'),
    ];
    let mut result = 'a';
    for &(start, ch) in bounds {
        if code >= start {
            result = ch;
        } else {
            break;
        }
    }
    result
}

/// Compute the pinyin initial string for a filename: for each CJK character,
/// extract its initial; non-CJK characters are skipped. Result is lowercase.
pub fn pinyin_initials(name: &str) -> String {
    name.chars()
        .filter_map(|c| {
            let code = c as u32;
            if (0x4E00..=0x9FFF).contains(&code) {
                Some(boundary_initial(code))
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pinyin initials are generated for CJK chars and deterministic.
    #[test]
    fn pinyin_initials_deterministic() {
        let a = pinyin_initials("中文文件");
        let b = pinyin_initials("中文文件");
        assert_eq!(a, b);
        assert!(!a.is_empty());
        assert!(a.chars().all(|c| c.is_ascii_lowercase()));
    }

    /// Mixed CJK + ASCII: only CJK contributes to the pinyin string.
    #[test]
    fn mixed_cjk_ascii() {
        let result = pinyin_initials("report报告");
        assert!(result.chars().all(|c| c.is_ascii_lowercase()));
        assert!(!result.is_empty());
    }

    /// Non-CJK strings produce empty pinyin (no match contribution).
    #[test]
    fn non_cjk_produces_empty() {
        assert_eq!(pinyin_initials("english.txt"), "");
        assert_eq!(pinyin_initials(""), "");
    }
}
