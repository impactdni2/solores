use heck::{ToPascalCase, ToSnakeCase};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{LitBool, LitInt};

use crate::idl_format::IdlCodegenModule;

use super::instructions::{to_ix_accounts, IxAccount, NamedInstruction};

pub struct IxPinocchioCodegenModule<'a> {
    pub instructions: &'a [NamedInstruction],
}

impl IdlCodegenModule for IxPinocchioCodegenModule<'_> {
    fn name(&self) -> &str {
        "instructions_pinocchio"
    }

    fn gen_head(&self) -> TokenStream {
        quote! {
            use pinocchio::{account::AccountView, instruction::InstructionAccount};
            use crate::instructions::*;
        }
    }

    fn gen_body(&self) -> TokenStream {
        self.instructions
            .iter()
            .filter(|ix| ix.has_accounts())
            .map(gen_pinocchio_view)
            .collect()
    }
}

fn gen_pinocchio_view(ix: &NamedInstruction) -> TokenStream {
    let accounts = ix
        .accounts
        .as_ref()
        .map_or(Vec::new(), |v| to_ix_accounts(v));

    let mut tokens = TokenStream::new();

    write_view_struct(ix, &mut tokens, &accounts);
    write_from_account_view_arr_for_view(ix, &mut tokens, &accounts);
    write_from_view_for_account_view_arr(ix, &mut tokens, &accounts);
    write_from_view_for_instruction_account_arr(ix, &mut tokens, &accounts);

    tokens
}

fn view_ident(ix: &NamedInstruction) -> proc_macro2::Ident {
    format_ident!("{}View", ix.name.to_pascal_case())
}

/// Generate the View struct
fn write_view_struct(ix: &NamedInstruction, tokens: &mut TokenStream, accounts: &[IxAccount]) {
    let view_ident = view_ident(ix);
    let view_fields = accounts.iter().map(|acc| {
        let account_name = format_ident!("{}", &acc.name.to_snake_case());
        quote! {
            pub #account_name: &'a AccountView
        }
    });
    tokens.extend(quote! {
        #[derive(Copy, Clone, Debug)]
        pub struct #view_ident<'a> {
            #(#view_fields),*
        }
    });
}

/// From<&'a [AccountView; N]> for XView<'a>
fn write_from_account_view_arr_for_view(
    ix: &NamedInstruction,
    tokens: &mut TokenStream,
    accounts: &[IxAccount],
) {
    let view_ident = view_ident(ix);
    let accounts_len_ident = ix.accounts_len_ident();
    let from_fields = accounts.iter().enumerate().map(|(i, acc)| {
        let account_ident = format_ident!("{}", &acc.name.to_snake_case());
        let index_lit = LitInt::new(&i.to_string(), Span::call_site());
        quote! {
            #account_ident: &arr[#index_lit]
        }
    });
    tokens.extend(quote! {
        impl<'a> From<&'a [AccountView; #accounts_len_ident]> for #view_ident<'a> {
            fn from(arr: &'a [AccountView; #accounts_len_ident]) -> Self {
                Self {
                    #(#from_fields),*
                }
            }
        }
    });
}

/// From<&'a XView<'a>> for [&'a AccountView; N]
fn write_from_view_for_account_view_arr(
    ix: &NamedInstruction,
    tokens: &mut TokenStream,
    accounts: &[IxAccount],
) {
    let view_ident = view_ident(ix);
    let accounts_len_ident = ix.accounts_len_ident();
    let view_fields = accounts.iter().map(|acc| {
        let account_ident = format_ident!("{}", &acc.name.to_snake_case());
        quote! {
            view.#account_ident
        }
    });
    tokens.extend(quote! {
        impl<'a> From<&'a #view_ident<'a>> for [&'a AccountView; #accounts_len_ident] {
            fn from(view: &'a #view_ident<'a>) -> Self {
                [
                    #(#view_fields),*
                ]
            }
        }
    });
}

/// From<XView<'a>> for [InstructionAccount<'a>; N]
fn write_from_view_for_instruction_account_arr(
    ix: &NamedInstruction,
    tokens: &mut TokenStream,
    accounts: &[IxAccount],
) {
    let view_ident = view_ident(ix);
    let accounts_len_ident = ix.accounts_len_ident();
    let instruction_accounts = accounts.iter().map(|acc| {
        let account_ident = format_ident!("{}", &acc.name.to_snake_case());
        let is_signer = LitBool::new(acc.signer, Span::call_site());
        let is_writable = LitBool::new(acc.writable, Span::call_site());
        quote! {
            InstructionAccount {
                address: view.#account_ident.address(),
                is_signer: #is_signer,
                is_writable: #is_writable,
            }
        }
    });
    tokens.extend(quote! {
        impl<'a> From<#view_ident<'a>> for [InstructionAccount<'a>; #accounts_len_ident] {
            fn from(view: #view_ident<'a>) -> Self {
                [
                    #(#instruction_accounts),*
                ]
            }
        }
    });
}
