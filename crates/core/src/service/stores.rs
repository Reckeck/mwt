use super::*;

/// Allowed destinations for moving a sub-account.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct StoreMoveTargets {
    /// Flat (no `children`), in tree order; the current parent is left out.
    pub parents: Vec<StoreTreeNode>,
    /// Whether it may become a top-level wallet (no root has its name).
    pub can_detach: bool,
}

/// Depth-first flat order without `children`: parent, then its subtree.
/// Sub-account defaults for [`UnlockedVault::create_store`]: the kind's first
/// template, bound to a market, or to USD for currency accounts and BTC for
/// tokens. Real callers pass type and binding explicitly.
pub(crate) fn default_sub_extras(kind: StoreKind) -> StoreExtras {
    let sub_type = kind.sub_types().first().copied();
    let binding = sub_type.map(|sub_type| match sub_type.binding_shape() {
        BindingShape::Market(market) => Binding::Market(market),
        BindingShape::Pair => Binding::Pair {
            base: AssetRef::crypto("BTC", "Bitcoin", Some("bitcoin"), Chain::Bitcoin),
            quote: AssetRef::fiat("USD", "US Dollar"),
        },
        BindingShape::Asset if sub_type == SubType::Token => Binding::Asset(AssetRef::crypto(
            "BTC",
            "Bitcoin",
            Some("bitcoin"),
            Chain::Bitcoin,
        )),
        BindingShape::Asset => Binding::Asset(AssetRef::fiat("USD", "US Dollar")),
    });
    StoreExtras {
        sub_type,
        binding,
        ..StoreExtras::default()
    }
}

/// The top-up that records a new wallet's opening balance.
pub(crate) fn opening_topup(store_id: &str, opening: OpeningBalance) -> Transaction {
    let mut meta = TxMeta::new("Opening balance");
    meta.categories = vec!["opening_balance".to_string()];
    if let Some(at) = opening.at.filter(|at| !at.trim().is_empty()) {
        meta.timestamp = at;
    }
    Transaction::Topup(crate::model::transaction::TopupTx {
        meta,
        store_id: store_id.to_string(),
        asset: opening.asset,
        amount: opening.amount,
        fee: None,
        from: None,
        details: crate::model::transaction::KindDetails::default(),
        price: PriceStamp::default(),
    })
}

pub(crate) fn flatten_tree(nodes: Vec<StoreTreeNode>) -> Vec<StoreTreeNode> {
    let mut out = Vec::new();
    for mut node in nodes {
        let children = std::mem::take(&mut node.children);
        out.push(node);
        out.extend(flatten_tree(children));
    }
    out
}

impl UnlockedVault {
    pub(crate) fn find_store(&self, id: &str) -> Option<&Store> {
        crate::money::tree::find_store(&self.doc, id)
    }

    /// Parent chain from the root down to `id`'s parent (see [`crate::money::tree::ancestors_of`]).
    pub(crate) fn ancestors_of(&self, id: &str) -> Vec<&Store> {
        crate::money::tree::ancestors_of(&self.doc, id)
    }

    /// 0 for a top-level wallet.
    pub fn depth(&self, id: &str) -> usize {
        crate::money::tree::depth(&self.doc, id)
    }

    /// Top-level wallet that `id` belongs to (itself for a root).
    pub fn root_of(&self, id: &str) -> String {
        crate::money::tree::root_of(&self.doc, id)
    }

    /// Levels below `id`: 0 for a leaf.
    pub(crate) fn subtree_height(&self, id: &str) -> usize {
        let mut seen = HashSet::from([id.to_string()]);
        let mut level = vec![id.to_string()];
        let mut height = 0;
        loop {
            let next: Vec<String> = level
                .iter()
                .flat_map(|parent| self.children_of(parent))
                .filter(|child| seen.insert(child.id.clone()))
                .map(|child| child.id.clone())
                .collect();
            if next.is_empty() {
                return height;
            }
            height += 1;
            level = next;
        }
    }

    /// R5: names are unique among siblings (roots among roots), ignoring case
    /// and surrounding spaces. `node` is skipped so a rename to itself passes.
    pub(crate) fn ensure_unique_name(
        &self,
        node: Option<&str>,
        parent: Option<&str>,
        name: &str,
    ) -> Result<()> {
        let wanted = name.trim().to_lowercase();
        let taken = self.doc.stores.iter().any(|store| {
            node != Some(store.id.as_str())
                && store.parent_id.as_deref() == parent
                && store.name.trim().to_lowercase() == wanted
        });
        if taken {
            return Err(Error::StoreTree(StoreTreeViolation::NameTaken));
        }
        Ok(())
    }

