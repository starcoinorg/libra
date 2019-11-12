use async_std::task;
use futures::Future;
use futures03::{channel::oneshot, compat::Future01CompatExt};
use grpcio::{self, Environment, RpcContext, ServerBuilder, UnarySink};
use proto::miner::{
    create_miner_proxy, MineCtxRequest, MineCtxResponse, MinedBlockRequest, MinedBlockResponse,
    MinerProxy,
};
use std::sync::Mutex;
use std::{
    io::{self, Read},
    sync::Arc,
};

#[derive(Clone)]
pub struct MinerProxyServer<S>
    where
        S: MineState + Clone + Send + Clone + 'static,
{
    miner_proxy_inner: Arc<MinerProxyServerInner<S>>,
}

struct MinerProxyServerInner<S>
    where
        S: MineState + Clone + Send + Clone + 'static,
{
    state: S,
}

pub trait MineState: Send + Sync {
    fn get_current_mine_ctx(&self) -> MineCtx;
    fn drop_miner_state(&self, mine_ctx: &MineCtx) -> bool;
    fn mark_accept(&self);
}


#[derive(PartialEq, Eq)]
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
    tx: Option<oneshot::Sender<()>>,
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

    pub fn set_mine_ctx(&mut self, mine_ctx: MineCtx) -> oneshot::Receiver<()> {
        let mut x = self.inner.lock().unwrap();
        let (tx, rx) = oneshot::channel();
        *x = StateInner {
            mine_ctx: Some(mine_ctx),
            tx: Some(tx),
        };
        rx
    }
}

impl MineState for MineStateManager {
    fn get_current_mine_ctx(&self) -> MineCtx {
        let inner = self.inner.lock().unwrap();
        let ctx = inner.mine_ctx.as_ref().unwrap();
        MineCtx {
            header: ctx.header.clone(),
            nonce: ctx.nonce,
        }
    }

    fn drop_miner_state(&self, mine_ctx: &MineCtx) -> bool {
        return true;
        let mut x = self.inner.lock().unwrap();
        match &x.mine_ctx {
            None => false,
            Some(mine_ctx_inner) => {
                if mine_ctx_inner == mine_ctx {
                    *x = StateInner {
                        mine_ctx: None,
                        tx: None,
                    };
                    true
                } else {
                    false
                }
            }
        }
    }

    fn mark_accept(&self) {
        let mut x = self.inner.lock().unwrap();
        if let Some(tx) = x.tx.take() {
            tx.send(()).unwrap();
        }
    }
}

impl<S: MineState + Clone + Send + Clone + 'static> MinerProxy for MinerProxyServer<S> {
    fn get_mine_ctx(
        &mut self,
        ctx: RpcContext,
        req: MineCtxRequest,
        sink: UnarySink<MineCtxResponse>,
    ) {
        let mine_ctx = self.miner_proxy_inner.state.get_current_mine_ctx();
        let resp = MineCtxResponse {
            nonce: mine_ctx.nonce,
            header: mine_ctx.header,
        };
        let fut = sink
            .success(resp)
            .map_err(|e| eprintln!("Failed to response to get_mine_ctx {}", e));
        ctx.spawn(fut);
    }

    fn mined(
        &mut self,
        ctx: RpcContext,
        req: MinedBlockRequest,
        sink: UnarySink<MinedBlockResponse>,
    ) {
        let mut accept = false;
        if let Some(mine_req) = req.mine_ctx {
            let mine_ctx = MineCtx {
                nonce: mine_req.nonce,
                header: mine_req.header,
            };
            if self.miner_proxy_inner.state.drop_miner_state(&mine_ctx) == true {
                self.miner_proxy_inner.state.mark_accept();
                accept = true;
            }
        }

        let resp = MinedBlockResponse { accept };
        let fut = sink.success(resp).map_err(|e| eprintln!("Failed to response to mined {}", e));
        ctx.spawn(fut);
    }
}

pub fn setup_minerproxy_service<S>(mine_state: S) -> grpcio::Server
    where
        S: MineState + Clone + Send + Sync + 'static,
{
    let env = Arc::new(Environment::new(1));
    let miner_proxy_srv = MinerProxyServer {
        miner_proxy_inner: Arc::new(MinerProxyServerInner { state: mine_state }),
    };
    let service = create_miner_proxy(miner_proxy_srv);
    let server = ServerBuilder::new(env)
        .register_service(service)
        .bind("127.0.0.1", 4251)
        .build()
        .unwrap();
    server
}

pub fn run_service() {
    let mut mine_state = MineStateManager::new();
    let mut grpc_srv = setup_minerproxy_service(mine_state.clone());
    grpc_srv.start();
    for &(ref host, port) in grpc_srv.bind_addrs() {
        println!("listening on {}:{}", host, port);
    }
    task::spawn(async move {
        let tx = mine_state.set_mine_ctx(MineCtx {
            header: vec![2],
            nonce: 0 as u64,
        });
        tx.await.unwrap();
    });
    let (tx, rx) = oneshot::channel();

    task::spawn(async {
        println!("Press enter to exit...");
        let _ = io::stdin().read(&mut [0]).unwrap();
        let _ = tx.send(());
    });

    task::block_on(async move {
        rx.await.unwrap();
        grpc_srv.shutdown().compat().await.unwrap();
    });
}

fn main() {
    run_service();
}
