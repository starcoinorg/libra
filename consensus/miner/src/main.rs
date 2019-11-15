use grpcio;
use std::{sync::Arc, pin::Pin, thread};
use grpcio::{ChannelBuilder, EnvBuilder};
use proto::miner::{MineCtxRequest, MinedBlockRequest, MinerProxyClient, MineCtx as MineCtxRpc};
use async_std::{
    task,
    stream::Stream,
    prelude::*,
    task::{Context, Poll},
};
use miner::types::MineCtx;
use std::task::Waker;
use std::sync::Mutex;

struct MineCtxStream {
    client: MinerProxyClient,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl MineCtxStream {
    fn new(client: MinerProxyClient) -> Self {
        let waker: Arc<Mutex<Option<Waker>>> = Arc::new(Mutex::new(None));
        let task_waker = waker.clone();

        task::spawn(async move {
            loop {
                thread::sleep(std::time::Duration::from_secs(1));
                let mut inner_waker = task_waker.lock().unwrap();
                if let Some(waker) = inner_waker.take() {
                    waker.wake();
                }
            }
        });
        MineCtxStream {
            client,
            waker,
        }
    }
}

impl Stream for MineCtxStream {
    type Item = MineCtx;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut waker = self.waker.lock().unwrap();
        match self.client.get_mine_ctx(&MineCtxRequest {}) {
            Ok(resp) => {
                if let Some(mine_ctx) = resp.mine_ctx {
                    let ctx = MineCtx { header: mine_ctx.header, nonce: mine_ctx.nonce };
                    Poll::Ready(Some(ctx))
                } else {
                    *waker = Some(cx.waker().clone());
                    Poll::Pending
                }
            }
            Err(_e) => {
                *waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

struct MineClient {
    rpc_client: MinerProxyClient
}

impl MineClient {
    pub fn new(miner_server: String) -> Self {
        let env = Arc::new(EnvBuilder::new().build());
        let ch = ChannelBuilder::new(env).connect(&miner_server);
        let rpc_client = MinerProxyClient::new(ch);
        MineClient {
            rpc_client
        }
    }

    pub async fn start(&self) {
        let mut ctx_stream = MineCtxStream::new(self.rpc_client.clone());
        while let Some(ctx) = ctx_stream.next().await {
            println!("the ctx is {:?}", ctx);
            let req = MinedBlockRequest {
                mine_ctx: Some(MineCtxRpc {
                    header: vec![2],
                    nonce: 0 as u64,
                }),
                proof: vec![1 as u8],
            };
            let resp = self.rpc_client.mined(&req);
            println!("mined{:?}", resp);
        }
    }
}

fn main() {
    let miner = MineClient::new("127.0.0.1:4251".to_string());
    task::block_on(
        miner.start()
    );
}