    /// Whether `node` (an existing store, or `None` for a new one) may sit
    /// under `parent` with `name`. Rules R1–R6.
    pub(crate) fn validate_placement(
        &self,
        node: Option<&str>,
        parent: Option<&str>,
        name: &str,
    ) -> Result<()> {
        if name.trim().is_empty() {
            return Err(Error::InvalidInput("store name must not be empty".into()));
        }
        if let Some(parent_id) = parent {
            let parent_store = self
                .find_store(parent_id)
                .ok_or_else(|| Error::Missing("parent store".into()))?;
            if let Some(node_id) = node {
                if self
                    .find_store(node_id)
                    .is_some_and(|store| store.parent_id.is_none())
                {
                    return Err(Error::StoreTree(StoreTreeViolation::RootMove));
                }
                if self
                    .find_store(node_id)
                    .is_some_and(|store| store.kind != parent_store.kind)
                {
                    return Err(Error::WalletRule(WalletRule::SubTypeNotAllowed));
                }
                if parent_id == node_id
                    || self.subtree_ids(node_id).iter().any(|id| id == parent_id)
                {
                    return Err(Error::StoreTree(StoreTreeViolation::Cycle));
                }
            }
            let height = node.map_or(0, |id| self.subtree_height(id));
            if self.depth(parent_id) + 1 + height > MAX_STORE_DEPTH {
                return Err(Error::StoreTree(StoreTreeViolation::TooDeep));
            }
            if parent_store.archived {
                return Err(Error::StoreTree(StoreTreeViolation::ParentArchived));
            }
        }
        // A typed sub-account cannot become a top-level wallet.
        if parent.is_none()
            && node
                .and_then(|id| self.find_store(id))
                .is_some_and(|store| store.parent_id.is_some())
        {
            return Err(Error::WalletRule(WalletRule::NotApplicable));
        }
        self.ensure_unique_name(node, parent, name)
    }

    pub fn create_store(
        &mut self,
        name: &str,
        kind: StoreKind,
        color: &str,
        note: &str,
        parent_id: Option<String>,
    ) -> Result<Store> {
        let extras = match parent_id.as_deref().and_then(|id| self.find_store(id)) {
            Some(parent) => default_sub_extras(parent.kind),
            None => StoreExtras::default(),
        };
        self.create_store_with(name, kind, color, note, parent_id, extras)
    }

    /// Which extras a wallet of this kind and level may carry, shared by
    /// create and update.
    pub(crate) fn check_store_extras(
        &self,
        kind: StoreKind,
        sub_type: Option<SubType>,
        is_root: bool,
        extras: &StoreExtras,
    ) -> Result<()> {
        let credit = sub_type.is_some_and(SubType::is_credit);
        if extras.liability.is_some() && kind != StoreKind::Liability && !credit {
            return Err(Error::WalletRule(WalletRule::NotApplicable));
        }
        if let Some(terms) = &extras.liability
            && terms.due_day.is_some_and(|day| !(1..=31).contains(&day))
        {
            return Err(Error::InvalidInput("the payment day must be 1–31".into()));
        }
        let misplaced = (extras.custody.is_some() && !(is_root && kind == StoreKind::Crypto))
            || (extras.property.is_some() && !(is_root && kind == StoreKind::Property))
            || (extras.provider.is_some() && !(is_root && kind == StoreKind::Exchange))
            || (extras.secured_by.is_some() && kind != StoreKind::Liability);
        if misplaced {
            return Err(Error::WalletRule(WalletRule::NotApplicable));
        }
        if let Some(target) = &extras.secured_by
            && !self
                .find_store(target)
                .is_some_and(|store| store.kind == StoreKind::Property && store.parent_id.is_none())
        {
            return Err(Error::Missing(format!("property {target}")));
        }
        if let Some(contact_id) = &extras.contact_id {
            if !is_root {
                return Err(Error::InvalidInput(
                    "a contact is set on the top-level wallet; sub-accounts share it".into(),
                ));
            }
            if !self
                .doc
                .contacts
                .iter()
                .any(|contact| &contact.id == contact_id)
            {
                return Err(Error::Missing(format!("contact {contact_id}")));
            }
        }
        Ok(())
    }

