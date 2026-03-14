use crate::types::HistoricalData;

impl HistoricalData {
    /// Applies split and dividend adjustments to EPS and price fields.
    ///
    /// Multiplies `eps`, `price_high`, and `price_low` by each record's
    /// `adjustment_factor`. Records with a factor of `1` are left unchanged.
    /// Sets `is_split_adjusted = true` only when at least one record has a
    /// non-unity factor; for tickers without splits the flag stays `false`
    /// (no "Split-Adjusted" badge in UI) and subsequent calls re-scan harmlessly.
    pub fn apply_adjustments(&mut self) {
        if self.is_split_adjusted {
            return;
        }

        for record in &mut self.records {
            if record.adjustment_factor != rust_decimal::Decimal::ONE {
                record.eps *= record.adjustment_factor;
                record.price_high *= record.adjustment_factor;
                record.price_low *= record.adjustment_factor;
                self.is_split_adjusted = true;
            }
        }
    }

    /// Normalizes all monetary fields to `target_currency` using per-record exchange rates.
    ///
    /// Converts `sales`, `eps`, `price_high`, `price_low`, `net_income`,
    /// `pretax_income`, and `total_equity`. Records without an `exchange_rate`
    /// are left unchanged. This method is idempotent for the same target currency.
    pub fn apply_normalization(&mut self, target_currency: &str) {
        if self.display_currency.as_deref() == Some(target_currency) {
            return;
        }

        for record in &mut self.records {
            if let Some(rate) = record.exchange_rate {
                record.sales *= rate;
                record.eps *= rate;
                record.price_high *= rate;
                record.price_low *= rate;
                if let Some(ref mut val) = record.net_income {
                    *val *= rate;
                }
                if let Some(ref mut val) = record.pretax_income {
                    *val *= rate;
                }
                if let Some(ref mut val) = record.total_equity {
                    *val *= rate;
                }
            }
        }
        self.display_currency = Some(target_currency.to_string());
    }
}

#[cfg(test)]
mod tests {
    use crate::types::*;
    use rust_decimal::Decimal;

    #[test]
    fn test_currency_normalization() {
        let mut data = HistoricalData {
            ticker: "NESN.SW".to_string(),
            currency: "CHF".to_string(),
            display_currency: None,
            is_complete: true,
            is_split_adjusted: true,
            records: vec![HistoricalYearlyData {
                fiscal_year: 2021,
                sales: Decimal::from(100),
                eps: Decimal::from(10),
                price_high: Decimal::from(1000),
                price_low: Decimal::from(800),
                adjustment_factor: Decimal::ONE,
                exchange_rate: Some(Decimal::new(11, 1)), // 1.1
                net_income: None,
                pretax_income: None,
                total_equity: None,
                overrides: vec![],
            }],
            pe_range_analysis: None,
        };

        data.apply_normalization("USD");

        assert_eq!(data.display_currency, Some("USD".to_string()));
        assert_eq!(data.records[0].sales, Decimal::from(110)); // 100 * 1.1
        assert_eq!(data.records[0].eps, Decimal::from(11)); // 10 * 1.1
    }

    #[test]
    fn test_split_adjustment() {
        let mut data = HistoricalData {
            ticker: "AAPL".to_string(),
            currency: "USD".to_string(),
            display_currency: None,
            is_complete: true,
            is_split_adjusted: false,
            records: vec![
                HistoricalYearlyData {
                    fiscal_year: 2021,
                    eps: Decimal::from(10),
                    price_high: Decimal::from(100),
                    price_low: Decimal::from(80),
                    adjustment_factor: Decimal::ONE,
                    exchange_rate: None,
                    ..Default::default()
                },
                HistoricalYearlyData {
                    fiscal_year: 2019,
                    eps: Decimal::from(5),
                    price_high: Decimal::from(50),
                    price_low: Decimal::from(40),
                    adjustment_factor: Decimal::from(2), // 2:1 split
                    exchange_rate: None,
                    ..Default::default()
                },
            ],
            pe_range_analysis: None,
        };

        data.apply_adjustments();

        assert!(data.is_split_adjusted);
        // 2021 should remain unchanged
        assert_eq!(data.records[0].eps, Decimal::from(10));
        // 2019 should be doubled
        assert_eq!(data.records[1].eps, Decimal::from(10));
        assert_eq!(data.records[1].price_high, Decimal::from(100));
        assert_eq!(data.records[1].price_low, Decimal::from(80));
    }
}
