//! Provider rate-limit model — tracks API quota consumption per provider.

use loco_rs::prelude::*;
pub use super::_entities::provider_rate_limits::{ActiveModel, Entity, Model};

impl Model {
    /// Finds a provider rate-limit record by provider name.
    pub async fn find_by_name(db: &DatabaseConnection, name: &str) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(super::_entities::provider_rate_limits::Column::Name.eq(name))
            .one(db)
            .await
    }

    /// Upserts the quota consumption for a provider (creates if not found).
    pub async fn update_quota(
        db: &DatabaseConnection,
        name: &str,
        consumed: i32,
    ) -> Result<Self, DbErr> {
        let model = Self::find_by_name(db, name).await?;
        let mut active_model: ActiveModel = match model {
            Some(m) => m.into(),
            None => ActiveModel {
                name: Set(name.to_string()),
                ..Default::default()
            },
        };
        active_model.quota_consumed = Set(consumed);
        active_model.last_updated = Set(chrono::Utc::now().naive_utc());
        
        let saved = active_model.save(db).await?;
        // Use sea_orm::TryIntoModel
        use sea_orm::TryIntoModel;
        Ok(saved.try_into_model().map_err(|_| DbErr::Custom("Failed to convert ActiveModel to Model".to_string()))?)
    }
}
