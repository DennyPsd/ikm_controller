use crate::msges::data_change_list::DataChangeList;
use crate::msges::event_list::EventList;
use crate::msges::event_rule_create::{EventRuleCreate, EventRuleCreateArgs};
use crate::msges::event_rule_list::EventRuleList;
use crate::msges::event_rule_set::EventRuleSet;
use crate::msges::kmh_report_calc::KMHReportCalc;
use crate::msges::kmh_report_create::KMHReportCreate;
use crate::msges::kmh_report_list::KMHReportList;
use crate::msges::kmh_report_set::KMHReportSet;
use crate::msges::load_grad_table::LoadGradTable;
use crate::msges::park_list::ParkList;
use crate::msges::product_create::ProductCreate;
use crate::msges::product_delete::{ProductDelete, ProductDeleteArgs};
use crate::msges::product_list::ProductList;
use crate::msges::product_set::ProductSet;
use crate::msges::tank_config_set::TankConfigSet;
use crate::msges::tank_list::TankList;
use crate::msges::user_login::UserLogin;
use crate::msges::user_logout::UserLogout;
use taxon_core::prelude::*;
use taxon_core::components::ipc::IPCRole;
use taxon_core::utils::asyncapi::AsyncapiBuilder;

#[allow(dead_code)]
pub fn ikm_controller_client_api() -> AsyncapiBuilder {
  AsyncapiBuilder::new(IPCRole::Router)
    .operation::<LoadGradTable, ()>()
    .operation::<TankList, ()>()
    .operation::<TankConfigSet, ()>()
    .operation::<DataChangeList, ()>()
    .operation::<EventList, ()>()
    .operation::<EventRuleCreate, ()>()
    .operation::<EventRuleList, ()>()
    .operation::<EventRuleSet, ()>()
    .operation::<ParkList, ()>()
    .operation::<ProductList, ()>()
    .operation::<KMHReportList, ()>()
    .operation::<KMHReportSet, ()>()
    .operation::<KMHReportCreate, ()>()
    .operation::<KMHReportCalc, ()>()
    .operation::<UserLogin, ()>()
    .operation::<UserLogout, ()>()
    .operation::<ProductCreate, ()>()
    .operation::<ProductSet, ()>()
    .operation::<ProductDelete, ()>()
}
