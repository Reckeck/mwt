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
//! A well-formed command compiles (the hidden prelude stubs `tauri` and `crate::ipc`):
//!
//! ```
//! # use mwt_app_macros::ipc;
//! # include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/stub/ipc_stub.rs"));
//! #[ipc(read)]
//! fn list(_vault: &UnlockedVault, id: String) -> Result<String, ipc::IpcError> { Ok(id) }
//! # fn main() { assert_eq!(__IPC_META_list.name, "list"); }
//! ```
//!
//! A fifth flag is refused:
//!
//! ```compile_fail
//! # use mwt_app_macros::ipc;
//! # include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/stub/ipc_stub.rs"));
//! #[ipc(read, write, sandbox = deny, locked_ok, read)]
//! fn list(vault: &UnlockedVault) -> Result<(), ipc::IpcError> { Ok(()) }
//! # fn main() {}
//! ```
//!
//! `read` and `write` together are refused:
//!
//! ```compile_fail
//! # use mwt_app_macros::ipc;
//! # include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/stub/ipc_stub.rs"));
//! #[ipc(read, write)]
//! fn list(vault: &UnlockedVault) -> Result<(), ipc::IpcError> { Ok(()) }
//! # fn main() {}
//! ```
//!
//! So is no mode at all without `locked_ok`:
//!
//! ```compile_fail
//! # use mwt_app_macros::ipc;
//! # include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/stub/ipc_stub.rs"));
//! #[ipc(sandbox = deny)]
//! fn list(vault: &UnlockedVault) -> Result<(), ipc::IpcError> { Ok(()) }
//! # fn main() {}
//! ```
//!
//! A reference of the wrong kind (`&` with `write`):
//!
//! ```compile_fail
//! # use mwt_app_macros::ipc;
//! # include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/stub/ipc_stub.rs"));
//! #[ipc(write)]
//! fn change(vault: &UnlockedVault) -> Result<(), ipc::IpcError> { Ok(()) }
//! # fn main() {}
//! ```
//!
//! An unknown flag:
//!
//! ```compile_fail
//! # use mwt_app_macros::ipc;
//! # include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/stub/ipc_stub.rs"));
//! #[ipc(read, fast)]
//! fn list(vault: &UnlockedVault) -> Result<(), ipc::IpcError> { Ok(()) }
//! # fn main() {}
//! ```
//!
//! An `async` body (commands with an `.await` are not `#[ipc]` commands):
//!
//! ```compile_fail
//! # use mwt_app_macros::ipc;
//! # include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/stub/ipc_stub.rs"));
//! #[ipc(read)]
//! async fn list(vault: &UnlockedVault) -> Result<(), ipc::IpcError> { Ok(()) }
//! ```

use proc_macro::TokenStream as CompilerTokens;
use proc_macro2::{Ident, TokenStream};
use quote::{ToTokens, format_ident, quote};
use std::iter::Peekable;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    Attribute, Error, Expr, FnArg, GenericArgument, ItemFn, Meta, Pat, PathArguments, ReturnType,
    Token, Type, TypeReference, parse2,
};

/// How many flags `#[ipc(..)]` takes at most.
const MAX_FLAGS: usize = 4;

