use futures03::{channel::oneshot};
use std::sync::{Arc, Mutex};

#[derive(PartialEq, Eq, Debug)]
pub struct MineCtx {
    pub nonce: u64,
    pub header: Vec<u8>,
}

#[derive(Clone)]
pub struct MineStateManager {
    inner: Arc<Mutex<StateInner>>,
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
        }
    }
}

pub trait MineState: Send + Sync {
    fn get_current_mine_ctx(&self) -> Option<MineCtx>;
    fn mine_accept(&self, mine_ctx: &MineCtx, proof: Vec<u8>) -> bool;
    fn set_mine_ctx(&mut self, mine_ctx: MineCtx) -> oneshot::Receiver<Vec<u8>>;
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
                // todo: verify proof
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


    fn set_mine_ctx(&mut self, mine_ctx: MineCtx) -> oneshot::Receiver<Vec<u8>> {
        let mut x = self.inner.lock().unwrap();
        let (tx, rx) = oneshot::channel();
        *x = StateInner {
            mine_ctx: Some(mine_ctx),
            tx: Some(tx),
        };
        rx
    }
}