use backend::app::App;
use backend::services::exchange_rate_provider::{self, CachedRates, ExchangeRatePair, ExchangeRateResponse};
use chrono::Utc;
use loco_rs::testing::prelude::request;
use rust_decimal::Decimal;
use serial_test::serial;

/// Point the provider URL at a non-routable address so Frankfurter always
/// fails fast in tests, giving us deterministic fallback behaviour.
fn force_provider_offline() {
    std::env::set_var("EXCHANGE_RATE_PROVIDER_URL", "http://192.0.2.1:1/noop");
}

/// Restore the default provider URL after a test.
fn restore_provider_url() {
    std::env::remove_var("EXCHANGE_RATE_PROVIDER_URL");
}

/// Builds a `CachedRates` with realistic EUR/CHF/USD pairs, marked as fresh.
fn fresh_cached_rates() -> CachedRates {
    let eur_chf = Decimal::new(9400, 4);  // 0.9400
    let eur_usd = Decimal::new(10800, 4); // 1.0800
    let one = Decimal::ONE;

    CachedRates {
        rates: vec![
            ExchangeRatePair { from_currency: "EUR".into(), to_currency: "CHF".into(), rate: eur_chf },
            ExchangeRatePair { from_currency: "EUR".into(), to_currency: "USD".into(), rate: eur_usd },
            ExchangeRatePair { from_currency: "CHF".into(), to_currency: "EUR".into(), rate: one / eur_chf },
            ExchangeRatePair { from_currency: "USD".into(), to_currency: "EUR".into(), rate: one / eur_usd },
            ExchangeRatePair { from_currency: "CHF".into(), to_currency: "USD".into(), rate: eur_usd / eur_chf },
            ExchangeRatePair { from_currency: "USD".into(), to_currency: "CHF".into(), rate: eur_chf / eur_usd },
        ],
        fetched_at: Utc::now(),
        rate_date: "2026-02-12".to_string(),
    }
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[tokio::test]
#[serial]
async fn can_get_exchange_rates() {
    request::<App, _, _>(|request, _ctx| async move {
        // Seed the in-memory cache so the endpoint returns without hitting Frankfurter
        exchange_rate_provider::seed_cache(fresh_cached_rates()).await;

        let res = request.get("/api/v1/exchange-rates").await;
        res.assert_status_success();

        let body: ExchangeRateResponse = res.json();
        assert!(!body.stale, "Fresh cache should not be stale");
        assert_eq!(body.rates_as_of, "2026-02-12");
        assert!(!body.rates.is_empty(), "Rates should not be empty");

        // Verify Cache-Control header
        assert_eq!(res.header("cache-control"), "public, max-age=300");

        // Clean up
        exchange_rate_provider::clear_cache().await;
    })
    .await;
}

#[tokio::test]
#[serial]
async fn exchange_rates_returns_required_pairs() {
    request::<App, _, _>(|request, _ctx| async move {
        exchange_rate_provider::seed_cache(fresh_cached_rates()).await;

        let res = request.get("/api/v1/exchange-rates").await;
        res.assert_status_success();

        let body: ExchangeRateResponse = res.json();
        assert_eq!(body.rates.len(), 6, "Should return 6 directional pairs");

        // Verify all expected currency pairs exist
        let expected_pairs = [
            ("EUR", "CHF"), ("EUR", "USD"),
            ("CHF", "EUR"), ("USD", "EUR"),
            ("CHF", "USD"), ("USD", "CHF"),
        ];

        for (from, to) in &expected_pairs {
            let found = body.rates.iter().any(|r| {
                r.from_currency == *from && r.to_currency == *to
            });
            assert!(found, "Missing pair: {from}→{to}");
        }

        // Verify rates are positive
        for pair in &body.rates {
            assert!(
                pair.rate > Decimal::ZERO,
                "Rate for {}→{} should be positive, got {}",
                pair.from_currency, pair.to_currency, pair.rate,
            );
        }

        exchange_rate_provider::clear_cache().await;
    })
    .await;
}

#[tokio::test]
#[serial]
async fn exchange_rates_falls_back_to_db_rates() {
    request::<App, _, _>(|request, _ctx| async move {
        // Force Frankfurter to be unreachable so we deterministically test DB fallback
        force_provider_offline();
        exchange_rate_provider::clear_cache().await;

        // The migration seeds CHF→USD and EUR→USD for fiscal years 2016-2025.
        // With empty cache and Frankfurter unreachable, the handler must fall back to DB.
        let res = request.get("/api/v1/exchange-rates").await;
        res.assert_status_success();

        let body: ExchangeRateResponse = res.json();
        assert!(body.stale, "DB fallback should be stale");
        assert_eq!(body.rates.len(), 6, "Should have 6 directional pairs from DB");

        // rates_as_of should be a fiscal year string (e.g., "2025")
        let year: i32 = body.rates_as_of.parse()
            .expect("DB fallback rates_as_of should be a fiscal year");
        assert!(year >= 2016 && year <= 2030, "Year should be reasonable: {year}");

        // Verify rates are positive
        for pair in &body.rates {
            assert!(
                pair.rate > Decimal::ZERO,
                "DB fallback rate for {}→{} should be positive, got {}",
                pair.from_currency, pair.to_currency, pair.rate,
            );
        }

        // Clean up
        exchange_rate_provider::clear_cache().await;
        restore_provider_url();
    })
    .await;
}

#[tokio::test]
#[serial]
async fn exchange_rates_returns_503_when_no_data() {
    request::<App, _, _>(|_request, ctx| async move {
        // Force Frankfurter to be unreachable
        force_provider_offline();
        exchange_rate_provider::clear_cache().await;

        // Delete all exchange rates from the DB so there's no fallback
        use sea_orm::EntityTrait;
        use backend::models::_entities::exchange_rates;
        exchange_rates::Entity::delete_many()
            .exec(&ctx.db)
            .await
            .unwrap();

        // With empty cache + empty DB + unreachable provider → must get 503
        let res = _request.get("/api/v1/exchange-rates").await;
        assert_eq!(res.status_code(), 503, "Should return 503 when all sources fail");

        let body: serde_json::Value = res.json();
        assert!(
            body["error"].as_str().unwrap().contains("temporarily unavailable"),
            "503 body should contain unavailable message, got: {body}",
        );

        // Clean up
        exchange_rate_provider::clear_cache().await;
        restore_provider_url();
    })
    .await;
}
