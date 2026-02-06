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
#[derive(Deserialize, Clone, Debug)]
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

/// The content of a type definition without the name field
/// Used when flattening into structs that already have a name field
#[derive(Deserialize, Clone, Debug)]
pub struct TypeContent {
    pub r#type: TypedefType,
    pub docs: Option<Vec<String>>,
    pub serialization: Option<String>,
    pub repr: Option<Repr>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct NamedType {
    pub name: String,
    #[serde(flatten)]
    pub content: TypeContent,
}

impl NamedType {
    /// Create a NamedType from a name and TypeContent
    pub fn from_name_and_content(name: String, content: TypeContent) -> Self {
        Self { name, content }
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct Repr {
    pub kind: String,
    #[serde(default)]
    pub packed: bool,
}

impl NamedType {
    pub fn to_token_stream(&self, cli_args: &crate::Args) -> TokenStream {
        let name = format_ident!("{}", conditional_pascal_case(&self.name));
        // rust enums cannot impl Pod due to illegal bitpatterns
        let typedef_struct = match &self.content.r#type {
            TypedefType::r#struct(typedef_struct) => typedef_struct,
            TypedefType::r#enum(typedef_enum) => {
                return quote! {
                    #[derive(Clone, Copy, Debug, BorshDeserialize, BorshSerialize, PartialEq)]
                    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
                    pub enum #name {
                        #typedef_enum
                    }
                }
            }
        };

        // Check if this type should use zero-copy derives from CLI args or IDL serialization field
        let use_zero_copy = cli_args.zero_copy.iter().any(|e| e == &self.name)
            || self
                .content
                .serialization
                .as_ref()
                .map_or(false, |s| s == "bytemuck");

        // Check if this type should use unsafe bytemuck
        let use_unsafe_bytemuck = self
            .content
            .serialization
            .as_ref()
            .map_or(false, |s| s == "bytemuckunsafe");

        // Generate repr attribute based on CLI args or IDL repr field
        let repr_attr = if let Some(repr) = &self.content.repr {
            let kind = if repr.kind == "c" { "C" } else { &repr.kind };
            let kind = format_ident!("{}", kind);
            if repr.packed {
                quote! { #[repr(packed, #kind)] }
            } else {
                quote! { #[repr(#kind)] }
            }
        } else if use_zero_copy || use_unsafe_bytemuck {
            quote! { #[repr(C)] }
        } else {
            TokenStream::new()
        };

        let derive = if use_zero_copy {
            quote! {
                #[derive(Clone, Debug, BorshDeserialize, BorshSerialize, PartialEq, Pod, Copy, Zeroable)]
            }
        } else if use_unsafe_bytemuck {
            quote! {
                #[derive(Clone, Debug, BorshDeserialize, BorshSerialize, PartialEq, Copy)]
            }
        } else {
            quote! {
                #[derive(Clone, Copy, Debug, BorshDeserialize, BorshSerialize, PartialEq)]
            }
        };

        let unsafe_impls = if use_unsafe_bytemuck {
            quote! {
                unsafe impl Pod for #name {}
                unsafe impl Zeroable for #name {}
            }
        } else {
            TokenStream::new()
        };

        // Generate struct definition (tuple vs named)
        let struct_def = match &typedef_struct.fields {
            StructFields::Named(_) => {
                // Named struct: pub struct Name { fields }
                quote! {
                    #repr_attr
                    #derive
                    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
                    pub struct #name {
                        #typedef_struct
                    }
                }
            }
            StructFields::Unnamed(_) => {
                // Tuple struct: pub struct Name(fields);
                quote! {
                    #repr_attr
                    #derive
                    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
                    pub struct #name(#typedef_struct);
                }
            }
        };

        quote! {
            #struct_def
            #unsafe_impls
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "kind")]
pub enum TypedefType {
    r#struct(TypedefStruct),
    r#enum(TypedefEnum),
}

/// Represents struct fields which can be either named or unnamed (tuple)
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum StructFields {
    /// Named fields: [{"name": "x", "type": "u8"}]
    Named(Vec<TypedefField>),
    /// Unnamed/tuple fields: ["bool", "u8"]
    Unnamed(Vec<TypedefFieldTypeWrap>),
}

#[derive(Deserialize, Clone, Debug)]
pub struct TypedefStruct {
    pub fields: StructFields,
}

#[derive(Deserialize, Clone, Debug)]
pub struct TypedefField {
    pub name: String,
    #[serde(deserialize_with = "string_or_struct")]
    pub r#type: TypedefFieldType,
    pub docs: Option<Vec<String>>,
}

/// All instances should be annotated with
/// deserialize_with = "string_or_struct"
#[derive(Deserialize, Clone, Debug)]
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

#[derive(Deserialize, Clone, Debug)]
pub struct TypedefFieldArray(
    #[serde(deserialize_with = "string_or_struct")] Box<TypedefFieldType>,
    u32, // borsh spec says array sizes are u32
);

/// serde newtype workaround for use in Vec<TypedefFieldType>:
/// https://github.com/serde-rs/serde/issues/723#issuecomment-871016087
#[derive(Deserialize, Clone, Debug)]
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

#[derive(Deserialize, Clone, Debug)]
pub struct TypedefEnum {
    pub variants: Vec<EnumVariant>,
}

#[derive(Deserialize, Clone, Debug)]
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

#[derive(Deserialize, Clone, Debug)]
pub struct EnumVariant {
    pub name: String,
    pub fields: Option<EnumVariantFields>,
}

impl ToTokens for TypedefStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match &self.fields {
            StructFields::Named(fields) => {
                let typedef_fields = fields.iter().map(|f| {
                    // Check if this field needs serde_big_array attribute
                    let serde_attr = if let TypedefFieldType::array(TypedefFieldArray(_, len)) = &f.r#type {
                        if *len > 32 {
                            quote! { #[cfg_attr(feature = "serde", serde(with = "serde_big_array::BigArray"))] }
                        } else {
                            TokenStream::new()
                        }
                    } else {
                        TokenStream::new()
                    };

                    let name = format_ident!("{}", f.name.to_snake_case());
                    let ty = &f.r#type;
                    quote! {
                        #serde_attr
                        pub #name: #ty
                    }
                });
                tokens.extend(quote! {
                    #(#typedef_fields),*
                })
            }
            StructFields::Unnamed(fields) => {
                // Tuple struct: just list the types
                let typedef_fields = fields.iter().map(|f| {
                    let ty = &f.0;
                    quote! { pub #ty }
                });
                tokens.extend(quote! {
                    #(#typedef_fields),*
                })
            }
        }
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

impl StructFields {
    pub fn has_pubkey(&self) -> bool {
        match self {
            Self::Named(v) => v.iter().any(|f| f.r#type.is_or_has_pubkey()),
            Self::Unnamed(v) => v.iter().any(|f| f.0.is_or_has_pubkey()),
        }
    }

    pub fn has_defined(&self) -> bool {
        match self {
            Self::Named(v) => v.iter().any(|f| f.r#type.is_or_has_defined()),
            Self::Unnamed(v) => v.iter().any(|f| f.0.is_or_has_defined()),
        }
    }
}

impl TypedefType {
    pub fn has_pubkey_field(&self) -> bool {
        match self {
            Self::r#enum(e) => e.variants.iter().any(|e| e.has_pubkey()),
            Self::r#struct(s) => s.fields.has_pubkey(),
        }
    }

    pub fn has_defined_field(&self) -> bool {
        match self {
            Self::r#enum(e) => e.variants.iter().any(|e| e.has_defined()),
            Self::r#struct(s) => s.fields.has_defined(),
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
