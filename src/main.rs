#[macro_use]
extern crate lazy_static;

use std::process;
use crate::config::Config;
use crate::network::Network;

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
    download(&conf, &token).await
}


async fn download(conf: &Config, token: &String) -> Result<(), String> {
    let n = Network::new(&conf, &token)?;

    Ok(())
}
