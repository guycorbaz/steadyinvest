//! `SeaORM` Entity for `analysis_snapshots` table.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "analysis_snapshots")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_id: i32,
    pub ticker_id: i32,
    pub snapshot_data: Json,
    pub thesis_locked: bool,
    // NOTE: The actual DB column is MEDIUMBLOB (created via migration custom type).
    // SeaORM has no MediumBlob column_type variant, so VarBinary is used as a
    // compatible stand-in for reading/writing Vec<u8>. If regenerating entities
    // with sea-orm-cli, this annotation will need manual restoration.
    #[sea_orm(column_type = "VarBinary(StringLen::None)", nullable)]
    pub chart_image: Option<Vec<u8>>,
    #[sea_orm(column_type = "Text", nullable)]
    pub notes: Option<String>,
    pub captured_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::tickers::Entity",
        from = "Column::TickerId",
        to = "super::tickers::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Tickers,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Users,
}

impl Related<super::tickers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tickers.def()
    }
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Users.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
