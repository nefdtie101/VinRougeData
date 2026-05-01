/// SQL LIKE matching: `%` = any sequence, `_` = any single char, case-insensitive.
pub(super) fn like_match(text: &str, pattern: &str) -> bool {
    let t: Vec<char> = text.chars().collect();
    let p: Vec<char> = pattern.chars().collect();
    let (tn, pn) = (t.len(), p.len());
    let mut dp = vec![vec![false; pn + 1]; tn + 1];
    dp[0][0] = true;
    for j in 1..=pn {
        if p[j - 1] == '%' {
            dp[0][j] = dp[0][j - 1];
        }
    }
    for i in 1..=tn {
        for j in 1..=pn {
            dp[i][j] = match p[j - 1] {
                '%' => dp[i - 1][j] || dp[i][j - 1],
                '_' => dp[i - 1][j - 1],
                c   => dp[i - 1][j - 1] && t[i - 1].to_ascii_lowercase() == c.to_ascii_lowercase(),
            };
        }
    }
    dp[tn][pn]
}

/// Normalise common date formats to ISO 8601 (YYYY-MM-DD) for consistent string ordering.
/// Recognises: YYYY-MM-DD, YYYY/MM/DD, DD/MM/YYYY, DD-MM-YYYY, MM/DD/YYYY.
/// Falls back to the original string if unrecognised.
pub(super) fn normalize_date(s: &str) -> String {
    let s = s.trim();
    if s.len() == 10 {
        let sep = s.as_bytes()[4];
        if sep == b'-' || sep == b'/' {
            let y = &s[0..4];
            let m = &s[5..7];
            let d = &s[8..10];
            if y.chars().all(|c| c.is_ascii_digit()) {
                return format!("{y}-{m}-{d}");
            }
        }
        let sep2 = s.as_bytes()[2];
        if sep2 == b'/' || sep2 == b'-' {
            let a = &s[0..2];
            let b = &s[3..5];
            let y = &s[6..10];
            if y.chars().all(|c| c.is_ascii_digit())
                && a.chars().all(|c| c.is_ascii_digit())
                && b.chars().all(|c| c.is_ascii_digit())
            {
                let first: u32 = a.parse().unwrap_or(0);
                let (month, day) = if first > 12 { (b, a) } else { (a, b) };
                return format!("{y}-{month}-{day}");
            }
        }
    }
    s.to_string()
}

/// Return true when `s` matches a recognisable date pattern.
/// Accepts: YYYY-MM-DD, YYYY/MM/DD, DD/MM/YYYY, DD-MM-YYYY, MM/DD/YYYY.
pub(super) fn is_date_str(s: &str) -> bool {
    if s.len() != 10 { return false; }
    let b = s.as_bytes();
    let sep4 = b[4];
    let sep2 = b[2];
    if (sep4 == b'-' || sep4 == b'/') && b[7] == sep4 {
        return b[0..4].iter().all(|c| c.is_ascii_digit())
            && b[5..7].iter().all(|c| c.is_ascii_digit())
            && b[8..10].iter().all(|c| c.is_ascii_digit());
    }
    if (sep2 == b'-' || sep2 == b'/') && b[5] == sep2 {
        return b[0..2].iter().all(|c| c.is_ascii_digit())
            && b[3..5].iter().all(|c| c.is_ascii_digit())
            && b[6..10].iter().all(|c| c.is_ascii_digit());
    }
    false
}
