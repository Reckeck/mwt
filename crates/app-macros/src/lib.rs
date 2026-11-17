//! `#[ipc]`: turns an ordinary sync `fn` into a Tauri command that runs through
//! `crate::ipc::run` (sandbox gate, blocking thread, vault lock, error
//! conversion). The macro only does structure; behaviour lives in `ipc::run`.
//!
//! The expansion names `crate::ipc::...`, so this crate is for `crates/app`
//! only. At most four flags:
//!
//! - `read` or `write`: the first parameter is `&UnlockedVault` or
//!   `&mut UnlockedVault` (exactly one of the two, unless `locked_ok`),
//! - `sandbox = deny`: refuse while the sandbox is open,
//! - `locked_ok`: no vault check; the first parameter is `&AppState` or absent.
//!
//! ```ignore
//! #[ipc(read)]
//! fn list_stores(vault: &UnlockedVault) -> Result<Vec<Store>> { .. }
//! #[ipc(write, sandbox = deny)]
//! fn delete_store(vault: &mut UnlockedVault, id: String) -> Result<()> { .. }
//! #[ipc(locked_ok)]
//! fn vault_status(state: &AppState) -> Result<VaultStatus> { .. }
//! ```
//!
//! A fifth flag is refused:
//!
//! ```compile_fail
//! # use mwt_app_macros::ipc;
//! #[ipc(read, write, sandbox = deny, locked_ok, read)]
//! fn list(vault: &UnlockedVault) -> Result<()> { Ok(()) }
//! ```
//!
//! `read` and `write` together are refused:
//!
//! ```compile_fail
//! # use mwt_app_macros::ipc;
//! #[ipc(read, write)]
//! fn list(vault: &UnlockedVault) -> Result<()> { Ok(()) }
//! ```
//!
//! So is no mode at all without `locked_ok`:
//!
//! ```compile_fail
//! # use mwt_app_macros::ipc;
//! #[ipc(sandbox = deny)]
//! fn list(vault: &UnlockedVault) -> Result<()> { Ok(()) }
//! ```
//!
//! A reference of the wrong kind (`&` with `write`):
//!
//! ```compile_fail
//! # use mwt_app_macros::ipc;
//! #[ipc(write)]
//! fn change(vault: &UnlockedVault) -> Result<()> { Ok(()) }
//! ```
//!
//! An unknown flag:
//!
//! ```compile_fail
//! # use mwt_app_macros::ipc;
//! #[ipc(read, fast)]
//! fn list(vault: &UnlockedVault) -> Result<()> { Ok(()) }
//! ```
//!
//! An `async` body (commands with an `.await` are not `#[ipc]` commands):
//!
//! ```compile_fail
//! # use mwt_app_macros::ipc;
//! #[ipc(read)]
//! async fn list(vault: &UnlockedVault) -> Result<()> { Ok(()) }
//! ```

use proc_macro::TokenStream as CompilerTokens;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    Error, Expr, FnArg, GenericArgument, ItemFn, Meta, Pat, PathArguments, ReturnType, Token, Type,
    TypeReference, parse2,
};

/// How many flags `#[ipc(..)]` takes at most.
const MAX_FLAGS: usize = 4;

