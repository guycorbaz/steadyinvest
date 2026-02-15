//! `SeaORM` Entity for `comparison_set_items` table.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "comparison_set_items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub comparison_set_id: i32,
    pub analysis_snapshot_id: i32,
    pub sort_order: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::comparison_sets::Entity",
        from = "Column::ComparisonSetId",
        to = "super::comparison_sets::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    ComparisonSets,
    #[sea_orm(
        belongs_to = "super::analysis_snapshots::Entity",
        from = "Column::AnalysisSnapshotId",
        to = "super::analysis_snapshots::Column::Id",
        on_update = "NoAction",
        on_delete = "Restrict"
    )]
    AnalysisSnapshots,
}

impl Related<super::comparison_sets::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ComparisonSets.def()
    }
}

impl Related<super::analysis_snapshots::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AnalysisSnapshots.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
