# solores

Solana IDL to Rust client / CPI interface generator.

> [solita](https://github.com/metaplex-foundation/solita), light of my life, fire of my loins

This software is still in its early stages of development. USE AT YOUR OWN RISK. It's a codegen CLI, so you can always read and modify the generated code if you need to.

## Contents

- [solores](#solores)
  - [Contents](#contents)
  - [Supported IDL Formats](#supported-idl-formats)
  - [Installation](#installation)
  - [Examples](#examples)
    - [Anchor IDL](#anchor-idl)
  - [Features](#features)
    - [Serde](#serde)
    - [Keys From Array](#keys-from-array)
    - [Accounts From Array](#accounts-from-array)
    - [Instruction Accounts Verification Functions](#instruction-accounts-verification-functions)
    - [Zero-copy/bytemuck support](#zero-copy-bytemuck-support)
    - [`*_with_program_id()`](#_with_program_id)
  - [Comparison To Similar Libs](#comparison-to-similar-libs)
    - [anchor-gen](#anchor-gen)
  - [Known Missing Features](#known-missing-features)
    - [General](#general)
    - [Anchor](#anchor)

<small><i><a href='http://ecotrust-canada.github.io/markdown-toc/'>Table of contents generated with markdown-toc</a></i></small>

## Supported IDL Formats

- [Anchor](https://github.com/coral-xyz/anchor)

## Installation

`cargo install solores` to install the CLI binary.

## Examples

### Anchor IDL

For anchor IDLs, the crate will also:

- export all accounts' discriminant as consts.
- create a `*Account` newtype that includes account discriminant checking in borsh serde operations
- export event struct defs


## Features

### Serde

`serde` is added as an optional dependency behind the `serde` feature-flag to the generated crate to provide `Serialize` and `Deserialize` implementations for the various typedefs and onchain accounts.

Do note that since it's a simple derive, `Pubkey`s are de/serialized as byte arrays instead of base-58 strings.

### Keys From Array

The various `*Keys` struct also impl `From<[Pubkey; *_IX_ACCOUNTS_LEN]>` to support indexing

```rust ignore
use my_token_interface::{TRANSFER_IX_ACCOUNTS_LEN, TransferKeys};
use solana_program::{pubkey::Pubkey, sysvar::instructions::{BorrowedAccountMeta, BorrowedInstruction}};
use std::convert::TryInto;

fn index_instruction(ix: BorrowedInstruction) {
    let metas: [BorrowedAccountMeta<'_>; TRANSFER_IX_ACCOUNTS_LEN] = ix.accounts.try_into().unwrap();
    let pubkeys = metas.map(|meta| *meta.pubkey);
    let transfer_keys: TransferKeys = pubkeys.into();

    // Now you can do stuff like `transfer_keys.src` instead of
    // having to keep track of the various account indices
    //
    // ...
}
```

### Accounts From Array

The various `*Accounts` also impl `From<&[AccountInfo; *_IX_ACCOUNTS_LEN]>` to make unpacking from the program accounts slice more ergonomic.

```rust ignore
use my_token_interface::{TRANSFER_IX_ACCOUNTS_LEN, TransferAccounts, TransferArgs, TransferIxArgs, transfer_invoke};
use solana_program::{account_info::{AccountInfo, next_account_info}, entrypoint::ProgramResult, program::invoke, pubkey::Pubkey};

pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
    let transfer_accounts: &[AccountInfo; TRANSFER_IX_ACCOUNTS_LEN] = accounts[..TRANSFER_IX_ACCOUNTS_LEN].try_into().unwrap();
    let accounts: TransferAccounts = transfer_accounts.into();

    transfer_invoke(
        accounts,
        TransferIxArgs {
            transfer_args: TransferArgs { amount: 1_000 },
        }
    )
}
```

### Instruction Accounts Verification Functions

A function to compare equality between the pubkeys of a instruction `*Accounts` struct with a `*Keys` struct is generated:

```rust ignore
use my_token_interface::{TransferAccounts, TransferKeys, transfer_verify_account_keys};
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey, program_error::ProgramError};

pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
    let accounts: TransferAccounts = ...
    let expected_keys: TransferKeys = ...

    // transfer_verify_account_keys() returns the first non-matching pubkeys between accounts and expected_keys
    if let Err((actual_pubkey, expected_pubkey)) = transfer_verify_account_keys(accounts, expected_keys) {
        return Err(ProgramError::InvalidAccountData);
    }
}
```

This function is not generated if the instruction has no account inputs.

A function to ensure writable + signer privileges of a instruction `*Accounts` struct is also generated:

```rust ignore
use my_token_interface::{TransferAccounts, TransferKeys, transfer_verify_account_privileges};
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey, program_error::ProgramError};

pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
    let accounts: TransferAccounts = ...

    if let Err((offending_acc, program_err)) = transfer_verify_account_privileges(accounts) {
        solana_program::msg!("Writable/signer privilege escalation for {}: {}", offending_acc.key, program_err);
        return Err(program_err);
    }
}
```

This function is not generated if the instruction has no privileged account inputs (only non-signer and non-writable accounts).

### Zero-copy/bytemuck support

Pass `-z <name-of-type-or-account-in-idl>` to additionally derive `Pod + Zeroable + Copy` for the generated types. Accepts multiple options. The correctness of the derive is not checked.

### `*_with_program_id()`

The following instructions that take a program ID pubkey as argument are also exported:

- `*_ix_with_program_id()`
- `*_invoke_with_program_id()`
- `*_invoke_signed_with_program_id()`

They allow the creation of `Instruction`s and invoking of programs of the same interface at a different program ID.

## Comparison To Similar Libs

### anchor-gen

Compared to [anchor-gen](https://github.com/saber-hq/anchor-gen), solores:

- Has no dependency on [anchor](https://github.com/coral-xyz/anchor). The generated crate's dependencies are:

  - [borsh](https://github.com/near/borsh-rs) + [solana-program](https://github.com/solana-labs/solana/tree/master/sdk/program)
  - [thiserror](https://github.com/dtolnay/thiserror) + [num-derive](https://github.com/rust-num/num-derive) + [num-traits](https://github.com/rust-num/num-traits) if the idl contains error enum definitions.
  - [bytemuck](https://github.com/Lokathor/bytemuck) if any `-z` types are provided

- Produces human-readable rust code in a new, separate crate instead of using a proc-macro.

- Exposes lower-level constructs such as functions for creating the `solana_program::instruction::Instruction` struct to allow for greater customizability.

## Known Missing Features

Please check the repo's issues list for more.

### General

- Does not check correctness of zero-copy/bytemuck accounts derives

### Anchor

- Does not handle account namespaces
- Does not handle the state instruction namespace
