use miner::MineClient;
use async_std::task;

fn main() {
    let miner = MineClient::new("127.0.0.1:4251".to_string());
    task::block_on(
        miner.start()
    );
}