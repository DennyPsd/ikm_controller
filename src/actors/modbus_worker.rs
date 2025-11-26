// modbus_worker.rs (исправлённый)
use crate::actors::modbus_types::{ModbusReply, ModbusTimings};
use crate::actors::modbus_worker_job::{ModbusPortJob, ModbusPortJobArgs, send_and_read};
use ractor::{Actor, ActorProcessingErr, ActorRef};
use std::collections::HashMap;
use linked_hash_set::LinkedHashSet;
use tokio_serial::SerialStream;
use tracing::{error, info};

#[derive(Debug)]
pub struct Queues {
    pub queue: LinkedHashSet<Vec<u8>>,
    pub res_storage: HashMap<Vec<u8>, Result<Vec<u8>, String>>,
}

impl Queues {
    pub fn new() -> Self {
        Self {
            queue: LinkedHashSet::new(),
            res_storage: HashMap::new(),
        }
    }

    pub fn q_head(&self) -> Option<&Vec<u8>> {
        self.queue.iter().next()
    }

    pub fn q_remove(&mut self, cmd: &[u8]) -> bool {
        self.queue.remove(cmd)
    }

    pub fn q_len(&self) -> usize {
        self.queue.len()
    }

    pub fn res_put(&mut self, cmd: Vec<u8>, data: Result<Vec<u8>, String>) {
        self.res_storage.insert(cmd, data);
    }
}

#[derive(Debug)]
pub struct ModbusWorkerState {
    pub stream: Option<SerialStream>,
    pub queues: Queues,
    pub busy: bool,
    pub blocked: bool,
    pub timings: ModbusTimings,
}

#[derive(Debug)]
pub enum ModbusWorkerMsg {
    /// cmd, reply_to (ActorRef that expects ModbusReplyMsg)
    SendToDevice {
        cmd: Vec<u8>,
        reply_to: ActorRef<ModbusReplyMsg>,
    },

    ScanBatch {
        cmds: Vec<Vec<u8>>,
        reply_to: ActorRef<ModbusReplyBatchMsg>,
    },

    Process,

    ProcessFinished {
        cmd: Vec<u8>,
        result: Result<Vec<u8>, String>,
        port: SerialStream,
    },

    Block,
    Unblock,
    Stop,
}

#[derive(Debug)]
pub enum ModbusWorkerReplyMsg {
    /// For single-command reply
    Reply(ModbusReply),
}
pub type ModbusReplyMsg = ModbusWorkerReplyMsg;

#[derive(Debug)]
pub enum ModbusWorkerReplyBatchMsg {
    ReplyBatch(Vec<(Vec<u8>, Vec<u8>)>),
}
pub type ModbusReplyBatchMsg = ModbusWorkerReplyBatchMsg;

pub struct ModbusWorker;

impl ModbusWorker {
    pub fn new() -> Self {
        Self
    }
}

#[ractor::async_trait]
impl Actor for ModbusWorker {
    type Msg = ModbusWorkerMsg;
    type State = ModbusWorkerState;
    type Arguments = (ModbusTimings, SerialStream);

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        (timings, stream): Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        info!("ModbusWorker started (stream available)");
        Ok(ModbusWorkerState {
            stream: Some(stream),
            queues: Queues::new(),
            busy: false,
            blocked: false,
            timings,
        })
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            ModbusWorkerMsg::SendToDevice { cmd, reply_to } => {
                if state.blocked {
                    let _ = reply_to.cast(ModbusWorkerReplyMsg::Reply(ModbusReply::IoBlocked));
                    return Ok(());
                }

                // already have stored response
                if let Some(stored) = state.queues.res_storage.remove(&cmd) {
                    match stored {
                        Ok(data) => {
                            let _ = reply_to.cast(ModbusWorkerReplyMsg::Reply(ModbusReply::Ok(data)));
                        }
                        Err(err) => {
                            let _ = reply_to.cast(ModbusWorkerReplyMsg::Reply(ModbusReply::Err(err)));
                        }
                    }
                    return Ok(());
                }

                // already queued
                if state.queues.queue.contains(&cmd) {
                    let _ = reply_to.cast(ModbusWorkerReplyMsg::Reply(ModbusReply::InProgress));
                    return Ok(());
                }

                // enqueue
                state.queues.queue.insert(cmd);
                if !state.busy {
                    state.busy = true;
                    let _ = myself.cast(ModbusWorkerMsg::Process);
                }
                let _ = reply_to.cast(ModbusWorkerReplyMsg::Reply(ModbusReply::InProgress));
            }

            ModbusWorkerMsg::ScanBatch { cmds, reply_to } => {
                if state.stream.is_none() {
                    let _ = reply_to.cast(ModbusReplyBatchMsg::ReplyBatch(vec![]));
                    return Ok(());
                }

                // take stream (move out), process commands inline, then put stream back
                let mut port = match state.stream.take() {
                    Some(p) => p,
                    None => {
                        let _ = reply_to.cast(ModbusReplyBatchMsg::ReplyBatch(vec![]));
                        return Ok(());
                    }
                };

                let t = state.timings;
                let mut out: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();

                for cmd in cmds.into_iter() {
                    // выполняем send_and_read синхронно (в текущем акторе)
                    let res = send_and_read(&mut port, &cmd, t.first_byte_timeout, t.per_byte_timeout).await;
                    match res {
                        Ok(data) => {
                            if !data.is_empty() {
                                out.push((cmd.clone(), data));
                            }
                        }
                        Err(err) => {
                            // в случае ошибки можно логировать и продолжать
                            error!("ScanBatch: cmd {:?} failed: {}", cmd, err);
                        }
                    }
                }

                // вернём порт в состояние
                state.stream = Some(port);
                let _ = reply_to.cast(ModbusReplyBatchMsg::ReplyBatch(out));
            }

            ModbusWorkerMsg::Process => {
                if state.blocked || state.stream.is_none() {
                    return Ok(());
                }

                if let Some(cmd) = state.queues.q_head() {
                    let stream = match state.stream.take() {
                        Some(p) => p,
                        None => {
                            state.busy = false;
                            return Ok(());
                        }
                    };

                    let args = ModbusPortJobArgs {
                        worker: myself.clone(),
                        cmd: cmd.clone(),
                        timings: state.timings,
                        stream,
                    };

                    // spawn job actor which will return via ProcessFinished
                    ractor::Actor::spawn(None, ModbusPortJob::new(), args).await.map_err(|e| {
                        error!("failed spawn job: {:?}", e);
                        ActorProcessingErr::from("spawn job")
                    })?;
                } else {
                    state.busy = false;
                }
            }

            ModbusWorkerMsg::ProcessFinished { cmd, result, port } => {
                let _ = state.queues.q_remove(&cmd);
                state.queues.res_put(cmd, result.clone());
                state.stream = Some(port);

                if state.queues.q_len() > 0 && !state.blocked {
                    let _ = myself.cast(ModbusWorkerMsg::Process);
                } else {
                    state.busy = false;
                }
            }

            ModbusWorkerMsg::Block => {
                state.blocked = true;
            }

            ModbusWorkerMsg::Unblock => {
                state.blocked = false;
                if state.queues.q_len() > 0 && !state.busy {
                    state.busy = true;
                    let _ = myself.cast(ModbusWorkerMsg::Process);
                }
            }

            ModbusWorkerMsg::Stop => {
                let _ = myself.stop(None);
            }
        }

        Ok(())
    }
}