    /// A sub-account's type must be offered by its wallet kind and its
    /// binding must have the shape the type expects.
    pub(crate) fn check_sub_account(
        kind: StoreKind,
        sub_type: Option<SubType>,
        binding: Option<&Binding>,
    ) -> Result<(SubType, Binding)> {
        let sub_type = sub_type.ok_or(Error::WalletRule(WalletRule::BindingRequired))?;
        if !kind.admits_sub_type(sub_type) {
            return Err(Error::WalletRule(WalletRule::SubTypeNotAllowed));
        }
        let binding = match (sub_type.binding_shape(), binding) {
            (BindingShape::Market(market), None | Some(Binding::Any)) => Binding::Market(market),
            (_, None | Some(Binding::Any)) => {
                return Err(Error::WalletRule(WalletRule::BindingRequired));
            }
            (shape, Some(binding)) if binding.shape() == Some(shape) => binding.clone(),
            _ => return Err(Error::WalletRule(WalletRule::BindingMismatch)),
        };
        if let Binding::Pair { base, quote } = &binding
            && base.key() == quote.key()
        {
            return Err(Error::InvalidInput(
                "a pair needs two different assets".into(),
            ));
        }
        Ok((sub_type, binding))
    }

    /// The vault always has exactly one cash wallet; called on create and
    /// open. Returns whether one had to be added.
    pub fn ensure_cash_wallet(&mut self, name: &str) -> Result<bool> {
        let has_cash = self
            .doc
            .stores
            .iter()
            .any(|store| store.kind == StoreKind::Cash && store.parent_id.is_none());
        if has_cash {
            return Ok(false);
        }
        let name = if name.trim().is_empty() { "Cash" } else { name };
        let mut store = Store::new(name, StoreKind::Cash, "#16a34a");
        // A fixed id: two vaults made apart meet in one record, and an old
        // random-id wallet is folded into this one by the repair pass (R1).
        store.id = CASH_WALLET_ID.to_string();
        if self.ensure_unique_name(None, None, &store.name).is_err() {
            store.name = format!("{} 2", store.name);
        }
        self.doc.stores.insert(0, store);
        self.persist()?;
        Ok(true)
    }

    pub fn create_store_with(
        &mut self,
        name: &str,
        kind: StoreKind,
        color: &str,
        note: &str,
        parent_id: Option<String>,
        extras: StoreExtras,
    ) -> Result<Store> {
        self.validate_placement(None, parent_id.as_deref(), name)?;
        let parent = parent_id
            .as_deref()
            .and_then(|id| self.find_store(id))
            .cloned();
        // A sub-account is always of its wallet's kind.
        let kind = parent.as_ref().map_or(kind, |parent| parent.kind);
        let (sub_type, binding) = match &parent {
            Some(_) => {
                let (sub_type, binding) =
                    Self::check_sub_account(kind, extras.sub_type, extras.binding.as_ref())?;
                (Some(sub_type), binding)
            }
            None => {
                if !kind.user_creatable() {
                    return Err(Error::WalletRule(WalletRule::NotCreatable));
                }
                (None, Binding::Any)
            }
        };
        if parent.is_none()
            && let Some(max) = kind.max_roots()
            && self
                .doc
                .stores
                .iter()
                .filter(|store| store.kind == kind && store.parent_id.is_none())
                .count()
                >= max
        {
            return Err(Error::WalletRule(WalletRule::CashSingleton));
        }
        self.check_store_extras(kind, sub_type, parent.is_none(), &extras)?;
        let mut store = Store::new(name, kind, color);
        store.note = note.trim().to_string();
        store.purpose = extras.purpose.unwrap_or_else(|| {
            sub_type
                .and_then(SubType::default_purpose)
                .or_else(|| parent.as_ref().map(|parent| parent.purpose))
                .unwrap_or_else(|| kind.default_purpose())
        });
        store.sub_type = sub_type;
        store.binding = binding;
        store.liability = extras.liability;
        store.contact_id = extras.contact_id;
        if parent.is_none() {
            store.custody = extras.custody.or(store.custody);
            store.property = extras
                .property
                .or_else(|| (kind == StoreKind::Property).then(PropertyInfo::default));
            // New objects start with the rule of their type (plan §11.4).
            if let Some(info) = store
                .property
                .as_mut()
                .filter(|_| kind == StoreKind::Property)
                && info.revalue.is_none()
            {
                info.revalue = valuation::initial_rule(info.category);
            }
            store.exchange = match (kind, extras.provider) {
                (StoreKind::Exchange, Some(provider)) => Some(ExchangeProfile {
                    provider,
                    import_interval_sec: 900,
                    terminal: Default::default(),
                    import: Default::default(),
                }),
                _ => None,
            };
        } else {
            store.custody = None;
        }
        store.secured_by = extras.secured_by;
        // Margin only means something on an exchange's futures account.
        store.margin = extras
            .margin
            .filter(|_| kind == StoreKind::Exchange && sub_type == Some(SubType::Futures));
        store.parent_id = parent_id;
        self.doc.stores.push(store.clone());
        // Validated before anything is written, so a bad opening balance
        // leaves no half-created wallet behind.
        if let Some(opening) = extras.opening.filter(|opening| !opening.amount.is_zero()) {
            if matches!(kind, StoreKind::Property | StoreKind::Liability) {
                self.doc.stores.pop();
                return Err(Error::WalletRule(WalletRule::NotApplicable));
            }
            let mut tx = opening_topup(&store.id, opening);
            if let Err(err) = self.prepare_transaction(&mut tx) {
                self.doc.stores.pop();
                return Err(err);
            }
            self.doc.transactions.push(tx);
        }
        self.recompute_holdings();
        self.persist()?;
        Ok(store)
    }

