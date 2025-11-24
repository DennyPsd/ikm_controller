use ractor::rpc::CallResult;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde::{Deserialize, Serialize};
use serde_json::json;
use smol_str::SmolStr;
use std::time::{Duration, Instant};
use taxon_core::actors::ipc::errors::IPCError;
use taxon_core::infrastructure::device::FacilityDevice;
use taxon_core::prelude::{IPCActionKind, IPCActorMsg, IPCMessageCrate};
use tokio::time::sleep;
use tracing::{error, info};

use crate::actors::hart::hart_fabric::HartFabricMsg;
use crate::actors::hart::protocol::types::PortReply;
use crate::actors::hart::protocol::utils::checksum;

use crate::http::v3::transfer::{decode_addr, push_optional_data};

pub struct IpcHandler;

#[derive(Debug)]
pub struct IpcHandlerState {
    pub hart_fabric: ActorRef<HartFabricMsg>,
    #[allow(dead_code)]
    pub ipc_router: ActorRef<Option<IPCActorMsg>>,
}

#[derive(Debug)]
pub enum IpcHandlerMsg {
    Ipc(IPCMessageCrate),
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SendToDeviceArgs {
    #[serde(default)]
    pub command: u8,
    #[serde(default)]
    pub data: Option<SmolStr>, // base64 payload (optional)
}

#[ractor::async_trait]
impl Actor for IpcHandler {
    type Msg = IpcHandlerMsg;
    type State = IpcHandlerState;
    type Arguments = IpcHandlerState;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ractor::ActorProcessingErr> {
        Ok(args)
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        info!("ikm_controller: received message");
        match msg {
            IpcHandlerMsg::Ipc(ipc_msg) => {
                println!("*Обработка сообщения по IPC*");
                Ok(())
            }
        }
    }
}
