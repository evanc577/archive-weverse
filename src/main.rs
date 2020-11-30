#[macro_use]
extern crate lazy_static;

use std::process;

mod config;
mod network;

#[tokio::main]
async fn main() {
    match run().await {
        Ok(_) => process::exit(0),
        Err(err) => {
            eprintln!("{}", err);
            process::exit(1);
        }
    }
}

async fn run() -> Result<(), String> {
    let conf = config::read_config()?;
    let token = config::read_token(&conf.cookies_file)?;
    network::download(&conf, &token).await
}