    /// A new bank together with its first card: a card is a bank account, so
    /// a bank is never left without one. The opening balance goes on the
    /// card. All or nothing.
    pub fn create_bank_with_card(
        &mut self,
        name: &str,
        color: &str,
        note: &str,
        mut extras: StoreExtras,
        card: FirstCard,
    ) -> Result<Store> {
        let currency = card.currency.trim().to_uppercase();
        if currency.is_empty() {
            return Err(Error::InvalidInput("the card needs a currency".into()));
        }
        let opening = extras.opening.take();
        let backup = self.doc.clone();
        let result = (|| -> Result<Store> {
            let bank = self.create_store_with(name, StoreKind::Bank, color, note, None, extras)?;
            let card_name = if card.name.trim().is_empty() {
                format!("{name} {currency}")
            } else {
                card.name.trim().to_string()
            };
            self.create_store_with(
                &card_name,
                StoreKind::Bank,
                color,
                "",
                Some(bank.id.clone()),
                StoreExtras {
                    sub_type: Some(SubType::Card),
                    binding: Some(Binding::Asset(AssetRef::fiat(&currency, &currency))),
                    opening,
                    ..StoreExtras::default()
                },
            )?;
            Ok(bank)
        })();
        if result.is_err() {
            self.doc = backup;
            self.recompute_holdings();
            self.persist()?;
        }
        result
    }

    /// Edit a store's own fields. Its place in the tree changes only through
    /// [`Self::move_store`].
    pub fn update_store(
        &mut self,
        id: &str,
        name: &str,
        color: &str,
        note: &str,
        extras: StoreExtras,
    ) -> Result<Store> {
        let current = self
            .find_store(id)
            .ok_or_else(|| Error::Missing("store".into()))?;
        let parent = current.parent_id.clone();
        let kind = current.kind;
        let sub_type = current.sub_type;
        if !name.trim().is_empty() {
            self.ensure_unique_name(Some(id), parent.as_deref(), name)?;
        }
        self.check_store_extras(kind, sub_type, parent.is_none(), &extras)?;
        let binding = match (&extras.binding, parent.is_some()) {
            (Some(binding), true) => {
                let (_, binding) = Self::check_sub_account(kind, sub_type, Some(binding))?;
                // Everything already booked here must still fit.
                let fits = self
                    .doc
                    .transactions
                    .iter()
                    .flat_map(Transaction::legs)
                    .filter(|leg| leg.store_id == id)
                    .all(|leg| binding.admits(&leg.asset));
                if !fits {
                    return Err(Error::WalletRule(WalletRule::BindingMismatch));
                }
                Some(binding)
            }
            (Some(_), false) => return Err(Error::WalletRule(WalletRule::NotApplicable)),
            (None, _) => None,
        };
        let out = {
            let store = self
                .doc
                .stores
                .iter_mut()
                .find(|store| store.id == id)
                .ok_or_else(|| Error::Missing("store".into()))?;
            if !name.trim().is_empty() {
                store.name = name.trim().to_string();
            }
            store.color = color.trim().to_string();
            store.note = note.trim().to_string();
            if let Some(purpose) = extras.purpose {
                store.purpose = purpose;
            }
            if let Some(binding) = binding {
                store.binding = binding;
            }
            store.liability = extras.liability;
            store.contact_id = extras.contact_id;
            if store.parent_id.is_none() {
                if extras.custody.is_some() {
                    store.custody = extras.custody;
                }
                if let Some(mut next) = extras.property {
                    // The rule is set by its own command only.
                    next.revalue = store
                        .property
                        .as_ref()
                        .and_then(|info| info.revalue.clone());
                    store.property = Some(next);
                }
                if let Some(provider) = extras.provider {
                    let profile = store.exchange.get_or_insert_with(|| ExchangeProfile {
                        provider,
                        import_interval_sec: 900,
                        terminal: Default::default(),
                        import: Default::default(),
                    });
                    profile.provider = provider;
                }
            }
            store.secured_by = extras.secured_by;
            store.clone()
        };
        self.prune_emergency_fund();
        self.persist()?;
        Ok(out)
    }

