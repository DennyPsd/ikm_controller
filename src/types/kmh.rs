use crate::types::tanks::Tank;
use chrono::{DateTime, Local};
use ikm_calc::calculation::kmh::KMHReport;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use taxon_core::infrastructure::facility::{DataLink, SharedData};
use uuid::Uuid;

#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub enum KMHReportStatus {
  #[default]
  FullfilRequired,
  Positive,
  Negative,
}
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct KMHReportInstance {
  pub id: Uuid,
  pub title: Option<SmolStr>,
  pub description: Option<SmolStr>,
  /** Кем создана запись */
  pub created_by: SmolStr,
  /** Кем изменена запись */
  pub updated_by: Option<SmolStr>,
  /** Дата создания записи */
  pub created_at: DateTime<Local>,
  /** Дата изменения записи */
  pub updated_at: Option<DateTime<Local>>,
  /** Идентификатор цистерны */
  pub tank: DataLink<Tank>,
  pub software_name: Option<SmolStr>,
  pub software_version: Option<SmolStr>,
  /** Статус отчета */
  pub status: KMHReportStatus,
  pub data: KMHReport,
}

impl SharedData for KMHReportInstance {
  fn id(&self) -> &Uuid {
    &self.id
  }

  fn title(&self) -> Option<&SmolStr> {
    self.title.as_ref()
  }

  fn created_by(&self) -> Option<&SmolStr> {
    Some(&self.created_by)
  }

  fn created_at(&self) -> Option<&DateTime<Local>> {
    Some(&self.created_at)
  }

  fn updated_by(&self) -> Option<&SmolStr> {
    self.updated_by.as_ref()
  }

  fn updated_at(&self) -> Option<&DateTime<Local>> {
    self.updated_at.as_ref()
  }
}

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
#[allow(dead_code)]
pub struct KMHGraduationTable {
  pub id: Uuid,
  pub data: Vec<Vec<f64>>,
}

impl SharedData for KMHGraduationTable {
  fn id(&self) -> &Uuid {
    &self.id
  }
}
