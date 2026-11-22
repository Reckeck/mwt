//! Which names of a source file are provably an `UnlockedVault`, for `rewire` (spec D3): parameters
//! and closure parameters typed with it, locals bound by `UnlockedVault::create/open/create_with/
//! in_memory(..)`, and locals bound by a tuple or struct pattern from a function of the same file
//! whose return type names it. Everything else is not proven; a later binding of a name shadows.

use std::collections::BTreeMap;

use syn::visit::{self, Visit};
use syn::{Expr, FnArg, ImplItemFn, ItemFn, ItemImpl, ItemStruct, Pat, ReturnType, Type};

use super::scan::{VAULT_TYPE, names_type};

/// The `UnlockedVault` constructors a local can be bound from.
const CONSTRUCTORS: [&str; 4] = ["create", "open", "create_with", "in_memory"];

/// Whether the name holds the vault or a reference to it: an owned one is passed as `&vault`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Held {
    Owned,
    Borrowed,
}

/// What a bound value is made of.
enum Shape {
    Vault(Held),
    /// The tuple positions that hold the vault.
    Tuple(Vec<(usize, Held)>),
    /// The named fields of a struct of the file that hold the vault.
    Fields(Vec<(String, Held)>),
}

pub(super) fn held_of_type(ty: &Type) -> Option<Held> {
    match ty {
        Type::Reference(reference) => {
            names_type(&reference.elem, VAULT_TYPE).then_some(Held::Borrowed)
        }
        Type::Paren(inner) => held_of_type(&inner.elem),
        Type::Group(inner) => held_of_type(&inner.elem),
        other => names_type(other, VAULT_TYPE).then_some(Held::Owned),
    }
}

/// The return types of the functions of a file and of the shared helper files, and the vault fields
/// of their structs.
#[derive(Default)]
pub(super) struct Shapes {
    structs: BTreeMap<String, Vec<(String, Held)>>,
    /// Keyed by (self type, name): a free function has no self type. `None`: the key is declared
    /// more than once, so it is not proven.
    returns: BTreeMap<(Option<String>, String), Option<Type>>,
    /// The self type of the impl being visited.
    current_impl: Option<String>,
}

/// The last segment of the self type of an impl, as `impl Name` writes it.
pub(super) fn impl_type_name(node: &ItemImpl) -> Option<String> {
    match &*node.self_ty {
        Type::Path(path) => Some(path.path.segments.last()?.ident.to_string()),
        _ => None,
    }
}

impl Shapes {
    pub(super) fn of<'file>(files: impl IntoIterator<Item = &'file syn::File>) -> Self {
        let mut shapes = Self::default();
        for file in files {
            shapes.visit_file(file);
        }
        shapes
    }

    fn note_fn(&mut self, owner: Option<String>, name: String, output: &ReturnType) {
        let ty = match output {
            ReturnType::Type(_, ty) => Some((**ty).clone()),
            ReturnType::Default => None,
        };
        let key = (owner, name);
        let known = self.returns.contains_key(&key);
        self.returns.insert(key, if known { None } else { ty });
    }

    fn resolve(&self, ty: &Type) -> Option<Shape> {
        if let Some(held) = held_of_type(ty) {
            return Some(Shape::Vault(held));
        }
        match ty {
            Type::Tuple(tuple) => {
                let held: Vec<(usize, Held)> = tuple
                    .elems
                    .iter()
                    .enumerate()
                    .filter_map(|(at, elem)| held_of_type(elem).map(|held| (at, held)))
                    .collect();
                (!held.is_empty()).then_some(Shape::Tuple(held))
            }
            Type::Path(path) => {
                let last = path.path.segments.last()?;
                if matches!(last.ident.to_string().as_str(), "Result" | "Option") {
                    let syn::PathArguments::AngleBracketed(arguments) = &last.arguments else {
                        return None;
                    };
                    let Some(syn::GenericArgument::Type(inner)) = arguments.args.first() else {
                        return None;
                    };
                    return self.resolve(inner);
                }
                self.structs
                    .get(&last.ident.to_string())
                    .map(|fields| Shape::Fields(fields.clone()))
            }
            _ => None,
        }
    }

    fn shape_of(&self, init: &Expr, self_type: Option<&str>) -> Option<Shape> {
        match init {
            Expr::Paren(inner) => self.shape_of(&inner.expr, self_type),
            Expr::Try(inner) => self.shape_of(&inner.expr, self_type),
            Expr::Await(inner) => self.shape_of(&inner.base, self_type),
            Expr::MethodCall(call) if call.method == "unwrap" || call.method == "expect" => {
                self.shape_of(&call.receiver, self_type)
            }
            Expr::Call(call) => {
                let Expr::Path(path) = &*call.func else {
                    return None;
                };
                let names: Vec<String> = path
                    .path
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect();
                // Only a single-segment call or `Self::`/`Type::` proves a callee: any longer path
                // (`other::setup()`) may name a different function of the same name.
                let key = match names.as_slice() {
                    [last] => (None, last.clone()),
                    [before, last] => {
                        let owner = if before == "Self" { self_type? } else { before };
                        if owner == VAULT_TYPE && CONSTRUCTORS.contains(&last.as_str()) {
                            return Some(Shape::Vault(Held::Owned));
                        }
                        (Some(owner.to_string()), last.clone())
                    }
                    _ => return None,
                };
                self.resolve(self.returns.get(&key)?.as_ref()?)
            }
            _ => None,
        }
    }
}