/// Identifiers the expansion declares itself; no command argument may use them.
const HOST: &str = "__ipc_host";
const BODY: &str = "__ipc_body";
const HANDLE: &str = "__ipc_handle";
const RESERVED: [&str; 3] = [HOST, BODY, HANDLE];

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

    /// The name the gate's closure gives the first body parameter.
    fn body_param(self) -> Ident {
        match self {
            Self::LockedOk => format_ident!("state"),
            Self::Read | Self::Write => format_ident!("vault"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Flags {
    mode: Mode,
    sandbox_deny: bool,
}

/// Which flags `#[ipc(..)]` has named so far.
#[derive(Default)]
struct Seen {
    read: bool,
    write: bool,
    locked_ok: bool,
    sandbox_deny: bool,
}

impl Seen {
    /// Marks `meta` as seen; unknown and repeated flags are errors.
    fn record(&mut self, meta: &Meta) -> Result<(), Error> {
        let seen = match meta {
            Meta::Path(path) if path.is_ident("read") => &mut self.read,
            Meta::Path(path) if path.is_ident("write") => &mut self.write,
            Meta::Path(path) if path.is_ident("locked_ok") => &mut self.locked_ok,
            Meta::NameValue(pair) if pair.path.is_ident("sandbox") && is_deny(&pair.value) => {
                &mut self.sandbox_deny
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
        Ok(())
    }

    /// The one mode the flags name.
    fn mode(&self, metas: &Punctuated<Meta, Token![,]>) -> Result<Mode, Error> {
        match (self.read, self.write, self.locked_ok) {
            (true, false, false) => Ok(Mode::Read),
            (false, true, false) => Ok(Mode::Write),
            (false, false, true) => Ok(Mode::LockedOk),
            (false, false, false) => Err(Error::new(
                proc_macro2::Span::call_site(),
                "#[ipc] needs read, write or locked_ok",
            )),
            _ => Err(Error::new_spanned(
                metas,
                "#[ipc] takes exactly one of read, write; locked_ok replaces both",
            )),
        }
    }
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
    let mut seen = Seen::default();
    for meta in &metas {
        seen.record(meta)?;
    }
    Ok(Flags {
        mode: seen.mode(&metas)?,
        sandbox_deny: seen.sandbox_deny,
    })
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

/// Takes the leading vault/state parameter off `params` when `mode` wants one.
fn take_first<'a>(
    params: &mut Peekable<impl Iterator<Item = &'a FnArg>>,
    function: &ItemFn,
    mode: Mode,
) -> Result<Option<Type>, Error> {
    let first = match params.peek() {
        Some(FnArg::Typed(param)) => first_matches(&param.ty, mode)?.then(|| (*param.ty).clone()),
        _ => None,
    };
    if first.is_some() {
        params.next();
    } else if mode != Mode::LockedOk {
        return Err(Error::new_spanned(
            &function.sig,
            "the first parameter of a read or write #[ipc] command must be the vault",
        ));
    }
    Ok(first)
}

/// Whether `ty` is the first parameter `mode` wants; a wrong vault kind is an error.
fn first_matches(ty: &Type, mode: Mode) -> Result<bool, Error> {
    let (expected, mutable) = match mode {
        Mode::Read => ("&UnlockedVault", false),
        Mode::Write => ("&mut UnlockedVault", true),
        Mode::LockedOk => return Ok(is_reference_to(ty, "AppState", false)),
    };
    if is_reference_to(ty, "UnlockedVault", mutable) {
        return Ok(true);
    }
    Err(Error::new_spanned(
        ty,
        format!("the first parameter of this #[ipc] command must be `{expected}`"),
    ))
}

/// The plain name of a command parameter, checked against the reserved ones.
fn param_ident(param: &FnArg) -> Result<(&Ident, &Type), Error> {
    let FnArg::Typed(typed) = param else {
        return Err(Error::new_spanned(param, "#[ipc] commands take no `self`"));
    };
    let Pat::Ident(pattern) = &*typed.pat else {
        return Err(Error::new_spanned(
            &typed.pat,
            "#[ipc] arguments must be plain names: Tauri names the JS keys from them",
        ));
    };
    if RESERVED.iter().any(|reserved| pattern.ident == reserved) {
        return Err(Error::new_spanned(
            &pattern.ident,
            format!(
                "`{}` is reserved by #[ipc]; rename this argument",
                pattern.ident
            ),
        ));
    }
    Ok((&pattern.ident, &typed.ty))
}

fn parse_signature(function: &ItemFn, mode: Mode) -> Result<Signature, Error> {
    let mut params = function.sig.inputs.iter().peekable();
    let first = take_first(&mut params, function, mode)?;
    let mut handle = false;
    let mut arguments = Vec::new();
    for param in params {
        let (name, ty) = param_ident(param)?;
        if !is_reference_to(ty, "AppHandle", false) {
            if name == "app" {
                return Err(Error::new_spanned(
                    name,
                    "`app` is reserved by #[ipc] for the &AppHandle",
                ));
            }
            arguments.push(Argument {
                name: name.clone(),
                ty: ty.clone(),
            });
        } else if name == "app" {
            handle = true;
        } else {
            return Err(Error::new_spanned(
                name,
                "a `&AppHandle` parameter of an #[ipc] command must be named `app`",
            ));
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

/// What the three generated items share.
struct Command<'a> {
    name: &'a Ident,
    runner_name: Ident,
    meta_name: Ident,
    ok: Type,
    signature: Signature,
}

impl Command<'_> {
    fn names(&self) -> Vec<&Ident> {
        self.signature
            .arguments
            .iter()
            .map(|arg| &arg.name)
            .collect()
    }

    fn types(&self) -> Vec<&Type> {
        self.signature.arguments.iter().map(|arg| &arg.ty).collect()
    }
}

/// The `#[tauri::command]` entry: forwards to the runner with the app handle.
fn command_item(command: &Command, attrs: &[Attribute]) -> TokenStream {
    let (name, ok) = (command.name, &command.ok);
    let (names, types) = (command.names(), command.types());
    let runner_name = &command.runner_name;
    quote! {
        #(#attrs)*
        #[tauri::command]
        pub async fn #name(
            app: tauri::AppHandle,
            #( #names: #types ),*
        ) -> Result<#ok, crate::ipc::IpcError> {
            #runner_name(app, #( #names ),*).await
        }
    }
}

/// The body `fn` renamed to `__ipc_body`, private and without attributes.
fn body_fn(function: &ItemFn) -> ItemFn {
    let mut body = function.clone();
    body.attrs.clear();
    body.vis = syn::Visibility::Inherited;
    body.sig.ident = format_ident!("{}", BODY);
    body
}

/// The arguments the gate's closure passes to the body.
fn call_args(command: &Command, mode: Mode) -> Vec<TokenStream> {
    let signature = &command.signature;
    let state = mode.body_param();
    let mut call_args = Vec::new();
    if signature.first.is_some() {
        call_args.push(quote! { #state });
    }
    if signature.handle {
        let handle_name = format_ident!("{}", HANDLE);
        call_args.push(quote! { &#handle_name });
    }
    call_args.extend(command.names().into_iter().map(|arg| quote! { #arg }));
    call_args
}

/// The runner: the body behind its gate, generic over the host so tests run
/// it without a Tauri runtime.
fn runner_item(command: &Command, function: &ItemFn, flags: Flags) -> TokenStream {
    let ok = &command.ok;
    let (names, types) = (command.names(), command.types());
    let (runner_name, meta_name) = (&command.runner_name, &command.meta_name);
    let host_name = format_ident!("{}", HOST);
    let handle_name = format_ident!("{}", HANDLE);
    let body = body_fn(function);
    let body_name = &body.sig.ident;
    let runner = flags.mode.runner();
    let sandbox_deny = flags.sandbox_deny;
    let call_args = call_args(command, flags.mode);
    let (host_generics, host_type, handle_clone) = if command.signature.handle {
        (
            quote! {},
            quote! { tauri::AppHandle },
            quote! { let #handle_name = #host_name.clone(); },
        )
    } else {
        (
            quote! { <Host: crate::ipc::run::VaultHost> },
            quote! { Host },
            quote! {},
        )
    };
    let closure_param = if command.signature.first.is_some() {
        flags.mode.body_param().into_token_stream()
    } else {
        quote! { _ }
    };
    quote! {
        /// The command body behind its gate, generic over the host so tests
        /// run it without a Tauri runtime.
        pub(crate) async fn #runner_name #host_generics(
            #host_name: #host_type,
            #( #names: #types ),*
        ) -> Result<#ok, crate::ipc::IpcError> {
            #body
            #handle_clone
            crate::ipc::run::#runner(
                #host_name,
                &#meta_name,
                crate::ipc::run::Gate { sandbox_deny: #sandbox_deny },
                move |#closure_param| #body_name(#(#call_args),*).map_err(Into::into),
            )
            .await
        }
    }
}

/// The `CommandMeta` constant the TS generator reads.
fn meta_item(command: &Command) -> TokenStream {
    let name = command.name;
    let meta_name = &command.meta_name;
    let names = command.names();
    let printers: Vec<TokenStream> = command.types().into_iter().map(ts_printer).collect();
    let ret_printer = ts_printer(&command.ok);
    quote! {
        #[allow(non_upper_case_globals)]
        pub const #meta_name: crate::ipc::run::CommandMeta = crate::ipc::run::CommandMeta {
            name: stringify!(#name),
            args: &[
                #( crate::ipc::run::ArgMeta { name: stringify!(#names), #[cfg(feature = "ts")] ts: #printers } ),*
            ],
            #[cfg(feature = "ts")]
            ret: #ret_printer,
        };
    }
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
    let command = Command {
        name: &function.sig.ident,
        runner_name: format_ident!("__ipc_run_{}", function.sig.ident),
        meta_name: format_ident!("__IPC_META_{}", function.sig.ident),
        ok: ok_type(&function)?,
        signature: parse_signature(&function, flags.mode)?,
    };
    let command_tokens = command_item(&command, &function.attrs);
    let runner_tokens = runner_item(&command, &function, flags);
    let meta_tokens = meta_item(&command);
    Ok(quote! { #command_tokens #runner_tokens #meta_tokens })
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
    fn the_ts_printers_are_behind_the_ts_feature() {
        let out = expanded("read", READ_ITEM);
        assert!(out.contains("# [cfg (feature = \"ts\")] ts :"));
        assert!(out.contains("# [cfg (feature = \"ts\")] ret :"));
    }

    #[test]
    fn read_expands_to_a_tauri_command_through_run_read() {
        let out = expanded("read", READ_ITEM);
        assert!(out.contains("# [tauri :: command]"));
        assert!(out.contains(
            "pub async fn list (app : tauri :: AppHandle , id : String) -> Result < Vec < Store > , crate :: ipc :: IpcError >"
        ));
        assert!(out.contains("crate :: ipc :: run :: read (__ipc_host"));
        assert!(out.contains("pub (crate) async fn __ipc_run_list < Host : crate :: ipc :: run :: VaultHost > (__ipc_host : Host , id : String)"));
        assert!(out.contains("__ipc_run_list (app , id) . await"));
        assert!(out.contains("sandbox_deny : false"));
        assert!(out.contains("move | vault | __ipc_body (vault , id) . map_err (Into :: into)"));
        assert!(out.contains("pub const __IPC_META_list : crate :: ipc :: run :: CommandMeta"));
        assert!(out.contains("name : stringify ! (id)"));
    }

    #[test]
    fn write_expands_through_run_write() {
        let out = expanded(
            "write",
            "fn change(vault: &mut UnlockedVault) -> Result<()> { todo!() }",
        );
        assert!(out.contains("crate :: ipc :: run :: write (__ipc_host"));
        assert!(out.contains("sandbox_deny : false"));
    }

    #[test]
    fn write_with_sandbox_deny_sets_the_gate() {
        let out = expanded(
            "write, sandbox = deny",
            "fn change(vault: &mut UnlockedVault) -> Result<()> { todo!() }",
        );
        assert!(out.contains("crate :: ipc :: run :: write (__ipc_host"));
        assert!(out.contains("sandbox_deny : true"));
    }

    #[test]
    fn locked_ok_passes_the_state_and_forwards_the_app_handle() {
        let out = expanded(
            "locked_ok",
            "fn status(state: &AppState, app: &AppHandle, key: Key) -> Result<Status> { todo!() }",
        );
        assert!(out.contains("crate :: ipc :: run :: locked_ok (__ipc_host"));
        assert!(out.contains("let __ipc_handle = __ipc_host . clone ()"));
        assert!(out.contains("move | state | __ipc_body (state , & __ipc_handle , key)"));
        assert!(out.contains("pub async fn status (app : tauri :: AppHandle , key : Key)"));
    }

    #[test]
    fn locked_ok_may_take_no_state() {
        let out = expanded("locked_ok", "fn ping() -> Result<()> { todo!() }");
        assert!(out.contains("move | _ | __ipc_body () . map_err"));
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
    fn a_misnamed_app_handle_is_refused() {
        let item = "fn status(state: &AppState, handle: &AppHandle) -> Result<()> { todo!() }";
        assert!(error_of("locked_ok", item).contains("must be named `app`"));
    }

    #[test]
    fn an_app_argument_that_is_not_the_app_handle_is_refused() {
        let item = "fn list(vault: &UnlockedVault, app: String) -> Result<()> { todo!() }";
        assert!(error_of("read", item).contains("`app` is reserved by #[ipc] for the &AppHandle"));
    }

    #[test]
    fn reserved_argument_names_are_refused() {
        for reserved in RESERVED {
            let item = format!(
                "fn list(vault: &UnlockedVault, {reserved}: u8) -> Result<()> {{ todo!() }}"
            );
            assert!(error_of("read", &item).contains("is reserved by #[ipc]"));
        }
    }

    #[test]
    fn a_duplicate_flag_is_refused() {
        assert!(error_of("read, read", READ_ITEM).contains("duplicate"));
    }
}
