use crate::chained_bft::consensusdb::ConsensusDB;
use crate::pow::block_tree::{BlockTree, CommitData};
use crate::state_replication::{StateComputer, TxnManager};
use consensus_types::{block::Block, payload_ext::BlockPayloadExt};
use futures::compat::Future01CompatExt;
use futures::{channel::mpsc, SinkExt, StreamExt};
use futures_locks::{Mutex, RwLock};
use itertools;
use libra_crypto::HashValue;
use libra_logger::prelude::*;
use libra_types::account_address::AccountAddress;
use libra_types::block_index::BlockIndex;
use libra_types::block_metadata::BlockMetadata;
use libra_types::transaction::TransactionStatus;
use libra_types::transaction::TransactionToCommit;
use libra_types::transaction::{SignedTransaction, Transaction};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use storage_client::{StorageRead, StorageWrite};
use tokio::runtime::Handle;
use atomic_refcell::AtomicRefCell;

pub struct ChainManager {
    inner: AtomicRefCell<ChainInner>,
}

#[derive(Clone)]
struct ChainInner {
    block_store: Arc<ConsensusDB>,
    state_computer: Arc<dyn StateComputer<Payload = Vec<SignedTransaction>>>,
    block_tree: Arc<RwLock<BlockTree>>,
    orphan_blocks: Arc<Mutex<HashMap<HashValue, Vec<HashValue>>>>, //key -> parent_block_id, value -> block_id
    author: AccountAddress,
    read_storage: Arc<dyn StorageRead>,
    new_block_sender: mpsc::Sender<u64>,
    chain_state: Arc<RwLock<ChainState>>,
    is_first: bool,
}

impl ChainInner {
    async fn set_run(&mut self) {
        self.chain_state.write().compat().await.unwrap().set_run()
    }

    pub async fn set_sync(&mut self) {
        self.chain_state.write().compat().await.unwrap().set_sync()
    }

    pub async fn is_init(&self) -> bool {
        self.chain_state.read().compat().await.unwrap().is_init()
    }

    pub async fn is_run(&self) -> bool {
        self.chain_state.read().compat().await.unwrap().is_run()
    }
}

#[derive(Clone)]
struct ChainState {
    state: State
}

impl ChainState {

    fn new(state: State) -> Self {
        ChainState {
            state
        }
    }

    fn set_run(&mut self) {
        self.state = State::RUNNING;
    }

    pub fn set_sync(&mut self) {
        self.state = State::SYNCING;
    }

    pub fn is_init(&self) -> bool {
        self.state.clone() == State::INIT
    }

    pub fn is_run(&self) -> bool {
        println!("---is_run---0000---{:?}------", self.state.clone());
        println!("---is_run---1111---{:?}------", (self.state.clone() == State::RUNNING));
        self.state.clone() == State::RUNNING
    }
}


#[derive(Clone, PartialEq, Debug)]
enum State {
    INIT,
    SYNCING,
    RUNNING,
}

impl ChainManager {
    pub fn new(
        block_store: Arc<ConsensusDB>,
        txn_manager: Arc<dyn TxnManager<Payload = Vec<SignedTransaction>>>,
        state_computer: Arc<dyn StateComputer<Payload = Vec<SignedTransaction>>>,
        rollback_mode: bool,
        author: AccountAddress,
        read_storage: Arc<dyn StorageRead>,
        write_storage: Arc<dyn StorageWrite>,
        dump_path: PathBuf,
        new_block_sender: mpsc::Sender<u64>,
        is_first: bool,
    ) -> Self {
        //orphan block
        let orphan_blocks = Arc::new(Mutex::new(HashMap::new()));

        //block tree
        let block_tree = Arc::new(RwLock::new(BlockTree::new::<BlockPayloadExt>(
            write_storage,
            txn_manager,
            rollback_mode,
            Arc::clone(&block_store),
            dump_path,
        )));

        let inner = ChainInner {
            block_store,
            state_computer,
            block_tree,
            orphan_blocks,
            author,
            read_storage,
            new_block_sender,
            chain_state: if is_first{Arc::new(RwLock::new(ChainState::new(State::RUNNING)))} else {
                Arc::new(RwLock::new(ChainState::new(State::INIT)))},
            is_first
        };

        ChainManager { inner: AtomicRefCell::new(inner) }
    }

