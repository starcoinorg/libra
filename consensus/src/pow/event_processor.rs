use crate::chained_bft::consensusdb::ConsensusDB;
use crate::pow::chain_manager::ChainManager;
use crate::pow::chain_state_request_handle::ChainStateRequestHandle;
use crate::pow::mine_state::{BlockIndex, MineStateManager};
use crate::pow::mint_manager::MintManager;
use crate::pow::sync_manager::SyncManager;
use crate::state_replication::{StateComputer, TxnManager};
use anyhow::{format_err, Error, Result};
use channel;
use consensus_types::block_retrieval::{
    BlockRetrievalResponse, BlockRetrievalStatus, PowBlockRetrievalRequest,
};
use consensus_types::pow_sync_block::{PowSyncInfoReq, PowSyncInfoResp};
use consensus_types::{block::Block, payload_ext::BlockPayloadExt};
use futures::channel::mpsc;
use futures::{SinkExt, StreamExt, TryStreamExt};
use libra_crypto::hash::CryptoHash;
use libra_crypto::hash::PRE_GENESIS_BLOCK_ID;
use libra_crypto::HashValue;
use libra_logger::prelude::*;
use libra_prost_ext::MessageExt;
use libra_types::account_address::AccountAddress;
use libra_types::crypto_proxies::ValidatorVerifier;
use libra_types::transaction::SignedTransaction;
use libra_types::PeerId;
use miner::miner::verify;
use miner::types::{Algo, Solution, U256};
use network::validator_network::{ChainStateNetworkEvents, ChainStateNetworkSender};
use network::{
    proto::{
        Block as BlockProto, ConsensusMsg,
        ConsensusMsg_oneof::{self},
    },
    validator_network::{ConsensusNetworkEvents, ConsensusNetworkSender, Event},
};
use std::convert::TryFrom;
use std::convert::TryInto;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use storage_client::{StorageRead, StorageWrite};
use tokio::runtime::Handle;

pub struct EventProcessor {
    pub sync_manager: Arc<SyncManager>,
    pub mint_manager: Arc<MintManager>,
    pub block_cache_receiver: Option<mpsc::Receiver<Block<BlockPayloadExt>>>,
    pub inner: EventInner,
}

#[derive(Clone)]
pub struct EventInner {
    block_store: Arc<ConsensusDB>,
    network_sender: ConsensusNetworkSender,
    author: AccountAddress,
    self_sender: channel::Sender<Result<Event<ConsensusMsg>>>,
    pub chain_manager: Arc<ChainManager>,
    sync_signal_sender: mpsc::Sender<(PeerId, (u64, HashValue))>,
    sync_block_sender: mpsc::Sender<(PeerId, BlockRetrievalResponse<BlockPayloadExt>)>,
    block_cache_sender: mpsc::Sender<Block<BlockPayloadExt>>,
    dev_mode: bool,
    begin_mint_sender: mpsc::Sender<()>,
}

impl EventProcessor {
    pub fn new(
        network_sender: ConsensusNetworkSender,
        txn_manager: Arc<dyn TxnManager<Payload = Vec<SignedTransaction>>>,
        state_computer: Arc<dyn StateComputer<Payload = Vec<SignedTransaction>>>,
        author: AccountAddress,
        block_store: Arc<ConsensusDB>,
        rollback_flag: bool,
        mine_state: MineStateManager<BlockIndex>,
        read_storage: Arc<dyn StorageRead>,
        write_storage: Arc<dyn StorageWrite>,
        event_handle_sender: channel::Sender<Result<Event<ConsensusMsg>>>,
        sync_block_sender: mpsc::Sender<(PeerId, BlockRetrievalResponse<BlockPayloadExt>)>,
        sync_signal_sender: mpsc::Sender<(PeerId, (u64, HashValue))>,
        dump_path: PathBuf,
        new_block_sender: mpsc::Sender<u64>,
        dev_mode: bool,
        begin_mint_sender: mpsc::Sender<()>,
        is_first: bool,
    ) -> Self {
        let (block_cache_sender, block_cache_receiver) = mpsc::channel(1024);
        let chain_manager = Arc::new(ChainManager::new(
            Arc::clone(&block_store),
            txn_manager.clone(),
            state_computer.clone(),
            rollback_flag,
            author.clone(),
            read_storage,
            write_storage,
            dump_path,
            new_block_sender,
            is_first,
        ));

        let sync_manager = Arc::new(SyncManager::new(
            author.clone(),
            event_handle_sender.clone(),
            network_sender.clone(),
            block_cache_sender.clone(),
            chain_manager.clone(),
            dev_mode,
            begin_mint_sender.clone(),
        ));
        let mint_manager = Arc::new(MintManager::new(
            txn_manager.clone(),
            state_computer.clone(),
            network_sender.clone(),
            author.clone(),
            event_handle_sender.clone(),
            block_store.clone(),
            mine_state,
            dev_mode,
        ));

        let inner = EventInner {
            block_cache_sender,
            block_store,
            network_sender,
            self_sender: event_handle_sender,
            author,
            sync_block_sender,
            sync_signal_sender,
            chain_manager,
            dev_mode,
            begin_mint_sender,
        };

        EventProcessor {
            sync_manager,
            mint_manager,
            block_cache_receiver: Some(block_cache_receiver),
            inner,
        }
    }

