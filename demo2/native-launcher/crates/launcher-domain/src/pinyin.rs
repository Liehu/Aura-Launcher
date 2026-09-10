//! Pinyin first-letter + full-syllable matching (P2.5/P2.6-A04/C04 and the
//! deferred "full pinyin table" optimization): index pinyin keys for CJK
//! filenames so users can search Chinese files by typing romanized input.
//! The accurate common-character table (pinyin_table) is the primary
//! lookup; characters outside it fall back to the deterministic boundary
//! approximation, so accuracy only improves as the table grows.

/// Extract the pinyin initial (lowercase a-z) for a CJK character.
/// Table-first; None for non-CJK.
pub fn pinyin_initial(c: char) -> Option<char> {
    let code = c as u32;
    if !(0x4E00..=0x9FFF).contains(&code) {
        return None;
    }
    match crate::pinyin_table::lookup_syllable(c) {
        Some(syl) => syl.chars().next(),
        None => Some(boundary_initial(code)),
    }
}

/// The accurate full pinyin syllable (tone-free) for a character, when it
/// is in the common-character table. None = unknown (use `pinyin_initial`
/// for the approximated initial instead).
pub fn pinyin_syllable(c: char) -> Option<&'static str> {
    if !(0x4E00..=0x9FFF).contains(&(c as u32)) {
        return None;
    }
    crate::pinyin_table::lookup_syllable(c)
}

/// Full pinyin romanization for a filename: syllables for table-covered
/// characters, the approximated initial for CJK characters outside the
/// table (so mixed names stay matchable), non-CJK skipped. Lowercase.
pub fn pinyin_full(name: &str) -> String {
    name.chars()
        .filter_map(|c| match pinyin_syllable(c) {
            Some(syl) => Some(syl.to_string()),
            None if (0x4E00..=0x9FFF).contains(&(c as u32)) => {
                Some(boundary_initial(c as u32).to_string())
            }
            None => None,
        })
        .collect()
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
    name.chars().filter_map(pinyin_initial).collect()
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

    /// Table-first initials: accurate for covered characters.
    #[test]
    fn initials_use_table() {
        assert_eq!(pinyin_initial('的'), Some('d'));
        assert_eq!(pinyin_initial('文'), Some('w'));
        assert_eq!(pinyin_initial('A'), None);
    }

    /// Full pinyin: syllables for covered chars, initials for the rest.
    #[test]
    fn full_pinyin_romanization() {
        assert_eq!(pinyin_full("文件夹"), "wenjianjia");
        assert_eq!(pinyin_full("报告"), "baogao");
        assert_eq!(pinyin_full("报表"), "baobiao");
        // non-CJK skipped
        assert_eq!(pinyin_full("report报告"), "baogao");
        // unknown CJK falls back to the boundary initial (still a letter)
        let s = pinyin_full("\u{9FFF}");
        assert_eq!(s.len(), 1);
        assert!(s.chars().next().unwrap().is_ascii_lowercase());
    }
}
