use loco_rs::prelude::*;
pub use super::_entities::audit_logs::{ActiveModel, Entity, Model};
use sea_orm::{QueryOrder, QuerySelect};

impl Model {
    pub async fn find_recent(db: &DatabaseConnection, limit: u64) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .order_by_desc(super::_entities::audit_logs::Column::CreatedAt)
            .limit(limit)
            .all(db)
            .await
    }

    pub async fn create(
        db: &DatabaseConnection,
        ticker: &str,
        exchange: &str,
        field: &str,
        old: Option<String>,
        new: Option<String>,
        event: &str,
        source: &str,
    ) -> Result<Self, DbErr> {
        let active_model = ActiveModel {
            ticker: Set(ticker.to_string()),
            exchange: Set(exchange.to_string()),
            field_name: Set(field.to_string()),
            old_value: Set(old),
            new_value: Set(new),
            event_type: Set(event.to_string()),
            source: Set(source.to_string()),
            created_at: Set(chrono::Utc::now().naive_utc()),
            ..Default::default()
        };
        active_model.insert(db).await
    }
}