/// Marks a sync `fn` as an IPC command. See the crate docs.
#[proc_macro_attribute]
pub fn ipc(args: CompilerTokens, item: CompilerTokens) -> CompilerTokens {
    expand(args.into(), item.into())
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

/// Returns the item unchanged. Stands in for `#[tauri::command]` in the
/// `trybuild` pass cases, which compile without Tauri.
#[doc(hidden)]
#[proc_macro_attribute]
pub fn passthrough(_args: CompilerTokens, item: CompilerTokens) -> CompilerTokens {
    item
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Read,
    Write,
    LockedOk,
}

impl Mode {
    /// The `crate::ipc::run` function that runs this mode.
    fn runner(self) -> Ident {
        match self {
            Self::Read => format_ident!("read"),
            Self::Write => format_ident!("write"),
            Self::LockedOk => format_ident!("locked_ok"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Flags {
    mode: Mode,
    sandbox_deny: bool,
}

/// Reads the mode and the sandbox gate from the attribute arguments.
fn parse_flags(args: TokenStream) -> Result<Flags, Error> {
    let metas = syn::parse::Parser::parse2(
        Punctuated::<Meta, Token![,]>::parse_terminated,
        args.clone(),
    )?;
    if metas.len() > MAX_FLAGS {
        return Err(Error::new_spanned(
            args,
            "#[ipc] takes at most four flags: read | write, sandbox = deny, locked_ok",
        ));
    }
    let mut read = false;
    let mut write = false;
    let mut locked_ok = false;
    let mut sandbox_deny = false;
    for meta in &metas {
        let seen = match meta {
            Meta::Path(path) if path.is_ident("read") => &mut read,
            Meta::Path(path) if path.is_ident("write") => &mut write,
            Meta::Path(path) if path.is_ident("locked_ok") => &mut locked_ok,
            Meta::NameValue(pair) if pair.path.is_ident("sandbox") && is_deny(&pair.value) => {
                &mut sandbox_deny
            }
            _ => {
                return Err(Error::new_spanned(
                    meta,
                    "unknown #[ipc] flag; expected read, write, sandbox = deny or locked_ok",
                ));
            }
        };
        if *seen {
            return Err(Error::new_spanned(meta, "duplicate #[ipc] flag"));
        }
        *seen = true;
    }
    let mode = match (read, write, locked_ok) {
        (true, false, false) => Mode::Read,
        (false, true, false) => Mode::Write,
        (false, false, true) => Mode::LockedOk,
        (false, false, false) => {
            return Err(Error::new(
                proc_macro2::Span::call_site(),
                "#[ipc] needs read, write or locked_ok",
            ));
        }
        _ => {
            return Err(Error::new_spanned(
                metas,
                "#[ipc] takes exactly one of read, write; locked_ok replaces both",
            ));
        }
    };
    Ok(Flags { mode, sandbox_deny })
}

fn is_deny(value: &Expr) -> bool {
    matches!(value, Expr::Path(path) if path.path.is_ident("deny"))
}

/// `&T` or `&mut T` whose `T` is a path ending in `name`.
fn is_reference_to(ty: &Type, name: &str, mutable: bool) -> bool {
    let Type::Reference(TypeReference {
        mutability, elem, ..
    }) = ty
    else {
        return false;
    };
    mutability.is_some() == mutable
        && matches!(&**elem, Type::Path(path)
            if path.path.segments.last().is_some_and(|segment| segment.ident == name))
}

/// One body parameter that is a command argument: its name and type.
struct Argument {
    name: Ident,
    ty: Type,
}

/// The body's parameters split by role.
struct Signature {
    /// The `&UnlockedVault` / `&mut UnlockedVault` / `&AppState` parameter.
    first: Option<Type>,
    /// A `&AppHandle` parameter, forwarded from the command's `app`.
    handle: bool,
    arguments: Vec<Argument>,
}

fn parse_signature(function: &ItemFn, mode: Mode) -> Result<Signature, Error> {
    let mut params = function.sig.inputs.iter().peekable();
    let mut first = None;
    if let Some(FnArg::Typed(param)) = params.peek() {
        let ty = &*param.ty;
        let wanted = match mode {
            Mode::Read => Some(is_reference_to(ty, "UnlockedVault", false)),
            Mode::Write => Some(is_reference_to(ty, "UnlockedVault", true)),
            Mode::LockedOk => is_reference_to(ty, "AppState", false).then_some(true),
        };
        match wanted {
            Some(true) => {
                first = Some(ty.clone());
                params.next();
            }
            Some(false) => {
                let expected = if mode == Mode::Write {
                    "&mut UnlockedVault"
                } else {
                    "&UnlockedVault"
                };
                return Err(Error::new_spanned(
                    ty,
                    format!("the first parameter of this #[ipc] command must be `{expected}`"),
                ));
            }
            None => {}
        }
    }
    if first.is_none() && mode != Mode::LockedOk {
        return Err(Error::new_spanned(
            &function.sig,
            "the first parameter of a read or write #[ipc] command must be the vault",
        ));
    }
    let mut handle = false;
    let mut arguments = Vec::new();
    for param in params {
        let FnArg::Typed(typed) = param else {
            return Err(Error::new_spanned(param, "#[ipc] commands take no `self`"));
        };
        let Pat::Ident(pattern) = &*typed.pat else {
            return Err(Error::new_spanned(
                &typed.pat,
                "#[ipc] arguments must be plain names: Tauri names the JS keys from them",
            ));
        };
        if is_reference_to(&typed.ty, "AppHandle", false) && pattern.ident == "app" {
            handle = true;
        } else {
            arguments.push(Argument {
                name: pattern.ident.clone(),
                ty: (*typed.ty).clone(),
            });
        }
    }
    Ok(Signature {
        first,
        handle,
        arguments,
    })
}

/// `T` of a `Result<T, ..>` return type.
fn ok_type(function: &ItemFn) -> Result<Type, Error> {
    let ReturnType::Type(_, ty) = &function.sig.output else {
        return Err(Error::new_spanned(
            &function.sig,
            "an #[ipc] body returns Result<T, E>",
        ));
    };
    if let Type::Path(path) = &**ty
        && let Some(segment) = path.path.segments.last()
        && segment.ident == "Result"
        && let PathArguments::AngleBracketed(generics) = &segment.arguments
        && let Some(GenericArgument::Type(ok)) = generics.args.first()
    {
        return Ok(ok.clone());
    }
    Err(Error::new_spanned(
        ty,
        "an #[ipc] body returns Result<T, E>",
    ))
}

/// A function pointer printing `ty` as TS through ts-rs.
fn ts_printer(ty: &Type) -> TokenStream {
    quote! { || <#ty as ts_rs::TS>::name(&ts_rs::Config::new()) }
}

/// Builds the command from the attribute arguments and the body `fn`.
fn expand(args: TokenStream, item: TokenStream) -> Result<TokenStream, Error> {
    let flags = parse_flags(args)?;
    let function: ItemFn = parse2(item)?;
    if let Some(token) = function.sig.asyncness {
        return Err(Error::new(
            token.span(),
            "an #[ipc] body is a plain sync fn; a command with `.await` stays a plain #[tauri::command]",
        ));
    }
    let signature = parse_signature(&function, flags.mode)?;
    let ok = ok_type(&function)?;

    let name = &function.sig.ident;
    let meta_name = format_ident!("__IPC_META_{}", name);
    let attrs = &function.attrs;
    let runner = flags.mode.runner();
    let sandbox_deny = flags.sandbox_deny;

    let names: Vec<&Ident> = signature.arguments.iter().map(|arg| &arg.name).collect();
    let types: Vec<&Type> = signature.arguments.iter().map(|arg| &arg.ty).collect();
    let printers: Vec<TokenStream> = types.iter().map(|ty| ts_printer(ty)).collect();
    let ret_printer = ts_printer(&ok);

    let mut body = function.clone();
    body.attrs.clear();
    body.vis = syn::Visibility::Inherited;
    body.sig.ident = format_ident!("body");

    let state = format_ident!(
        "{}",
        if flags.mode == Mode::LockedOk {
            "state"
        } else {
            "vault"
        }
    );
    let mut call_args = Vec::new();
    if signature.first.is_some() {
        call_args.push(quote! { #state });
    }
    let handle_clone = signature
        .handle
        .then(|| quote! { let handle = app.clone(); });
    if signature.handle {
        call_args.push(quote! { &handle });
    }
    call_args.extend(names.iter().map(|arg| quote! { #arg }));
    let closure_param = if signature.first.is_some() {
        quote! { #state }
    } else {
        quote! { _ }
    };

    Ok(quote! {
        #(#attrs)*
        #[tauri::command]
        pub async fn #name(
            app: tauri::AppHandle,
            #( #names: #types ),*
        ) -> Result<#ok, crate::ipc::IpcError> {
            #body
            #handle_clone
            crate::ipc::run::#runner(
                app,
                &#meta_name,
                crate::ipc::run::Gate { sandbox_deny: #sandbox_deny },
                move |#closure_param| body(#(#call_args),*).map_err(Into::into),
            )
            .await
        }

        #[allow(non_upper_case_globals)]
        pub const #meta_name: crate::ipc::run::CommandMeta = crate::ipc::run::CommandMeta {
            name: stringify!(#name),
            args: &[
                #( crate::ipc::run::ArgMeta { name: stringify!(#names), ts: #printers } ),*
            ],
            ret: #ret_printer,
        };
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expanded(args: &str, item: &str) -> String {
        expand(args.parse().unwrap(), item.parse().unwrap())
            .unwrap()
            .to_string()
    }

    fn error_of(args: &str, item: &str) -> String {
        expand(args.parse().unwrap(), item.parse().unwrap())
            .unwrap_err()
            .to_string()
    }

    const READ_ITEM: &str =
        "fn list(vault: &UnlockedVault, id: String) -> Result<Vec<Store>> { todo!() }";

    #[test]
    fn read_expands_to_a_tauri_command_through_run_read() {
        let out = expanded("read", READ_ITEM);
        assert!(out.contains("# [tauri :: command]"));
        assert!(out.contains(
            "pub async fn list (app : tauri :: AppHandle , id : String) -> Result < Vec < Store > , crate :: ipc :: IpcError >"
        ));
        assert!(out.contains("crate :: ipc :: run :: read (app"));
        assert!(out.contains("sandbox_deny : false"));
        assert!(out.contains("move | vault | body (vault , id) . map_err (Into :: into)"));
        assert!(out.contains("pub const __IPC_META_list : crate :: ipc :: run :: CommandMeta"));
        assert!(out.contains("name : stringify ! (id)"));
    }

    #[test]
    fn write_expands_through_run_write() {
        let out = expanded(
            "write",
            "fn change(vault: &mut UnlockedVault) -> Result<()> { todo!() }",
        );
        assert!(out.contains("crate :: ipc :: run :: write (app"));
        assert!(out.contains("sandbox_deny : false"));
    }

    #[test]
    fn write_with_sandbox_deny_sets_the_gate() {
        let out = expanded(
            "write, sandbox = deny",
            "fn change(vault: &mut UnlockedVault) -> Result<()> { todo!() }",
        );
        assert!(out.contains("crate :: ipc :: run :: write (app"));
        assert!(out.contains("sandbox_deny : true"));
    }

    #[test]
    fn locked_ok_passes_the_state_and_forwards_the_app_handle() {
        let out = expanded(
            "locked_ok",
            "fn status(state: &AppState, app: &AppHandle, key: Key) -> Result<Status> { todo!() }",
        );
        assert!(out.contains("crate :: ipc :: run :: locked_ok (app"));
        assert!(out.contains("let handle = app . clone ()"));
        assert!(out.contains("move | state | body (state , & handle , key)"));
        assert!(out.contains("pub async fn status (app : tauri :: AppHandle , key : Key)"));
    }

    #[test]
    fn locked_ok_may_take_no_state() {
        let out = expanded("locked_ok", "fn ping() -> Result<()> { todo!() }");
        assert!(out.contains("move | _ | body () . map_err"));
    }

    #[test]
    fn both_read_and_write_are_refused() {
        assert!(error_of("read, write", READ_ITEM).contains("exactly one"));
    }

    #[test]
    fn no_mode_without_locked_ok_is_refused() {
        assert!(error_of("sandbox = deny", READ_ITEM).contains("needs read, write or locked_ok"));
        assert!(error_of("", READ_ITEM).contains("needs read, write or locked_ok"));
    }

    #[test]
    fn a_reference_of_the_wrong_kind_is_refused() {
        let item = "fn change(vault: &UnlockedVault) -> Result<()> { todo!() }";
        assert!(error_of("write", item).contains("&mut UnlockedVault"));
        let item = "fn list(vault: &mut UnlockedVault) -> Result<()> { todo!() }";
        assert!(error_of("read", item).contains("`&UnlockedVault`"));
    }

    #[test]
    fn an_unknown_flag_is_refused() {
        assert!(error_of("read, fast", READ_ITEM).contains("unknown #[ipc] flag"));
        assert!(error_of("read, sandbox = allow", READ_ITEM).contains("unknown #[ipc] flag"));
    }

    #[test]
    fn an_async_body_is_refused() {
        let item = "async fn list(vault: &UnlockedVault) -> Result<()> { todo!() }";
        assert!(error_of("read", item).contains("plain sync fn"));
    }

    #[test]
    fn a_fifth_flag_is_refused() {
        assert!(
            error_of("read, write, sandbox = deny, locked_ok, read", READ_ITEM)
                .contains("at most four")
        );
    }

    #[test]
    fn a_duplicate_flag_is_refused() {
        assert!(error_of("read, read", READ_ITEM).contains("duplicate"));
    }
}
