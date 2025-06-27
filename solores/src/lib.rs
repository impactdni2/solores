#![doc = include_str!("../README.md")]

use std::{
    env,
    fs::{self, File, OpenOptions},
    path::PathBuf,
};

use clap::{command, Parser};
use idl_format::IdlFormat;

use crate::idl_format::anchor::AnchorIdl;

// Just make all mods pub to allow ppl to use the lib

pub mod idl_format;
pub mod utils;
pub mod write_cargotoml;
pub mod write_gitignore;
pub mod write_src;

use write_cargotoml::write_cargotoml;
use write_gitignore::write_gitignore;
use write_src::*;

const DEFAULT_OUTPUT_CRATE_NAME_MSG: &str = "<name-of-program>_interface";
const DEFAULT_PROGRAM_ID_MSG: &str = "program ID in IDL else system program ID if absent";
const RUST_LOG_ENV_VAR: &str = "RUST_LOG";

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    pub idl_path: PathBuf,

    #[arg(
        long,
        short,
        help = "directory to output generated crate to",
        default_value = "./"
    )]
    pub output_dir: PathBuf,

    #[arg(
        long,
        help = "output crate name",
        default_value = DEFAULT_OUTPUT_CRATE_NAME_MSG,
    )]
    pub output_crate_name: String,

    #[arg(long, short, help = "program ID / address / pubkey", default_value = DEFAULT_PROGRAM_ID_MSG)]
    pub program_id: Option<String>,

    #[arg(
        long,
        short,
        help = "typedefs and accounts to derive bytemuck::Pod for. Does not currently check validity of derivation."
    )]
    pub zero_copy: Vec<String>,

    #[arg(
        long,
        short,
        help = "solana-program dependency version for generated crate",
        default_value = "workspace = true"
    )]
    pub solana_program_vers: String,

    #[arg(
        long,
        short,
        help = "borsh dependency version for generated crate",
        default_value = "workspace = true"
    )]
    pub borsh_vers: String,

    #[arg(
        long,
        help = "thiserror dependency version for generated crate",
        default_value = "workspace = true"
    )]
    pub thiserror_vers: String,

    #[arg(
        long,
        help = "num-derive dependency version for generated crate",
        default_value = "workspace = true"
    )]
    pub num_derive_vers: String,

    #[arg(
        long,
        help = "num-traits dependency version for generated crate",
        default_value = "workspace = true"
    )]
    pub num_traits_vers: String,

    #[arg(
        long,
        help = "serde dependency version for generated crate",
        default_value = "workspace = true"
    )]
    pub serde_vers: String,

    #[arg(
        long,
        help = "bytemuck dependency version for generated crate",
        default_value = "workspace = true"
    )]
    pub bytemuck_vers: String,

    #[arg(
        long,
        help = "serde_bytes dependency version for generated crate",
        default_value = "workspace = true"
    )]
    pub serde_bytes_vers: String,

    #[arg(long, help = "write gitignore file", default_value = "false")]
    pub write_gitignore: bool,

    #[arg(long, help = "cargo edition", default_value = "2024")]
    pub cargo_edition: String,
}

/// The CLI entrypoint
pub fn main() {
    if env::var(RUST_LOG_ENV_VAR).is_err() {
        env::set_var(RUST_LOG_ENV_VAR, "info")
    }
    env_logger::init();
    log_panics::init();

    let mut args = Args::parse();

    let mut file = OpenOptions::new().read(true).open(&args.idl_path).unwrap();

    let idl = load_idl(&mut file);

    if args.output_crate_name == DEFAULT_OUTPUT_CRATE_NAME_MSG {
        args.output_crate_name = format!("{}_interface", idl.program_name());
    }

    args.program_id = args.program_id.and_then(|s| {
        if s == DEFAULT_PROGRAM_ID_MSG {
            None
        } else {
            Some(s)
        }
    });

    args.output_dir.push(&args.output_crate_name);
    fs::create_dir_all(args.output_dir.join("src/")).unwrap();

    // TODO: multithread, 1 thread per generated file
    if args.write_gitignore {
        write_gitignore(&args).unwrap();
    }
    write_cargotoml(&args, idl.as_ref()).unwrap();
    write_lib(&args, idl.as_ref()).unwrap();

    log::info!(
        "{} crate written to {}",
        args.output_crate_name,
        args.output_dir.to_string_lossy()
    );
}

pub fn load_idl(file: &mut File) -> Box<dyn IdlFormat> {
    match serde_json::from_reader::<&File, AnchorIdl>(file) {
        Ok(anchor_idl) => {
            log::info!("Successfully loaded anchor IDL");
            Box::new(anchor_idl)
        }
        Err(e) => {
            panic!("Could not determine IDL format: {:?}", e);
        }
    }
}
