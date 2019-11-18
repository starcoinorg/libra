pub mod types;
pub mod server;

#[cfg(test)]
mod tests {
    use crate::server;
    #[test]
    fn test_miner_rpc_server() {
        server::run_service();
    }
}