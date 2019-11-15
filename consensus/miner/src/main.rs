use grpcio;
use std::{sync::Arc, env, pin::Pin};
use grpcio::{ChannelBuilder, EnvBuilder};
use proto::miner::{MineCtxRequest, MinedBlockRequest, MinerProxyClient, MineCtxResponse, MineCtx as MineCtxRpc};
use async_std::{
    task,
    stream::Stream,
    prelude::*,
    task::{Context, Poll},
};
use miner::types::MineCtx;

struct MineCtxStream {
    client: MinerProxyClient,
}

impl MineCtxStream {
    fn new(client: MinerProxyClient) -> Self {
        MineCtxStream {
            client
        }
    }
}

impl Stream for MineCtxStream {
    type Item = MineCtx;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.client.get_mine_ctx(&MineCtxRequest {}) {
            Ok(resp) => {
                if let Some(mine_ctx) = resp.mine_ctx {
                    let ctx = MineCtx { header: mine_ctx.header, nonce: mine_ctx.nonce };
                    Poll::Ready(Some(ctx))
                } else {
                    println!("pending");
                    Poll::Pending
                }
            }
            Err(e) => {
                Poll::Pending
            }
        }
    }
}

fn main() {
    let env = Arc::new(EnvBuilder::new().build());
    let ch = ChannelBuilder::new(env).connect("127.0.0.1:4251");
    let client = MinerProxyClient::new(ch);


    task::block_on(async {
        let mut ctx_stream = MineCtxStream::new(client.clone());
        while let Some(ctx) = ctx_stream.next().await {
            println!("the ctx is {:?}", ctx);
            let req = MinedBlockRequest {
                mine_ctx: Some(MineCtxRpc {
                    header: vec![2],
                    nonce: 0 as u64,
                }),
                proof: vec![1 as u8],
            };
            let resp = client.mined(&req);
            println!("mined{:?}", resp);
        }
    }
    );
}