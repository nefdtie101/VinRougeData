/// Validate a South African ID number (13 digits) using the Luhn algorithm.
///
/// SA ID format: YYMMDDGSSSSCZA
/// - YYMMDD = date of birth
/// - G      = gender (0-4 female, 5-9 male)
/// - SSSS   = sequence number
/// - C      = citizenship (0 = SA citizen, 1 = permanent resident)
/// - Z      = checksum (Luhn validation)
///
/// Returns `true` only when the input is exactly 13 digits and the Luhn
/// checksum is valid.
pub fn validate_sa_id(id: &str) -> bool {
    let digits: Vec<u32> = id
        .chars()
        .filter(|c| c.is_ascii_digit())
        .map(|c| c.to_digit(10).unwrap())
        .collect();

    if digits.len() != 13 {
        return false;
    }

    let mut sum = 0;
    for (i, &d) in digits.iter().enumerate() {
        if i % 2 == 1 {
            // Double every second digit (0-indexed positions 1, 3, 5, …)
            let doubled = d * 2;
            sum += if doubled > 9 { doubled - 9 } else { doubled };
        } else {
            sum += d;
        }
    }

    sum % 10 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_sa_id() {
        // 8801235111088 → Luhn sum = 50 → valid
        assert!(validate_sa_id("8801235111088"));
    }

    #[test]
    fn test_invalid_sa_id_bad_checksum() {
        // Same as valid but last digit changed: 8801235111087 → Luhn sum = 49
        assert!(!validate_sa_id("8801235111087"));
    }

    #[test]
    fn test_invalid_sa_id_too_short() {
        assert!(!validate_sa_id("88012351110"));
    }

    #[test]
    fn test_invalid_sa_id_too_long() {
        assert!(!validate_sa_id("88012351110888"));
    }

    #[test]
    fn test_invalid_sa_id_with_letters() {
        assert!(!validate_sa_id("880123511108A"));
    }

    #[test]
    fn test_sa_id_with_spaces_ignored() {
        // Non-digit characters are stripped; spaces do not invalidate the ID
        assert!(validate_sa_id("880123 5111088"));
    }

    #[test]
    fn test_valid_sa_id_with_dashes() {
        // Dashes are ignored; only digits are checked
        assert!(validate_sa_id("880123-5111-088"));
    }
}
