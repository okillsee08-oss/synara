use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("version") => println!("synara 0.1.0"),
        Some("help") | None => println!("synara <command>\n\nCommands:\n  server    Run the Synara server\n  version   Print version\n  help      Show help"),
        Some("server") => synara_server::run().await?,
        Some(x) => {
            eprintln!("unknown command: {x}");
            std::process::exit(2);
        }
    }
    Ok(())
}
