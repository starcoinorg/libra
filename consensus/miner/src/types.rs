use futures03::{channel::oneshot};
use std::sync::{Arc, Mutex};
use cuckoo::Cuckoo;
use cuckoo::util::pow_input;
use byteorder::{ByteOrder, LittleEndian};

pub const MAX_EDGE: u8 = 6;
pub const CYCLE_LENGTH: usize = 8;

#[derive(PartialEq, Eq, Debug)]
pub struct MineCtx {
    pub nonce: u64,
    pub header: Vec<u8>,
}

#[derive(Clone)]
pub struct MineStateManager {
    inner: Arc<Mutex<StateInner>>,
    cuckoo: Cuckoo,
}

struct StateInner {
    mine_ctx: Option<MineCtx>,
    tx: Option<oneshot::Sender<Vec<u8>>>,
}

impl MineStateManager {
    pub fn new() -> Self {
        MineStateManager {
            inner: Arc::new(Mutex::new(StateInner {
                mine_ctx: None,
                tx: None,
            })),
            cuckoo: Cuckoo::new(MAX_EDGE, CYCLE_LENGTH),
        }
    }
}

pub trait MineState: Send + Sync {
    fn get_current_mine_ctx(&self) -> Option<MineCtx>;
    fn mine_accept(&self, mine_ctx: &MineCtx, proof: Vec<u8>) -> bool;
    fn mine_block(&mut self, mine_ctx: MineCtx) -> oneshot::Receiver<Vec<u8>>;
}

impl MineState for MineStateManager {
    fn get_current_mine_ctx(&self) -> Option<MineCtx> {
        let inner = self.inner.lock().unwrap();
        let ctx = inner.mine_ctx.as_ref()?;
        Some(MineCtx {
            header: ctx.header.clone(),
            nonce: ctx.nonce,
        })
    }
    fn mine_accept(&self, mine_ctx_req: &MineCtx, proof: Vec<u8>) -> bool {
        let mut x = self.inner.lock().unwrap();
        if let Some(mine_ctx) = &x.mine_ctx {
            if mine_ctx == mine_ctx_req {
                let input = pow_input(&mine_ctx.header, mine_ctx.nonce);
                let mut proof_u32 = vec![0u32; CYCLE_LENGTH];
                LittleEndian::read_u32_into(&proof, &mut proof_u32);
                if self.cuckoo.verify(&input, &proof_u32) !=true{
                    return false;
                }
            } else {
                return false;
            }
        } else { return false; }

        if let Some(tx) = x.tx.take() {
            tx.send(proof).unwrap();
            *x = StateInner {
                mine_ctx: None,
                tx: None,
            };
        } else { return false; }
        return true;
    }

    fn mine_block(&mut self, mine_ctx: MineCtx) -> oneshot::Receiver<Vec<u8>> {
        let mut x = self.inner.lock().unwrap();
        let (tx, rx) = oneshot::channel();
        *x = StateInner {
            mine_ctx: Some(mine_ctx),
            tx: Some(tx),
        };
        rx
    }
}