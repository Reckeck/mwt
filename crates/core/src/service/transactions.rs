use super::*;

impl UnlockedVault {
    /// Transaction history, newest first, limited to a scope.
    pub fn transactions(&self, scope: &StoreScope) -> Vec<Transaction> {
        let ids = (!scope.store_ids.is_empty()).then(|| self.resolve_scope(scope));
        let mut out: Vec<Transaction> = match ids {
            Some(ids) => self.scoped_txs(&ids),
            None => self.doc.transactions.clone(),
        };
        out.sort_by(|left, right| right.meta().timestamp.cmp(&left.meta().timestamp));
        out
    }

    /// Fill what the user does not type: the purchase price of property
    /// (its unit price is in the base currency at the time of buying).
    pub(crate) fn complete_transaction(&self, tx: &mut Transaction) {
        if let Transaction::Topup(inner) = tx
            && let Some(unit_price) = inner.details.unit_price
            && inner.price.is_empty()
            && self
                .find_store(&inner.store_id)
                .is_some_and(|store| store.kind == StoreKind::Property)
        {
            let (base, _) = self.base_and_fx();
            inner.price.price_base = Some(unit_price);
            inner.price.price_base_currency = Some(base.clone());
            inner.price.price_usd = self
                .fx_at(&base, &inner.meta.timestamp)
                .filter(|rate| !rate.is_zero())
                .map(|rate| unit_price / rate);
        }
    }

    /// Everything a write does before storing: categories normalized and
    /// auto-filled, property prices completed, unpriced sides stamped, every
    /// rule checked.
    pub(crate) fn prepare_transaction(&self, tx: &mut Transaction) -> Result<()> {
        self.prepare_transaction_at(tx, iso_to_millis(&now_iso()))
    }

    /// [`Self::prepare_transaction`] with the future-date rule (V11) measured
    /// from `now_ms` — the scheduler's clock for automation runs.
    pub(crate) fn prepare_transaction_at(&self, tx: &mut Transaction, now_ms: i64) -> Result<()> {
        Self::normalize_categories(tx);
        self.apply_categorization_rules(tx);
        self.complete_transaction(tx);
        self.complete_adjustment(tx)?;
        Self::stamp_prices(&self.rate_book(), tx);
        self.validate_transaction_at(tx, now_ms)
    }

    /// Validate and append a transaction of any variant (plan §4.5, §8.1):
    /// categories are normalized and auto-filled, then every rule is checked.
    pub fn add_transaction(&mut self, tx: Transaction) -> Result<Transaction> {
        self.add_transaction_for(tx, None)
    }

    /// [`Self::add_transaction`], linked to a property (plan G4).
    pub fn add_transaction_for(
        &mut self,
        mut tx: Transaction,
        property_id: Option<&str>,
    ) -> Result<Transaction> {
        self.check_property_link(property_id)?;
        self.prepare_transaction(&mut tx)?;
        if tx.meta().id.trim().is_empty()
            || self
                .doc
                .transactions
                .iter()
                .any(|known| known.meta().id == tx.meta().id)
        {
            tx.meta_mut().id = uuid::Uuid::new_v4().to_string();
        }
        self.doc.transactions.push(tx.clone());
        self.set_property_link(&tx.meta().id, property_id);
        self.recompute_holdings();
        self.persist()?;
        Ok(tx)
    }

