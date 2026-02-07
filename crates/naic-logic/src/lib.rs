use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TickerInfo {
    pub ticker: String,
    pub name: String,
    pub exchange: String,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct HistoricalYearlyData {
    pub fiscal_year: i32,
    pub sales: rust_decimal::Decimal,
    pub eps: rust_decimal::Decimal,
    pub price_high: rust_decimal::Decimal,
    pub price_low: rust_decimal::Decimal,
    pub adjustment_factor: rust_decimal::Decimal,
    pub exchange_rate: Option<rust_decimal::Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct HistoricalData {
    pub ticker: String,
    pub currency: String,
    pub display_currency: Option<String>,
    pub records: Vec<HistoricalYearlyData>,
    pub is_complete: bool,
    pub is_split_adjusted: bool,
}

impl HistoricalData {
    pub fn apply_adjustments(&mut self) {
        if self.is_split_adjusted {
            return;
        }

        let mut adjusted = false;
        for record in &mut self.records {
            if record.adjustment_factor != rust_decimal::Decimal::ONE {
                record.eps *= record.adjustment_factor;
                record.price_high *= record.adjustment_factor;
                record.price_low *= record.adjustment_factor;
                adjusted = true;
            }
        }
        if adjusted {
            self.is_split_adjusted = true;
        }
    }

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
            }
        }
        self.display_currency = Some(target_currency.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn test_currency_normalization() {
        let mut data = HistoricalData {
            ticker: "NESN.SW".to_string(),
            currency: "CHF".to_string(),
            display_currency: None,
            is_complete: true,
            is_split_adjusted: true,
            records: vec![
                HistoricalYearlyData {
                    fiscal_year: 2021,
                    sales: Decimal::from(100),
                    eps: Decimal::from(10),
                    price_high: Decimal::from(1000),
                    price_low: Decimal::from(800),
                    adjustment_factor: Decimal::ONE,
                    exchange_rate: Some(Decimal::new(11, 1)), // 1.1
                },
            ],
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
