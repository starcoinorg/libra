use crate::types::{MineState, MineStateManager, MineCtx};
use async_std::task;
use futures::Future;
use futures03::{channel::oneshot, compat::Future01CompatExt};
use grpcio::{self, Environment, RpcContext, ServerBuilder, UnarySink};
use proto::miner::{
    create_miner_proxy, MineCtxRequest, MineCtxResponse, MinedBlockRequest, MinedBlockResponse,
    MinerProxy, MineCtx as MineCtxRpc,
};
use std::{
    sync::Arc,
    io::{self, Read},
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
        for i in 0..100 as u64 {
            let (rx, tx) = mine_state.mine_block(MineCtx {
                header: vec![2; 32],
                nonce: i,
            });
            let proof = rx.recv().await.unwrap();
            println!("mined success");
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
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