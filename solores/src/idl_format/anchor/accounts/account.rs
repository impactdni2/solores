use heck::{ToPascalCase, ToShoutySnakeCase};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::idl_format::anchor::typedefs::{NamedType, TypeContent};
use crate::utils::conditional_pascal_case;

/// Represents an account reference in the new IDL format (spec 0.1.0)
/// Contains only the name and discriminator, with the type definition in the types array
#[derive(Deserialize, Clone, Debug)]
pub struct NamedAccountRef {
    pub name: String,
    pub discriminator: [u8; 8],
}

/// Represents an account in the old IDL format
/// Contains the full type definition inline
#[derive(Deserialize, Clone, Debug)]
pub struct NamedAccountFull {
    pub name: String,
    #[serde(flatten)]
    pub type_content: TypeContent,
}

/// Unified account representation that supports both old and new IDL formats
#[derive(Clone, Debug)]
pub struct NamedAccount {
    pub name: String,
    pub type_def: NamedType,
    pub discriminator: Option<[u8; 8]>,
}

/// Enum for deserializing either format during initial parsing
#[derive(Deserialize)]
#[serde(untagged)]
pub enum RawNamedAccount {
    /// New format: just name + discriminator (type in types array)
    Reference(NamedAccountRef),
    /// Old format: inline type definition
    Full(NamedAccountFull),
}

impl NamedAccount {
    /// Create from old format (inline type definition)
    pub fn from_full(full: NamedAccountFull) -> Self {
        Self {
            name: full.name.clone(),
            type_def: NamedType::from_name_and_content(full.name, full.type_content),
            discriminator: None, // Will be computed from name
        }
    }

    /// Create from new format (reference) by resolving the type from types array
    pub fn from_ref(ref_account: NamedAccountRef, type_def: NamedType) -> Self {
        Self {
            name: ref_account.name,
            type_def,
            discriminator: Some(ref_account.discriminator),
        }
    }
}

impl NamedAccount {
    pub fn to_token_stream(&self, cli_args: &crate::Args) -> TokenStream {
        let name = &self.name;
        // discriminant
        let account_discm_ident = format_ident!("{}_ACCOUNT_DISCM", name.to_shouty_snake_case());

        // Use provided discriminator or compute from name (old format)
        let discm = self.discriminator.unwrap_or_else(|| {
            // pre-image: "account:{AccountStructName}"
            <[u8; 8]>::try_from(
                &Sha256::digest(format!("account:{}", name.to_pascal_case()).as_bytes()).as_slice()
                    [..8],
            )
            .unwrap()
        });
        let discm_tokens: TokenStream = format!("{:?}", discm).parse().unwrap();

        let struct_def = self.type_def.to_token_stream(cli_args);

        let struct_ident = format_ident!("{}", conditional_pascal_case(name));
        let account_ident = format_ident!("{}Account", conditional_pascal_case(name));

        quote! {
            pub const #account_discm_ident: [u8; 8] = #discm_tokens;

            #struct_def

            #[derive(Clone, Debug, PartialEq)]
            pub struct #account_ident(pub #struct_ident);

            impl #account_ident {
                pub fn deserialize(buf: &[u8]) -> std::io::Result<Self> {
                    use std::io::Read;
                    let mut reader = buf;
                    let mut maybe_discm = [0u8; 8];
                    reader.read_exact(&mut maybe_discm)?;
                    if maybe_discm != #account_discm_ident {
                        return Err(
                            std::io::Error::new(
                                std::io::ErrorKind::Other, format!("discm does not match. Expected: {:?}. Received: {:?}", #account_discm_ident, maybe_discm)
                            )
                        );
                    }
                    Ok(Self(#struct_ident::deserialize(&mut reader)?))
                }

                pub fn serialize<W: std::io::Write>(&self, mut writer: W) -> std::io::Result<()> {
                    writer.write_all(&#account_discm_ident)?;
                    self.0.serialize(&mut writer)
                }

                pub fn try_to_vec(&self) -> std::io::Result<Vec<u8>> {
                    let mut data = Vec::new();
                    self.serialize(&mut data)?;
                    Ok(data)
                }
            }
        }
    }
}
