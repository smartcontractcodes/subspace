//! Spartan-farmer implementation overview
//!
//! The application contains 2 primary commands: plot and farm.
//!
//! During plotting process we create a binary plot file, which contains spartan-encoded pieces one
//! after another as well as RocksDB key-value database with tags, where key is tag (first 8 bytes
//! of `hmac(encoding, salt)`) and value is an offset of corresponding encoded piece in the plot (we
//! can do this because all pieces have the same size). So for every 4096 bytes we also store a
//! record with 8-bytes tag and 8-bytes index (+some overhead of RocksDB itself).
//!
//! During farming process we receive a challenge and need to find a solution, given target and
//! solution range. In order to find solution we do range query in RocksDB. For that we interpret
//! target as 64-bit unsigned integer, and find all of the keys in tags database that are
//! `target ± solution range` (while also handing overflow/underlow) converted back to bytes.
#![feature(try_blocks)]
#![feature(hash_drain_filter)]

mod commands;
mod config;
mod crypto;
mod plot;
mod utils;

use clap::{Clap, ValueHint};
use env_logger::Env;
use log::info;
use std::fs;
use std::path::PathBuf;
use subspace_core_primitives::{Piece, PIECE_SIZE};
use tokio::runtime::Runtime;

type Tag = [u8; 8];
type Salt = [u8; 8];

const PRIME_SIZE: usize = 32;
const SIGNING_CONTEXT: &[u8] = b"FARMER";
const BATCH_SIZE: u64 = (16 * 1024 * 1024 / PIECE_SIZE) as u64;
// TODO: Move to codec
// const CUDA_BATCH_SIZE: u64 = (32 * 1024) as u64;

#[derive(Debug, Clap)]
#[clap(about, version)]
enum Command {
    /// Erase existing plot
    ErasePlot {
        /// Use custom path for data storage instead of platform-specific default
        #[clap(long, value_hint = ValueHint::FilePath)]
        custom_path: Option<PathBuf>,
    },
    /// Start a farmer using previously created plot
    Farm {
        /// Use custom path for data storage instead of platform-specific default
        #[clap(long, value_hint = ValueHint::FilePath)]
        custom_path: Option<PathBuf>,
        #[clap(long, default_value = "ws://127.0.0.1:9944")]
        ws_server: String,
    },
}

fn main() {
    env_logger::init_from_env(Env::new().default_filter_or("info"));

    let command: Command = Command::parse();
    let runtime = Runtime::new().unwrap();

    match command {
        Command::ErasePlot { custom_path } => {
            let path = utils::get_path(custom_path);
            info!("Erasing the plot");
            let _ = fs::remove_file(path.join("plot.bin"));
            info!("Erasing plot metadata");
            let _ = fs::remove_dir_all(path.join("plot-tags"));
            info!("Erasing old identify");
            let _ = fs::remove_file(path.join("identity.bin"));
            info!("Erasing configuration");
            let _ = fs::remove_dir_all(path.join("config"));
            info!("Done");
        }
        Command::Farm {
            custom_path,
            ws_server,
        } => {
            let path = utils::get_path(custom_path);
            runtime.block_on(commands::farm(path, &ws_server)).unwrap();
        }
    }
}