    /// Save several operations as one movement of money (an exchange split
    /// between recipients, a P2P deal): all join one chain — `flow_id`, or a
    /// new one — and either all are saved or none is.
    pub fn add_linked_transactions(
        &mut self,
        txs: Vec<Transaction>,
        flow_id: Option<&str>,
    ) -> Result<Vec<Transaction>> {
        if txs.is_empty() {
            return Err(Error::InvalidInput("nothing to save".into()));
        }
        let flow = flow_id
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let backup = self.doc.clone();
        let result = (|| -> Result<Vec<Transaction>> {
            let mut saved = Vec::with_capacity(txs.len());
            for mut tx in txs {
                tx.meta_mut().flow_id = Some(flow.clone());
                tx.meta_mut().id = uuid::Uuid::new_v4().to_string();
                self.prepare_transaction(&mut tx)?;
                self.doc.transactions.push(tx.clone());
                // The next one sees this one's balances, as if saved in turn.
                self.recompute_holdings();
                saved.push(tx);
            }
            Ok(saved)
        })();
        match result {
            Ok(saved) => {
                self.persist()?;
                Ok(saved)
            }
            Err(err) => {
                self.doc = backup;
                self.recompute_holdings();
                Err(err)
            }
        }
    }

    /// Replace a transaction entirely. Its id and origin are kept: an edit
    /// changes what happened, not where the record came from.
    pub fn update_transaction(&mut self, id: &str, tx: Transaction) -> Result<Transaction> {
        let property = self.property_of_tx(id).map(str::to_string);
        self.update_transaction_inner(id, tx, property.as_deref(), true)
    }

    /// [`Self::update_transaction`] for the shared event that owns the
    /// operation: the event has changed its own side already.
    pub(crate) fn update_transaction_unsynced(
        &mut self,
        id: &str,
        tx: Transaction,
    ) -> Result<Transaction> {
        let property = self.property_of_tx(id).map(str::to_string);
        self.update_transaction_inner(id, tx, property.as_deref(), false)
    }

    /// [`Self::update_transaction`] that also sets the operation's property;
    /// `None` unlinks it.
    pub fn update_transaction_for(
        &mut self,
        id: &str,
        tx: Transaction,
        property_id: Option<&str>,
    ) -> Result<Transaction> {
        self.update_transaction_inner(id, tx, property_id, true)
    }

    /// `follow_events`: an operation a shared event owns drags the event's
    /// purchase or settlement along (or is refused).
    pub(crate) fn update_transaction_inner(
        &mut self,
        id: &str,
        mut tx: Transaction,
        property_id: Option<&str>,
        follow_events: bool,
    ) -> Result<Transaction> {
        self.check_property_link(property_id)?;
        let index = self
            .doc
            .transactions
            .iter()
            .position(|known| known.meta().id == id)
            .ok_or_else(|| Error::Missing("transaction".into()))?;
        let mut origin = self.doc.transactions[index].meta().origin.clone();
        let keep = self.doc.transactions[index].meta().keep;
        let utility_item_id = self.doc.transactions[index].meta().utility_item_id.clone();
        // A new amount for an operation booked approximately is the real one.
        if let TxOrigin::Workflow { estimated, .. } = &mut origin
            && *estimated
            && automations::main_amount(&self.doc.transactions[index])
                != automations::main_amount(&tx)
        {
            *estimated = false;
        }
        tx.meta_mut().id = id.to_string();
        tx.meta_mut().origin = origin;
        // The «keep» mark is not the form's to change.
        tx.meta_mut().keep = keep;
        // Nor is the utility bill it settles: the form has no such field.
        tx.meta_mut().utility_item_id = utility_item_id;
        self.prepare_transaction(&mut tx)?;
        let event_edit = if follow_events {
            self.plan_shared_tx_edit(id, &tx)?
        } else {
            None
        };
        self.doc.transactions[index] = tx.clone();
        self.apply_shared_edit(event_edit);
        self.set_property_link(id, property_id);
        self.recompute_holdings();
        self.persist()?;
        Ok(tx)
    }

    pub fn delete_transaction(&mut self, id: &str) -> Result<()> {
        let before = self.doc.transactions.len();
        self.doc.transactions.retain(|tx| tx.meta().id != id);
        if self.doc.transactions.len() == before {
            return Err(Error::Missing("transaction".into()));
        }
        self.cascade_shared_delete(id);
        self.set_property_link(id, None);
        self.recompute_holdings();
        self.persist()?;
        Ok(())
    }
}
