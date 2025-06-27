#![cfg(feature = "test_gen_examples")]

use test_utils::{check_example, gen_example};

const BASE_WORKSPACE_DEPS_ARGS: [&str; 14] = [
    "--solana-program-vers",
    "workspace=true",
    "--borsh-vers",
    "workspace=true",
    "--thiserror-vers",
    "workspace=true",
    "--num-derive-vers",
    "workspace=true",
    "--num-traits-vers",
    "workspace=true",
    "--serde-vers",
    "workspace=true",
    "--bytemuck-vers",
    "workspace=true",
];


#[test]
fn test_unstake_it() -> Result<(), Box<dyn std::error::Error>> {
    const EXAMPLE_PATH: &str = "anchor/unstake_it";
    gen_example(EXAMPLE_PATH, &BASE_WORKSPACE_DEPS_ARGS)?;
    check_example(EXAMPLE_PATH, "unstake_interface")
}

#[test]
fn test_marinade() -> Result<(), Box<dyn std::error::Error>> {
    const EXAMPLE_PATH: &str = "anchor/marinade";
    gen_example(EXAMPLE_PATH, &BASE_WORKSPACE_DEPS_ARGS)?;
    check_example(EXAMPLE_PATH, "marinade_finance_interface")
}

#[test]
fn test_anchor_ix_no_privilege() -> Result<(), Box<dyn std::error::Error>> {
    const EXAMPLE_PATH: &str = "anchor/ix_no_privilege";
    gen_example(EXAMPLE_PATH, &BASE_WORKSPACE_DEPS_ARGS)?;
    check_example(EXAMPLE_PATH, "anchor_ix_no_privilege_interface")
}

#[test]
fn test_anchor_ix_no_args() -> Result<(), Box<dyn std::error::Error>> {
    const EXAMPLE_PATH: &str = "anchor/ix_no_args";
    gen_example(EXAMPLE_PATH, &BASE_WORKSPACE_DEPS_ARGS)?;
    check_example(EXAMPLE_PATH, "anchor_ix_no_args_interface")
}

#[test]
fn test_anchor_ix_no_accounts() -> Result<(), Box<dyn std::error::Error>> {
    const EXAMPLE_PATH: &str = "anchor/ix_no_accounts";
    gen_example(EXAMPLE_PATH, &BASE_WORKSPACE_DEPS_ARGS)?;
    check_example(EXAMPLE_PATH, "anchor_ix_no_accounts_interface")
}

#[test]
fn test_anchor_ix_no_accounts_pubkey_arg() -> Result<(), Box<dyn std::error::Error>> {
    const EXAMPLE_PATH: &str = "anchor/ix_no_accounts_pubkey_arg";
    gen_example(EXAMPLE_PATH, &BASE_WORKSPACE_DEPS_ARGS)?;
    check_example(EXAMPLE_PATH, "anchor_ix_no_accounts_pubkey_arg_interface")
}

#[test]
fn test_anchor_ix_blank() -> Result<(), Box<dyn std::error::Error>> {
    const EXAMPLE_PATH: &str = "anchor/ix_blank";
    gen_example(EXAMPLE_PATH, &BASE_WORKSPACE_DEPS_ARGS)?;
    check_example(EXAMPLE_PATH, "anchor_ix_blank_interface")
}


#[test]
fn test_drift() -> Result<(), Box<dyn std::error::Error>> {
    const EXAMPLE_PATH: &str = "anchor/drift";
    gen_example(
        EXAMPLE_PATH,
        &[
            BASE_WORKSPACE_DEPS_ARGS.as_ref(),
            &[
                "--program-id",
                "dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH",
            ],
        ]
        .concat(),
    )?;
    check_example(EXAMPLE_PATH, "drift_interface")
}