    pub fn _process_orphan_blocks(&self) {
        //TODO:orphan
    }

    pub async fn set_sync(&self) {
        self.inner.borrow_mut().set_sync().await;
    }

    pub async fn is_run(&self) -> bool {
        self.inner.borrow().is_run().await
    }

    pub async fn is_init(&self) -> bool {
        self.inner.borrow().is_init().await
    }

    pub fn save_block(
        &self,
        mut block_cache_receiver: mpsc::Receiver<Block<BlockPayloadExt>>,
        executor: Handle,
        mut chain_stop_receiver: mpsc::Receiver<()>,
        mut begin_mint_receiver: mpsc::Receiver<()>,
    ) {
        let chain_inner = self.inner.clone();
        let mut new_block_sender = self.inner.borrow().new_block_sender.clone();
        let chain_fut = async move {
            if chain_inner.borrow().is_first {
                new_block_sender
                    .send(0)
                    .await
                    .expect("new_block_sender send msg err.");
            }
            loop {
                ::futures::select! {
                                _ = begin_mint_receiver.select_next_some() => {
                                    println!("--------begin_mint_receiver----0000-----");
                                    if !chain_inner.borrow().is_run().await {
                                        println!("--------begin_mint_receiver----1111-----");
                                        chain_inner.borrow_mut().set_run().await;
                                        println!("--------begin_mint_receiver----2222-----");

                                        new_block_sender
                                        .send(0)
                                        .await
                                        .expect("new_block_sender send msg err.");
                                    }
                                }
                                block = block_cache_receiver.select_next_some() => {
                                    let mut payload = match block.payload() {
                                        Some(p) => p.get_txns(),
                                        None => vec![],
                                    };

                                    // Pre compute
                                    // 1. orphan block
                                    let parent_block_id = block.parent_id();
                                    let block_index = BlockIndex::new(&block.id(), &parent_block_id);
                                    let mut chain_lock = chain_inner.borrow().block_tree.write().compat().await.unwrap();
                                    if chain_lock.block_exist(&parent_block_id) && !chain_lock.block_exist(&block.id()) {
                                        // 2. find ancestors
                                        let (ancestors, pre_block_index) = chain_lock.find_ancestor_until_main_chain(&parent_block_id).expect("find ancestors err.");
                                        // 3. find blocks
                                        let blocks = chain_inner.borrow().block_store.get_blocks_by_hashs::<BlockPayloadExt>(ancestors).expect("find blocks err.");

                                        let mut commit_txn_vec = Vec::<(BlockMetadata, Vec<SignedTransaction>)>::new();
                                        for b in blocks {
                                            let mut tmp_txns = match b.payload() {
                                                Some(t) => t.get_txns(),
                                                None => vec![],
                                            };

                                            let miner_address = b.quorum_cert().commit_info().next_validator_set().expect("validator_set err.").payload().clone()[0].account_address();
                                            let block_meta_data = BlockMetadata::new(b.parent_id().clone(), b.timestamp_usecs(), BTreeMap::new(), miner_address.clone());
                                            commit_txn_vec.push((block_meta_data, tmp_txns));
                                        }

                                        let pre_compute_grandpa_block_id = pre_block_index.parent_id();
                                        let pre_compute_parent_block_id = pre_block_index.id();
                                        let miner_address = block.quorum_cert().commit_info().next_validator_set().expect("validator_set err.").payload().clone()[0].account_address();
                                        let block_meta_data = BlockMetadata::new(parent_block_id.clone(), block.timestamp_usecs(), BTreeMap::new(), miner_address.clone());
                                        commit_txn_vec.push((block_meta_data.clone(), payload.clone()));

                                        // 4. call pre_compute
                                        match chain_inner.borrow().state_computer.compute_by_hash(&pre_compute_grandpa_block_id, &parent_block_id, &block.id(), commit_txn_vec).await {
                                            Ok(processed_vm_output) => {
                                                let executed_trees = processed_vm_output.executed_trees();
                                                let state_id = executed_trees.state_root();
                                                let txn_accumulator_hash = executed_trees.txn_accumulator().root_hash();
                                                let txn_len = executed_trees.version().expect("version err.");

                                                if txn_accumulator_hash == block.quorum_cert().ledger_info().ledger_info().transaction_accumulator_hash() && state_id == block.quorum_cert().ledger_info().ledger_info().consensus_data_hash() {

                                                let mut txn_vec = vec![Transaction::BlockMetadata(block_meta_data)];
                                                txn_vec.extend(
                                                    payload
                                                        .iter()
                                                        .map(|txn| Transaction::UserTransaction(txn.clone())),
                                                );
                                                let len = txn_vec.len();
                                                let mut txn_data_list = vec![];
                                                let total_len = processed_vm_output.transaction_data().len();

                                                for i in 0..len {
                                                    txn_data_list.push(processed_vm_output.transaction_data()[total_len - len + i].clone());
                                                }

                                                let mut txns_to_commit = vec![];
                                                for (txn, txn_data) in itertools::zip_eq(txn_vec, txn_data_list) {
                                                    if let TransactionStatus::Keep(_) = txn_data.status() {
                                                        txns_to_commit.push(TransactionToCommit::new(
                                                            txn,
                                                            txn_data.account_blobs().clone(),
                                                            txn_data.events().to_vec(),
                                                            txn_data.gas_used(),
                                                            txn_data.status().vm_status().major_status,
                                                        ));
                                                    }
                                                }
                                                let commit_len = txns_to_commit.len();
                                                if (block.quorum_cert().ledger_info().ledger_info().commit_info().version() == txn_len) {
                                                    let commit_data = CommitData {txns_to_commit,
                                                        first_version: (txn_len - (commit_len as u64) + 1) as u64,
                                                        ledger_info_with_sigs: Some(block.quorum_cert().ledger_info().clone())};

                                                    let (new_root, latest_height) = chain_lock.add_block_info(block, &parent_block_id, processed_vm_output, commit_data).await.expect("add_block_info failed.");
                                                    if new_root {
                                                        if chain_inner.borrow().is_run().await {
                                                            let _ = new_block_sender.send(latest_height).await;
                                                        }

                                                        chain_lock.print_block_chain_root(chain_inner.borrow().author);
                                                    }
                                                } else {
                                                    warn!("Peer id {:?}, Drop block {:?}, block version is {}, vm output version is {}", chain_inner.borrow().author, block.id(),
                                                    block.quorum_cert().ledger_info().ledger_info().commit_info().version(), txn_len);
                                                }
                                            } else {
                                                warn!("Peer id {:?}, Drop block {:?}, parent_block_id {:?}, grandpa_block_id {:?}", chain_inner.borrow().author, block.id(), parent_block_id, pre_compute_grandpa_block_id);
                                            }
                                        }
                                        Err(e) => {error!("error: {:?}", e)},
                                    }
                                } else {
                                    //save orphan block
                //                    let mut write_lock = chain_inner.borrow().orphan_blocks.lock().compat().await.unwrap();
                //                    write_lock.insert(block_index.parent_id(), vec![block_index.id()]);
                                }
                                    }
                                    _ = chain_stop_receiver.select_next_some() => {
                                        break;
                                    }
                                    complete => {
                                       break;
                                   }
                                }
            }
        };
        executor.spawn(chain_fut);
    }

    pub async fn chain_root(&self) -> Option<HashValue> {
        self.inner
            .borrow()
            .block_tree
            .clone()
            .read()
            .compat()
            .await
            .unwrap()
            .root_hash()
    }

    pub async fn block_exist(&self, block_hash: &HashValue) -> bool {
        self.inner
            .borrow()
            .block_tree
            .clone()
            .read()
            .compat()
            .await
            .unwrap()
            .block_exist(block_hash)
    }

    pub async fn chain_height_and_root(&self) -> Option<(u64, BlockIndex)> {
        self.inner
            .borrow()
            .block_tree
            .clone()
            .read()
            .compat()
            .await
            .unwrap()
            .chain_height_and_root()
    }

    pub async fn reset_cache(&self) -> (u64, Vec<(u64, HashValue)>) {
        self.inner
            .borrow()
            .block_tree
            .clone()
            .write()
            .compat()
            .await
            .unwrap()
            .reset_cache()
    }
}
