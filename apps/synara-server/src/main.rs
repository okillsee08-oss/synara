#[tokio::main]
async fn main() -> anyhow::Result<()> {
    synara_server::run().await
}
