use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "historicals_overrides")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub ticker: String,
    pub fiscal_year: i32,
    pub field_name: String,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub value: Decimal,
    pub note: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

pub type HistoricalsOverrides = Entity;
