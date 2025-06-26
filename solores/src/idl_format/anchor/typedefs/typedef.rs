#![allow(non_camel_case_types)]

use std::str::FromStr;

use heck::{ToPascalCase, ToSnakeCase};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use serde::{Deserialize, Deserializer};
use syn::Index;
use void::Void;

use crate::utils::{
    conditional_pascal_case, primitive_or_pubkey_to_token, string_or_struct, PUBKEY_TOKEN,
};

// Custom struct to handle both string and object formats for "defined"
#[derive(Deserialize)]
#[serde(untagged)]
pub enum DefinedType {
    String(String),
    Object { name: String },
}

impl DefinedType {
    pub fn name(&self) -> &str {
        match self {
            DefinedType::String(s) => s,
            DefinedType::Object { name } => name,
        }
    }
}

// Custom deserializer for the defined field
fn deserialize_defined<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let defined_type = DefinedType::deserialize(deserializer)?;
    Ok(defined_type.name().to_string())
}

#[derive(Deserialize)]
pub struct NamedType {
    pub name: String,
    pub r#type: TypedefType,
    pub docs: Option<Vec<String>>,
    pub serialization: Option<String>,
    pub repr: Option<Repr>,
}

#[derive(Deserialize)]
pub struct Repr {
    pub kind: String,
    pub packed: bool,
}

impl NamedType {
    pub fn to_token_stream(&self, cli_args: &crate::Args) -> TokenStream {
        let name = format_ident!("{}", conditional_pascal_case(&self.name));
        // rust enums cannot impl Pod due to illegal bitpatterns
        let typedef_struct = match &self.r#type {
            TypedefType::r#struct(typedef_struct) => typedef_struct,
            TypedefType::r#enum(typedef_enum) => {
                return quote! {
                    #[derive(Clone, Debug, BorshDeserialize, BorshSerialize, PartialEq)]
                    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
                    pub enum #name {
                        #typedef_enum
                    }
                }
            }
        };

        let derive = if cli_args.zero_copy.iter().any(|e| e == &self.name) {
            quote! {
                #[repr(C)]
                #[derive(Clone, Debug, BorshDeserialize, BorshSerialize, PartialEq, Pod, Copy, Zeroable)]
            }
        } else {
            quote! {
                #[derive(Clone, Debug, BorshDeserialize, BorshSerialize, PartialEq)]
            }
        };
        quote! {
            #derive
            #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
            pub struct #name {
                #typedef_struct
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind")]
pub enum TypedefType {
    r#struct(TypedefStruct),
    r#enum(TypedefEnum),
}

#[derive(Deserialize)]
pub struct TypedefStruct {
    pub fields: Vec<TypedefField>,
}

#[derive(Deserialize)]
pub struct TypedefField {
    pub name: String,
    #[serde(deserialize_with = "string_or_struct")]
    pub r#type: TypedefFieldType,
    pub docs: Option<Vec<String>>,
}

/// All instances should be annotated with
/// deserialize_with = "string_or_struct"
#[derive(Deserialize)]
pub enum TypedefFieldType {
    // handled by string_or_struct's string
    PrimitiveOrPubkey(String),

    // rest handled by string_or_struct's struct
    #[serde(deserialize_with = "deserialize_defined")]
    defined(String),
    array(TypedefFieldArray),

    #[serde(deserialize_with = "string_or_struct")]
    option(Box<TypedefFieldType>),

    #[serde(deserialize_with = "string_or_struct")]
    vec(Box<TypedefFieldType>),
}

#[derive(Deserialize)]
pub struct TypedefFieldArray(
    #[serde(deserialize_with = "string_or_struct")] Box<TypedefFieldType>,
    u32, // borsh spec says array sizes are u32
);

/// serde newtype workaround for use in Vec<TypedefFieldType>:
/// https://github.com/serde-rs/serde/issues/723#issuecomment-871016087
#[derive(Deserialize)]
pub struct TypedefFieldTypeWrap(#[serde(deserialize_with = "string_or_struct")] TypedefFieldType);

impl FromStr for TypedefFieldType {
    type Err = Void;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::PrimitiveOrPubkey(s.into()))
    }
}

impl FromStr for Box<TypedefFieldType> {
    type Err = Void;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Box::new(TypedefFieldType::from_str(s)?))
    }
}