    pub fn chain_state_handle(
        &self,
        executor: Handle,
        chain_state_network_sender: ChainStateNetworkSender,
        chain_state_network_events: ChainStateNetworkEvents,
        state_stop_receiver: mpsc::Receiver<()>,
    ) {
        let cs_req_handle = ChainStateRequestHandle::new(
            chain_state_network_sender,
            chain_state_network_events,
            self.inner.block_store.clone(),
        );
        executor.spawn(cs_req_handle.start(state_stop_receiver));
    }

    pub fn event_process(
        &self,
        executor: Handle,
        network_events: ConsensusNetworkEvents,
        mut event_handle_receiver: channel::Receiver<Result<Event<ConsensusMsg>>>,
    ) {
        let mut network_events = network_events.map_err(Into::<Error>::into);
        let event_inner = self.inner.clone();
        let fut = async move {
            loop {
                ::futures::select! {
                    event = network_events.select_next_some() => {
                        Self::handle_event(event, event_inner.clone()).await;
                    }
                    event = event_handle_receiver.select_next_some() => {
                        Self::handle_event(event, event_inner.clone()).await;
                    }
                }
            }
        };
        executor.spawn(fut);
    }

    pub async fn handle_event(event: Result<Event<ConsensusMsg>>, event_inner: EventInner) {
        match event {
            Ok(message) => {
                match message {
                    Event::Message((peer_id, msg)) => {
                        let msg_message = match msg.message {
                            Some(msg) => msg,
                            None => {
                                warn!("Unexpected msg from {}: {:?}", event_inner.author, msg);
                                return;
                            }
                        };
                        match msg_message {
                            ConsensusMsg_oneof::NewBlock(new_block) => {
                                if event_inner.chain_manager.is_run().await {
                                    let block: Block<BlockPayloadExt> =
                                        Block::try_from(new_block).expect("parse block pb err.");

                                    info!(
                                        "Self is {:?}, Peer Id is {:?}, Block Id is {:?}, height {}",
                                        event_inner.author,
                                        peer_id,
                                        block.id(),
                                        block.round()
                                    );

                                    //verify ledger info
                                    if verify_block_for_pow(&block, event_inner.dev_mode) {
                                        if event_inner.author != peer_id {
                                            if let Some((height, block_index)) = event_inner
                                                .chain_manager
                                                .chain_height_and_root()
                                                .await
                                            {
                                                debug!(
                                                    "Self is {:?}, height is {}, Peer Id is {:?}, Block Id is {:?}, height {}",
                                                    event_inner.author,
                                                    height,
                                                    peer_id,
                                                    block.id(),
                                                    block.round()
                                                );

                                                if height < block.round()
                                                    && block.parent_id() != block_index.id()
                                                {
                                                    if let Err(err) = event_inner
                                                        .sync_signal_sender
                                                        .clone()
                                                        .send((
                                                            peer_id,
                                                            (block.round(), HashValue::zero()),
                                                        ))
                                                        .await
                                                    {
                                                        error!("send sync signal err: {:?}", err);
                                                    }

                                                    //broadcast new block
                                                    let block_pb = TryInto::<BlockProto>::try_into(
                                                        block.clone(),
                                                    )
                                                    .expect("parse block err.");

                                                    // send block
                                                    let msg = ConsensusMsg {
                                                        message: Some(
                                                            ConsensusMsg_oneof::NewBlock(block_pb),
                                                        ),
                                                    };
                                                    Self::broadcast_consensus_msg_but(
                                                        &mut event_inner.network_sender.clone(),
                                                        false,
                                                        event_inner.author,
                                                        &mut event_inner.self_sender.clone(),
                                                        msg,
                                                        vec![peer_id],
                                                    )
                                                    .await;
                                                }
                                            }
                                        }

                                        if let Err(err) =
                                            event_inner.block_cache_sender.clone().send(block).await
                                        {
                                            error!("send new block err: {:?}", err);
                                        }
                                    } else {
                                        warn!(
                                            "block : {:?} from : {:?} verify fail.",
                                            block.id(),
                                            peer_id
                                        );
                                    }
                                }
                            }
                            ConsensusMsg_oneof::PowRequestBlock(req_block) => {
                                let block_req = PowBlockRetrievalRequest::try_from(req_block)
                                    .expect("parse err.");
                                info!("Sync block from {:?}, block_req : {:?}", peer_id, block_req);
                                if block_req.num_blocks() > 0 {
                                    let resp_block_msg = if block_req.asc() {
                                        Self::find_blocks_asc(
                                            event_inner.clone(),
                                            block_req.num_blocks(),
                                            block_req.height(),
                                            block_req.block_id(),
                                        )
                                        .await
                                    } else {
                                        Self::find_blocks_desc(
                                            event_inner.clone(),
                                            block_req.num_blocks(),
                                            block_req.block_id(),
                                        )
                                        .await
                                    };

                                    Self::send_consensus_msg(
                                        peer_id,
                                        &mut event_inner.network_sender.clone(),
                                        event_inner.author.clone(),
                                        &mut event_inner.self_sender.clone(),
                                        resp_block_msg,
                                    )
                                    .await;
                                }
                            }
                            ConsensusMsg_oneof::RespondBlock(resp_block) => {
                                let block_resp = BlockRetrievalResponse::try_from(resp_block)
                                    .expect("parse err.");
                                info!(
                                    "Sync block from {:?}, block_resp : {:?}, {:?}",
                                    peer_id,
                                    block_resp.status(),
                                    block_resp.blocks().len()
                                );
                                if let Err(err) = event_inner
                                    .sync_block_sender
                                    .clone()
                                    .send((peer_id, block_resp))
                                    .await
                                {
                                    error!("send sync block err: {:?}", err);
                                };
                                ()
                            }
                            _ => {
                                warn!("Unexpected msg from {}: {:?}", peer_id, msg_message);
                            }
                        };
                    }
                    Event::RpcRequest((peer_id, msg, callback)) => {
                        let msg_message = match msg.message {
                            Some(msg) => msg,
                            None => {
                                warn!("Unexpected ds msg from {}: {:?}", event_inner.author, msg);
                                return;
                            }
                        };
                        match msg_message {
                            ConsensusMsg_oneof::PowSyncInfoReq(sync_req) => {
                                let req = PowSyncInfoReq::try_from(sync_req).expect("parse err.");
                                let mut common_ancestor = None;
                                for pow_sync_block in req.latest_blocks() {
                                    if let Ok(Some(block_index)) = event_inner
                                        .block_store
                                        .query_block_index_by_height(pow_sync_block.height())
                                    {
                                        if block_index.id() == pow_sync_block.block_hash() {
                                            common_ancestor = Some(pow_sync_block);
                                            break;
                                        }
                                    }
                                }

                                let latest_height =
                                    event_inner.block_store.latest_height().unwrap();
                                let resp: PowSyncInfoResp =
                                    PowSyncInfoResp::new(latest_height, common_ancestor);

                                let resp_msg = ConsensusMsg {
                                    message: Some(ConsensusMsg_oneof::PowSyncInfoResp(
                                        resp.try_into().expect("into err."),
                                    )),
                                };

                                if let Err(err) = callback
                                    .send(Ok(resp_msg.to_bytes().expect("fail to serialize proto")))
                                    .map_err(|_| format_err!("handling inbound rpc call timed out"))
                                {
                                    error!("failed to PowSyncInfoRespProto resp, error: {:?}", err);
                                }
                            }
                            _ => {
                                warn!("Unexpected rpc msg from {}: {:?}", peer_id, msg_message);
                            }
                        }
                    }
                    Event::NewPeer(peer_id) => {
                        info!("Peer {:?} connected", peer_id);
                        if event_inner.chain_manager.is_init().await {
                            let (latest_height, latest_blocks) =
                                event_inner.chain_manager.reset_cache().await;
                            let req = PowSyncInfoReq::new_req(latest_blocks);
                            let resp = event_inner
                                .network_sender
                                .clone()
                                .sync_block_by_pow(
                                    peer_id,
                                    req.try_into().unwrap(),
                                    Duration::from_secs(10),
                                )
                                .await;
                            match resp {
                                Ok(latest_resp) => {
                                    let pow_resp =
                                        PowSyncInfoResp::try_from(latest_resp).expect("parse err.");
                                    info!("pow_resp: {:?}", pow_resp);
                                    if pow_resp.latest_height() > latest_height {
                                        if let Some(common_ancestor) = pow_resp.common_ancestor() {
                                            if event_inner.chain_manager.is_init().await {
                                                if let Err(err) = event_inner
                                                    .sync_signal_sender
                                                    .clone()
                                                    .send((
                                                        peer_id,
                                                        (
                                                            common_ancestor.height(),
                                                            common_ancestor.block_hash(),
                                                        ),
                                                    ))
                                                    .await
                                                {
                                                    error!("send sync signal err: {:?}", err);
                                                }
                                                event_inner.chain_manager.set_sync().await;
                                            }
                                        }
                                    } else {
                                        let _ =
                                            event_inner.begin_mint_sender.clone().send(()).await;
                                    }
                                }
                                Err(e) => {
                                    warn!("{:?}", e);
                                }
                            }
                        }
                    }
                    Event::LostPeer(peer_id) => {
                        info!("Peer {:?} disconnected", peer_id);
                    }
                }
            }
            Err(e) => {
                warn!("{:?}", e);
            }
        }
    }