    /// Re-hang a sub-account under another node, or make it a top-level
    /// wallet with `parent_id = None`. A top-level wallet never moves (R6).
    pub fn move_store(&mut self, id: &str, parent_id: Option<&str>) -> Result<Store> {
        let store = self
            .find_store(id)
            .cloned()
            .ok_or_else(|| Error::Missing("store".into()))?;
        if store.parent_id.as_deref() == parent_id {
            return Ok(store);
        }
        self.validate_placement(Some(id), parent_id, &store.name)?;
        let out = {
            let store = self
                .doc
                .stores
                .iter_mut()
                .find(|store| store.id == id)
                .ok_or_else(|| Error::Missing("store".into()))?;
            store.parent_id = parent_id.map(str::to_string);
            store.clone()
        };
        self.persist()?;
        Ok(out)
    }

    /// Where `id` may be moved: every allowed parent (flat, without the
    /// current one) and whether it may become a top-level wallet.
    pub fn store_move_targets(&self, id: &str) -> Result<StoreMoveTargets> {
        let store = self
            .find_store(id)
            .ok_or_else(|| Error::Missing("store".into()))?;
        if store.parent_id.is_none() {
            return Ok(StoreMoveTargets::default());
        }
        let parents = flatten_tree(self.store_tree())
            .into_iter()
            .filter(|node| store.parent_id.as_deref() != Some(node.store.id.as_str()))
            .filter(|node| {
                self.validate_placement(Some(id), Some(&node.store.id), &store.name)
                    .is_ok()
            })
            .collect();
        Ok(StoreMoveTargets {
            parents,
            can_detach: false,
        })
    }

    /// Archive (or restore) a store together with its whole subtree. A node
    /// cannot be restored while an ancestor is still archived (R8).
    pub fn archive_store(&mut self, id: &str, archived: bool) -> Result<()> {
        let store = self
            .find_store(id)
            .ok_or_else(|| Error::Missing("store".into()))?;
        if archived && store.kind == StoreKind::Cash && store.parent_id.is_none() {
            return Err(Error::WalletRule(WalletRule::CashLocked));
        }
        if archived && store.archived {
            return Err(Error::InvalidInput("already archived".into()));
        }
        if !archived && self.ancestors_of(id).iter().any(|parent| parent.archived) {
            return Err(Error::StoreTree(StoreTreeViolation::RestoreParentFirst));
        }
        let mut affected: HashSet<String> = self.subtree_ids(id).into_iter().collect();
        affected.insert(id.to_string());
        for store in &mut self.doc.stores {
            if affected.contains(&store.id) {
                store.archived = archived;
            }
        }
        self.prune_emergency_fund();
        self.persist()
    }

    pub fn delete_store(&mut self, id: &str) -> Result<()> {
        if self
            .find_store(id)
            .is_some_and(|store| store.kind == StoreKind::Cash && store.parent_id.is_none())
        {
            return Err(Error::WalletRule(WalletRule::CashLocked));
        }
        if self.doc.transactions.iter().any(|tx| tx.touches(id)) {
            return Err(Error::InvalidInput(
                "store has transactions; archive instead".into(),
            ));
        }
        if !self.children_of(id).is_empty() {
            return Err(Error::InvalidInput(
                "store has sub-accounts; move or delete them first".into(),
            ));
        }
        let before = self.doc.stores.len();
        self.doc.stores.retain(|store| store.id != id);
        self.doc.holdings.retain(|holding| holding.store_id != id);
        if self.doc.stores.len() == before {
            return Err(Error::Missing("store".into()));
        }
        self.recompute_holdings();
        self.prune_emergency_fund();
        self.persist()?;
        Ok(())
    }
}
