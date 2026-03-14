/// Validates that a string is a valid ISO 4217 currency code (3 uppercase ASCII letters).
///
/// Used at API boundaries to enforce currency format per architecture spec.
///
/// # Examples
///
/// ```
/// use steady_invest_logic::is_valid_currency_code;
///
/// assert!(is_valid_currency_code("CHF"));
/// assert!(is_valid_currency_code("USD"));
/// assert!(!is_valid_currency_code("us"));     // too short + lowercase
/// assert!(!is_valid_currency_code("USDX"));   // too long
/// assert!(!is_valid_currency_code("123"));     // digits, not letters
/// ```
pub fn is_valid_currency_code(code: &str) -> bool {
    code.len() == 3 && code.bytes().all(|b| b.is_ascii_uppercase())
}

/// Converts a monetary value from one currency to another using the given rate.
///
/// This is the single source of truth for currency conversion (Cardinal Rule).
/// The rate should be a directional rate from source to target currency
/// (e.g., CHF→USD = 1.15 means 1 CHF = 1.15 USD).
///
/// # Examples
///
/// ```
/// use steady_invest_logic::convert_monetary_value;
///
/// // Convert 100 CHF to USD at rate 1.15
/// let usd = convert_monetary_value(100.0, 1.15);
/// assert!((usd - 115.0).abs() < 1e-10);
///
/// // Same currency (rate = 1.0) returns unchanged value
/// assert!((convert_monetary_value(42.0, 1.0) - 42.0).abs() < 1e-10);
/// ```
pub fn convert_monetary_value(amount: f64, rate: f64) -> f64 {
    if !rate.is_finite() || rate <= 0.0 {
        return amount;
    }
    amount * rate
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_monetary_value_basic() {
        // CHF→USD at 1.15
        let usd = convert_monetary_value(100.0, 1.15);
        assert!((usd - 115.0).abs() < 1e-10);
    }

    #[test]
    fn test_convert_monetary_value_same_currency() {
        // Rate = 1.0 means same currency — no change
        assert!((convert_monetary_value(42.0, 1.0) - 42.0).abs() < 1e-10);
    }

    #[test]
    fn test_convert_monetary_value_zero_rate() {
        // Zero rate is invalid — returns original amount unchanged
        assert!((convert_monetary_value(100.0, 0.0) - 100.0).abs() < 1e-10);
    }

    #[test]
    fn test_convert_monetary_value_negative_rate() {
        // Negative rate is invalid — returns original amount unchanged
        assert!((convert_monetary_value(100.0, -1.5) - 100.0).abs() < 1e-10);
    }

    #[test]
    fn test_convert_monetary_value_nan_rate() {
        // NaN rate is invalid — returns original amount unchanged
        assert!((convert_monetary_value(100.0, f64::NAN) - 100.0).abs() < 1e-10);
    }

    #[test]
    fn test_convert_monetary_value_inf_rate() {
        // Infinite rate is invalid — returns original amount unchanged
        assert!((convert_monetary_value(100.0, f64::INFINITY) - 100.0).abs() < 1e-10);
    }

    #[test]
    fn test_convert_monetary_value_negative_amount() {
        // Negative amounts are valid (losses)
        let result = convert_monetary_value(-50.0, 1.15);
        assert!((result - (-57.5)).abs() < 1e-10);
    }

    #[test]
    fn test_is_valid_currency_code() {
        assert!(is_valid_currency_code("CHF"));
        assert!(is_valid_currency_code("USD"));
        assert!(is_valid_currency_code("EUR"));
        assert!(!is_valid_currency_code("us")); // too short + lowercase
        assert!(!is_valid_currency_code("usd")); // lowercase
        assert!(!is_valid_currency_code("USDX")); // too long
        assert!(!is_valid_currency_code("123")); // digits
        assert!(!is_valid_currency_code("")); // empty
        assert!(!is_valid_currency_code("U D")); // contains space
    }
}