    async fn find_blocks_asc(
        event_inner: EventInner,
        num_blocks: u64,
        height: u64,
        _block_id: HashValue,
    ) -> ConsensusMsg {
        let blocks: Vec<Block<BlockPayloadExt>> = event_inner
            .block_store
            .query_blocks_by_height(height + 1, num_blocks as usize)
            .unwrap();

        let status = if (blocks.len() as u64) == num_blocks {
            BlockRetrievalStatus::Succeeded
        } else {
            BlockRetrievalStatus::NotEnoughBlocks
        };

        let resp_block = BlockRetrievalResponse::new(status, blocks);
        ConsensusMsg {
            message: Some(ConsensusMsg_oneof::RespondBlock(
                resp_block.try_into().expect("into err."),
            )),
        }
    }

    async fn find_blocks_desc(
        event_inner: EventInner,
        num_blocks: u64,
        block_id: HashValue,
    ) -> ConsensusMsg {
        let mut blocks = vec![];
        let mut latest_block = if block_id != HashValue::zero() {
            Some(block_id)
        } else {
            None
        };
        let mut not_exist_flag = false;
        for _i in 0..num_blocks {
            let block = match latest_block {
                Some(child_hash) => {
                    if child_hash == *PRE_GENESIS_BLOCK_ID {
                        break;
                    }

                    let child = event_inner
                        .block_store
                        .get_block_by_hash::<BlockPayloadExt>(&child_hash);
                    match child {
                        Some(c) => c,
                        None => {
                            info!("not_exist_flag : {:?}", child_hash);
                            not_exist_flag = true;
                            break;
                        }
                    }
                }
                None => match event_inner.chain_manager.chain_root().await {
                    Some(tmp) => event_inner
                        .block_store
                        .get_block_by_hash::<BlockPayloadExt>(&tmp)
                        .expect("root not exist"),
                    None => {
                        info!("not_exist_flag : chain root is none.");
                        not_exist_flag = true;
                        break;
                    }
                },
            };

            latest_block = Some(block.parent_id());
            blocks.push(block.into());

            if latest_block.unwrap() == *PRE_GENESIS_BLOCK_ID {
                break;
            }
        }

        let status = if not_exist_flag {
            BlockRetrievalStatus::IdNotFound
        } else {
            if (blocks.len() as u64) == num_blocks {
                BlockRetrievalStatus::Succeeded
            } else {
                BlockRetrievalStatus::NotEnoughBlocks
            }
        };

        let resp_block = BlockRetrievalResponse::new(status, blocks);
        ConsensusMsg {
            message: Some(ConsensusMsg_oneof::RespondBlock(
                resp_block.try_into().expect("into err."),
            )),
        }
    }

