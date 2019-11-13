use grpcio;

use std::env;
use std::sync::Arc;

use grpcio::{ChannelBuilder, EnvBuilder};
use proto::miner::{MineCtxRequest, MinedBlockRequest, MinerProxyClient, MineCtxResponse, MineCtx as MineCtxRpc};

fn main() {
    let env = Arc::new(EnvBuilder::new().build());
    let ch = ChannelBuilder::new(env).connect("127.0.0.1:4251");
    let client = MinerProxyClient::new(ch);

    let req = MineCtxRequest {};
    let mine_ctx = client.get_mine_ctx(&req);
    println!("proof {:?}", mine_ctx);
    let req = MinedBlockRequest {
        mine_ctx: Some(MineCtxRpc {
            header: vec![2],
            nonce: 0 as u64,
        }),
        proof: vec![1 as u8]
    };
    let resp = client.mined(&req);
    println!("mined{:?}", resp);
}