impl<'ast> Visit<'ast> for Shapes {
    fn visit_item_struct(&mut self, node: &'ast ItemStruct) {
        let fields = node
            .fields
            .iter()
            .filter_map(|field| {
                let name = field.ident.as_ref()?.to_string();
                Some((name, held_of_type(&field.ty)?))
            })
            .collect();
        self.structs.insert(node.ident.to_string(), fields);
        visit::visit_item_struct(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        self.note_fn(None, node.sig.ident.to_string(), &node.sig.output);
        visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast ImplItemFn) {
        let owner = self.current_impl.clone();
        self.note_fn(owner, node.sig.ident.to_string(), &node.sig.output);
        visit::visit_impl_item_fn(self, node);
    }

    fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
        let outer = std::mem::replace(&mut self.current_impl, impl_type_name(node));
        visit::visit_item_impl(self, node);
        self.current_impl = outer;
    }
}

/// Every name a pattern binds.
fn pattern_names(pat: &Pat, names: &mut Vec<String>) {
    match pat {
        Pat::Ident(ident) => names.push(ident.ident.to_string()),
        Pat::Type(typed) => pattern_names(&typed.pat, names),
        Pat::Reference(reference) => pattern_names(&reference.pat, names),
        Pat::Paren(inner) => pattern_names(&inner.pat, names),
        Pat::Tuple(tuple) => tuple
            .elems
            .iter()
            .for_each(|elem| pattern_names(elem, names)),
        Pat::TupleStruct(tuple) => tuple
            .elems
            .iter()
            .for_each(|elem| pattern_names(elem, names)),
        Pat::Slice(slice) => slice
            .elems
            .iter()
            .for_each(|elem| pattern_names(elem, names)),
        Pat::Or(or) => or.cases.iter().for_each(|case| pattern_names(case, names)),
        Pat::Struct(node) => node
            .fields
            .iter()
            .for_each(|field| pattern_names(&field.pat, names)),
        _ => {}
    }
}

fn ident_of(pat: &Pat) -> Option<String> {
    match pat {
        Pat::Ident(ident) if ident.subpat.is_none() => Some(ident.ident.to_string()),
        Pat::Type(typed) => ident_of(&typed.pat),
        _ => None,
    }
}

/// The names a pattern binds to the vault, given what its value is made of.
fn proven_names(pat: &Pat, shape: &Shape) -> Vec<(String, Held)> {
    match (shape, pat) {
        (_, Pat::Type(typed)) => proven_names(&typed.pat, shape),
        (Shape::Vault(held), _) => ident_of(pat)
            .map(|name| (name, *held))
            .into_iter()
            .collect(),
        (Shape::Tuple(held), Pat::Tuple(tuple)) => {
            let rest = tuple
                .elems
                .iter()
                .position(|elem| matches!(elem, Pat::Rest(_)));
            held.iter()
                .filter(|(at, _)| rest.is_none_or(|rest| *at < rest))
                .filter_map(|(at, held)| Some((ident_of(tuple.elems.get(*at)?)?, *held)))
                .collect()
        }
        (Shape::Fields(held), Pat::Struct(node)) => node
            .fields
            .iter()
            .filter_map(|field| {
                let syn::Member::Named(member) = &field.member else {
                    return None;
                };
                let (_, held) = held.iter().find(|(name, _)| member == name)?;
                Some((ident_of(&field.pat)?, *held))
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The names in scope, innermost last; `None` marks a name that is not the vault.
#[derive(Default)]
pub(super) struct Bindings {
    entries: Vec<(String, Option<Held>)>,
}

impl Bindings {
    pub(super) fn mark(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn reset(&mut self, mark: usize) {
        self.entries.truncate(mark);
    }

    /// A fresh scope for an item: a nested `fn` sees no local of the one around it.
    pub(super) fn take(&mut self) -> Vec<(String, Option<Held>)> {
        std::mem::take(&mut self.entries)
    }

    pub(super) fn restore(&mut self, entries: Vec<(String, Option<Held>)>) {
        self.entries = entries;
    }

    pub(super) fn held(&self, name: &str) -> Option<Held> {
        self.entries
            .iter()
            .rev()
            .find(|(bound, _)| bound == name)
            .and_then(|(_, held)| *held)
    }

    /// A pattern that binds names which are not the vault (shadows earlier ones).
    pub(super) fn shadow(&mut self, pat: &Pat) {
        let mut names = Vec::new();
        pattern_names(pat, &mut names);
        self.entries
            .extend(names.into_iter().map(|name| (name, None)));
    }

    /// A parameter or closure parameter: the vault when typed so, else a shadow.
    pub(super) fn bind_typed(&mut self, pat: &Pat, ty: Option<&Type>) {
        self.shadow(pat);
        if let (Some(name), Some(held)) = (ident_of(pat), ty.and_then(held_of_type)) {
            self.entries.push((name, Some(held)));
        }
    }

    pub(super) fn bind_inputs<'ast>(&mut self, inputs: impl IntoIterator<Item = &'ast FnArg>) {
        for input in inputs {
            if let FnArg::Typed(typed) = input {
                self.bind_typed(&typed.pat, Some(&typed.ty));
            }
        }
    }

    /// A `let`: the names the initializer proves, the rest shadow.
    pub(super) fn bind_let(
        &mut self,
        pat: &Pat,
        init: Option<&Expr>,
        shapes: &Shapes,
        self_type: Option<&str>,
    ) {
        self.shadow(pat);
        let declared = match pat {
            Pat::Type(typed) => held_of_type(&typed.ty),
            _ => None,
        };
        let proven = match (
            declared,
            init.and_then(|init| shapes.shape_of(init, self_type)),
        ) {
            (Some(held), _) => ident_of(pat).map(|name| (name, held)).into_iter().collect(),
            (None, Some(shape)) => proven_names(pat, &shape),
            (None, None) => Vec::new(),
        };
        self.entries
            .extend(proven.into_iter().map(|(name, held)| (name, Some(held))));
    }
}