#[derive(Deserialize)]
pub struct TypedefEnum {
    pub variants: Vec<EnumVariant>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum EnumVariantFields {
    Struct(Vec<TypedefField>),
    Tuple(Vec<TypedefFieldTypeWrap>),
}

impl EnumVariantFields {
    pub fn has_pubkey(&self) -> bool {
        match self {
            Self::Struct(v) => v.iter().any(|f| f.r#type.is_or_has_pubkey()),
            Self::Tuple(v) => v.iter().any(|f| f.0.is_or_has_pubkey()),
        }
    }

    pub fn has_defined(&self) -> bool {
        match self {
            Self::Struct(v) => v.iter().any(|f| f.r#type.is_or_has_defined()),
            Self::Tuple(v) => v.iter().any(|f| f.0.is_or_has_defined()),
        }
    }
}

#[derive(Deserialize)]
pub struct EnumVariant {
    pub name: String,
    pub fields: Option<EnumVariantFields>,
}

impl ToTokens for TypedefStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let typedef_fields = self.fields.iter().map(|f| quote! { pub #f });
        tokens.extend(quote! {
            #(#typedef_fields),*
        })
    }
}

impl ToTokens for TypedefField {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = format_ident!("{}", self.name.to_snake_case());
        let ty = &self.r#type;
        tokens.extend(quote! {
            #name: #ty
        })
    }
}

impl ToTokens for TypedefFieldType {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ty: TokenStream = match self {
            Self::PrimitiveOrPubkey(s) => primitive_or_pubkey_to_token(s).parse().unwrap(),
            Self::defined(s) => s.parse().unwrap(),
            Self::array(a) => a.to_token_stream(),
            Self::vec(v) => quote! {
                Vec<#v>
            },
            Self::option(o) => quote! {
                Option<#o>
            },
        };
        tokens.extend(ty);
    }
}

impl ToTokens for TypedefFieldArray {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ty = &self.0;
        let n = Index::from(self.1 as usize);
        tokens.extend(quote! {
            [#ty; #n]
        })
    }
}

impl ToTokens for TypedefEnum {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let variants = &self.variants;
        tokens.extend(quote! {
            #(#variants),*
        })
    }
}

impl ToTokens for EnumVariant {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let v = format_ident!("{}", self.name.to_pascal_case());
        let maybe_inner_fields = self
            .fields
            .as_ref()
            .map_or(quote! {}, |fields| match fields {
                EnumVariantFields::Struct(v) => {
                    let typedef_fields = v.iter();
                    quote! {
                        { #(#typedef_fields),* }
                    }
                }
                EnumVariantFields::Tuple(v) => {
                    let unnamed_fields = v.iter().map(|wrap| &wrap.0);
                    quote! {
                        ( #(#unnamed_fields),* )
                    }
                }
            });
        tokens.extend(quote! {
            #v #maybe_inner_fields
        });
    }
}

impl TypedefType {
    pub fn has_pubkey_field(&self) -> bool {
        match self {
            Self::r#enum(e) => e.variants.iter().any(|e| e.has_pubkey()),
            Self::r#struct(s) => s.fields.iter().any(|f| f.r#type.is_or_has_pubkey()),
        }
    }

    pub fn has_defined_field(&self) -> bool {
        match self {
            Self::r#enum(e) => e.variants.iter().any(|e| e.has_defined()),
            Self::r#struct(s) => s.fields.iter().any(|f| f.r#type.is_or_has_defined()),
        }
    }
}

impl TypedefFieldType {
    pub fn is_or_has_pubkey(&self) -> bool {
        match self {
            Self::PrimitiveOrPubkey(s) => primitive_or_pubkey_to_token(s) == PUBKEY_TOKEN,
            Self::array(a) => a.0.is_or_has_pubkey(),
            Self::option(o) => o.is_or_has_pubkey(),
            Self::vec(v) => v.is_or_has_pubkey(),
            Self::defined(_) => false,
        }
    }

    pub fn is_or_has_defined(&self) -> bool {
        match self {
            Self::PrimitiveOrPubkey(_) => false,
            Self::array(a) => a.0.is_or_has_defined(),
            Self::option(o) => o.is_or_has_defined(),
            Self::vec(v) => v.is_or_has_defined(),
            Self::defined(_) => true,
        }
    }
}

impl EnumVariant {
    pub fn has_pubkey(&self) -> bool {
        match &self.fields {
            None => false,
            Some(fields) => fields.has_pubkey(),
        }
    }

    pub fn has_defined(&self) -> bool {
        match &self.fields {
            None => false,
            Some(fields) => fields.has_defined(),
        }
    }
}