    pub async fn broadcast_consensus_msg(
        network_sender: &mut ConsensusNetworkSender,
        self_flag: bool,
        self_peer_id: PeerId,
        self_sender: &mut channel::Sender<Result<Event<ConsensusMsg>>>,
        msg: ConsensusMsg,
    ) {
        Self::broadcast_consensus_msg_but(
            network_sender,
            self_flag,
            self_peer_id,
            self_sender,
            msg,
            vec![],
        )
        .await;
    }

    pub async fn broadcast_consensus_msg_but(
        network_sender: &mut ConsensusNetworkSender,
        self_flag: bool,
        self_peer_id: PeerId,
        self_sender: &mut channel::Sender<Result<Event<ConsensusMsg>>>,
        msg: ConsensusMsg,
        ignore_peers: Vec<PeerId>,
    ) {
        if self_flag {
            let event_msg = Ok(Event::Message((self_peer_id, msg.clone())));
            if let Err(err) = self_sender.send(event_msg).await {
                error!("Error delivering a self proposal: {:?}", err);
            }
        }
        let msg_raw = msg.to_bytes().unwrap();
        if let Err(err) = network_sender
            .broadcast_bytes(msg_raw.clone(), ignore_peers)
            .await
        {
            error!(
                "Error broadcasting proposal  error: {:?}, msg: {:?}",
                err, msg
            );
        }
    }

