/// Library of small helper functions for rusty_bench.
/// The binary (src/main.rs) uses these helpers; keeping them in a library makes
/// them easy to test.

/// Compare decimal digits of two floating point estimates.
/// Returns the number of initial matching ASCII digits when both numbers are
/// formatted with up to `max_check` decimal places. Note: the implementation
/// counts the leading digit(s) (e.g. the '3' in 3.1415...) as part of the match.
pub fn matched_digits(estimate: f64, reference: f64, max_check: usize) -> usize {
    if !estimate.is_finite() {
        return 0;
    }
    let max_check = max_check.min(15);
    let est_s = format!("{:.1$}", estimate, max_check);
    let ref_s = format!("{:.1$}", reference, max_check);
    let est_digits: String = est_s.chars().filter(|c| c.is_ascii_digit()).collect();
    let ref_digits: String = ref_s.chars().filter(|c| c.is_ascii_digit()).collect();
    let mut matched = 0usize;
    for (a, b) in est_digits.chars().zip(ref_digits.chars()) {
        if a == b {
            matched += 1;
        } else {
            break;
        }
    }
    matched
}

/// Format a large integer into a human-readable string with suffixes.
/// Examples: 42 -> "42", 1_234 -> "1.23K", 1_234_567 -> "1.23M"
pub fn human_bytes(n: u64) -> String {
    if n < 1_000 {
        n.to_string()
    } else if n < 1_000_000 {
        format!("{:.2}K", n as f64 / 1_000f64)
    } else if n < 1_000_000_000 {
        format!("{:.2}M", n as f64 / 1_000_000f64)
    } else {
        format!("{:.2}B", n as f64 / 1_000_000_000f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matched_digits_exact() {
        // For identical numbers and max_check=5 we expect 1 (leading digit) + 5 digits = 6
        let m = matched_digits(std::f64::consts::PI, std::f64::consts::PI, 5);
        assert_eq!(m, 6);
    }

    #[test]
    fn test_matched_digits_nan() {
        assert_eq!(matched_digits(f64::NAN, std::f64::consts::PI, 10), 0);
    }

    #[test]
    fn test_human_bytes_small() {
        assert_eq!(human_bytes(42), "42");
    }

    #[test]
    fn test_human_bytes_k() {
        assert_eq!(human_bytes(1_234), "1.23K");
    }

    #[test]
    fn test_human_bytes_m() {
        assert_eq!(human_bytes(1_234_567), "1.23M");
    }

    #[test]
    fn test_human_bytes_b() {
        assert_eq!(human_bytes(1_234_567_890), "1.23B");
    }
}
