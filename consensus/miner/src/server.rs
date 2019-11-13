use async_std::task;
use futures::Future;
use futures03::{channel::oneshot, compat::Future01CompatExt};
use grpcio::{self, Environment, RpcContext, ServerBuilder, UnarySink};
use proto::miner::{
    create_miner_proxy, MineCtxRequest, MineCtxResponse, MinedBlockRequest, MinedBlockResponse,
    MinerProxy, MineCtx as MineCtxRpc,
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

impl<S: MineState + Clone + Send + Clone + 'static> MinerProxy for MinerProxyServer<S> {
    fn get_mine_ctx(
        &mut self,
        ctx: RpcContext,
        _req: MineCtxRequest,
        sink: UnarySink<MineCtxResponse>,
    ) {
        let mine_ctx_rpc = if let Some(mine_ctx) = self.miner_proxy_inner.state.get_current_mine_ctx() {
            Some(MineCtxRpc {
                nonce: mine_ctx.nonce,
                header: mine_ctx.header,
            })
        } else {
            None
        };
        let resp = MineCtxResponse {
            mine_ctx: mine_ctx_rpc
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
            let proof = req.proof;
            accept = self.miner_proxy_inner.state.mine_accept(&mine_ctx, proof);
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
        let proof = tx.await.unwrap();
        println!("mined success proof:{:?}", proof);
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