    pub async fn send_consensus_msg(
        send_peer_id: PeerId,
        network_sender: &mut ConsensusNetworkSender,
        self_peer_id: PeerId,
        self_sender: &mut channel::Sender<Result<Event<ConsensusMsg>>>,
        msg: ConsensusMsg,
    ) {
        if send_peer_id == self_peer_id {
            let event_msg = Ok(Event::Message((self_peer_id, msg.clone())));
            if let Err(err) = self_sender.send(event_msg).await {
                error!("Error delivering a self proposal: {:?}", err);
            }
        } else {
            if let Err(err) = network_sender.send_to(send_peer_id, msg.clone()).await {
                error!(
                    "Error broadcasting proposal to peer: {:?}, error: {:?}, msg: {:?}",
                    send_peer_id, err, msg
                );
            }
        }
    }
}

pub fn verify_block_for_pow(block: &Block<BlockPayloadExt>, dev_mode: bool) -> bool {
    if let Some(validators) = block.quorum_cert().certified_block().next_validator_set() {
        let miner = validators.payload()[0].clone();
        let validator_verifier = ValidatorVerifier::new_single(
            miner.account_address().clone(),
            miner.consensus_public_key().clone(),
        );
        match block.pow_validate_signatures(&validator_verifier) {
            Ok(_) => {
                let payload = block.payload().expect("payload is none");
                let target: U256 = U256::from_little_endian(&payload.target);
                let algo: &Algo = &payload.algo.into();
                let solution: Solution = payload.solve.clone().into();
                let header = block
                    .quorum_cert()
                    .ledger_info()
                    .ledger_info()
                    .hash()
                    .to_vec();
                return verify(&header, payload.nonce, solution, algo, &target, dev_mode);
            }
            _ => {}
        }
    }

    return false;
}
