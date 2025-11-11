use serde::Deserialize;
use toml::{map::Map, Value};

use crate::write_cargotoml::{
    DependencyValue, FeaturesDependencyValue, OptionalDependencyValue, BORSH_CRATE, BYTEMUCK_CRATE,
    NUM_DERIVE_CRATE, NUM_TRAITS_CRATE, SERDE_BIG_ARRAY_CRATE, SERDE_BYTES_CRATE, SERDE_CRATE,
    SOLANA_PROGRAM_CRATE, THISERROR_CRATE,
};

use super::{IdlCodegenModule, IdlFormat};

use self::{
    accounts::{AccountsCodegenModule, NamedAccount, RawNamedAccount},
    errors::{ErrorEnumVariant, ErrorsCodegenModule},
    instructions::{IxCodegenModule, NamedInstruction},
    typedefs::{NamedType, TypedefsCodegenModule},
};

pub mod accounts;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod typedefs;

#[derive(Deserialize)]
pub struct AnchorIdl {
    pub address: String,
    pub metadata: Metadata,
    #[serde(default)]
    pub accounts: Option<Vec<RawNamedAccount>>,
    pub types: Option<Vec<NamedType>>,
    pub instructions: Option<Vec<NamedInstruction>>,
    pub errors: Option<Vec<ErrorEnumVariant>>,
    // pub events: Option<Vec<Event>>,
}

#[derive(Deserialize)]
pub struct Metadata {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub spec: Option<String>,
    pub description: String,
}

impl AnchorIdl {
    /// Resolve account definitions from raw accounts
    /// In new IDL format (spec 0.1.0), accounts are references that need to be resolved from types array
    /// In old format, accounts contain inline type definitions
    fn resolve_accounts(&self) -> Option<Vec<NamedAccount>> {
        let raw_accounts = self.accounts.as_ref()?;

        let resolved: Vec<NamedAccount> = raw_accounts
            .iter()
            .filter_map(|raw| match raw {
                RawNamedAccount::Full(full) => {
                    // Old format: account has inline type definition
                    Some(NamedAccount::from_full(full.clone()))
                }
                RawNamedAccount::Reference(ref_account) => {
                    // New format: look up type definition in types array
                    let type_def = self.types.as_ref()?.iter()
                        .find(|t| t.name == ref_account.name)?
                        .clone();

                    Some(NamedAccount::from_ref(ref_account.clone(), type_def))
                }
            })
            .collect();

        if resolved.is_empty() {
            None
        } else {
            Some(resolved)
        }
    }

    /// Get types that are not accounts (for the typedefs module)
    /// In new format, account types are in the types array, so we need to filter them out
    fn get_non_account_types(&self) -> Option<Vec<NamedType>> {
        let types = self.types.as_ref()?;

        // Get set of account names
        let account_names: std::collections::HashSet<String> = self.accounts.as_ref()
            .map(|accounts| {
                accounts.iter()
                    .filter_map(|raw| match raw {
                        RawNamedAccount::Reference(r) => Some(r.name.clone()),
                        RawNamedAccount::Full(f) => Some(f.name.clone()),
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Filter out account types
        let non_account_types: Vec<NamedType> = types.iter()
            .filter(|t| !account_names.contains(&t.name))
            .cloned()
            .collect();

        if non_account_types.is_empty() {
            None
        } else {
            Some(non_account_types)
        }
    }
}

impl IdlFormat for AnchorIdl {
    fn program_name(&self) -> &str {
        &self.metadata.name
    }

    fn program_version(&self) -> &str {
        &self.metadata.version
    }

    fn program_address(&self) -> Option<&str> {
        Some(&self.address)
    }

    /// Anchor IDLs dont seem to have an identifier,
    /// assume unindentified IDLs are anchor by default.
    /// -> Make sure to try deserializing Anchor last
    fn is_correct_idl_format(&self) -> bool {
        true
    }

    fn modules<'me>(&'me self, args: &'me crate::Args) -> Vec<Box<dyn IdlCodegenModule + 'me>> {
        let mut res: Vec<Box<dyn IdlCodegenModule + 'me>> = Vec::new();

        // Resolve accounts (handling both old and new IDL formats)
        let resolved_accounts = self.resolve_accounts();
        // Store resolved accounts in a static-lifetime container for the module
        // We need to box and leak it to get a 'me lifetime
        if let Some(accounts) = resolved_accounts {
            let accounts_box = Box::new(accounts);
            let accounts_ref: &'me [NamedAccount] = Box::leak(accounts_box);
            res.push(Box::new(AccountsCodegenModule {
                cli_args: args,
                named_accounts: accounts_ref,
            }));
        }

        // Get non-account types (in new format, accounts are in types array)
        let non_account_types = self.get_non_account_types();
        if let Some(types) = non_account_types {
            let types_box = Box::new(types);
            let types_ref: &'me [NamedType] = Box::leak(types_box);
            res.push(Box::new(TypedefsCodegenModule {
                cli_args: args,
                named_types: types_ref,
            }));
        }

        if let Some(v) = &self.instructions {
            res.push(Box::new(IxCodegenModule {
                program_name: self.program_name(),
                instructions: v,
            }));
        }
        if let Some(v) = &self.errors {
            res.push(Box::new(ErrorsCodegenModule {
                program_name: self.program_name(),
                variants: v,
            }));
        }
        // if let Some(v) = &self.events {
        //     res.push(Box::new(EventsCodegenModule(v)));
        // }
        res
    }

    fn dependencies(&self, args: &crate::Args) -> Map<String, Value> {
        let mut map = Map::new();
        map.insert(BORSH_CRATE.into(), DependencyValue(&args.borsh_vers).into());
        map.insert(
            BYTEMUCK_CRATE.into(),
            FeaturesDependencyValue {
                dependency: DependencyValue(&args.bytemuck_vers),
                features: vec!["derive".into()],
            }
            .into(),
        );
        map.insert(
            SOLANA_PROGRAM_CRATE.into(),
            DependencyValue(&args.solana_program_vers).into(),
        );
        map.insert(
            SERDE_CRATE.into(),
            OptionalDependencyValue(DependencyValue(&args.serde_vers)).into(),
        );
        map.insert(
            SERDE_BYTES_CRATE.into(),
            OptionalDependencyValue(DependencyValue(&args.serde_bytes_vers)).into(),
        );
        map.insert(
            SERDE_BIG_ARRAY_CRATE.into(),
            OptionalDependencyValue(DependencyValue(&args.serde_big_array_vers)).into(),
        );
        if self.errors.is_some() {
            map.insert(
                THISERROR_CRATE.into(),
                DependencyValue(&args.thiserror_vers).into(),
            );
            map.insert(
                NUM_DERIVE_CRATE.into(),
                DependencyValue(&args.num_derive_vers).into(),
            );
            map.insert(
                NUM_TRAITS_CRATE.into(),
                DependencyValue(&args.num_traits_vers).into(),
            );
        }
        map
    }
}
