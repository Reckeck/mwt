use super::*;

impl UnlockedVault {
    /// Status of the Bybit key stored for exactly this store.
    pub fn exchange_key_status(&self, store_id: &str) -> crate::ipc_contract::ExchangeKeyStatus {
        self.doc
            .exchanges
            .bybit
            .iter()
            .find(|entry| entry.store_id == store_id)
            .map(|entry| crate::ipc_contract::ExchangeKeyStatus {
                configured: true,
                hint: Some(self.exchange_key_hint(entry)),
                spot_store_id: entry.spot_store_id.clone(),
                futures_store_id: entry.futures_store_id.clone(),
            })
            .unwrap_or_default()
    }

    /// Store (or replace) a Bybit key for an exchange store in the device
    /// secrets file, unverified (the app runs the read-only check first and
    /// calls [`Self::store_device_key`]). Level 3 only. Sub-account targets of
    /// a replaced key are kept.
    pub fn set_exchange_key(
        &mut self,
        store_id: &str,
        api_key: &str,
        api_secret: &str,
    ) -> Result<crate::ipc_contract::ExchangeKeyStatus> {
        self.store_device_key(store_id, api_key, api_secret, false)?;
        Ok(self.exchange_key_status(store_id))
    }

    /// Point imported spot / futures fills at sub-accounts of the key's
    /// store. Each target must be an exchange store inside that wallet.
    pub fn set_exchange_targets(
        &mut self,
        store_id: &str,
        spot_store_id: Option<String>,
        futures_store_id: Option<String>,
    ) -> Result<crate::ipc_contract::ExchangeKeyStatus> {
        let mut allowed: HashSet<String> = self.subtree_ids(store_id).into_iter().collect();
        allowed.insert(store_id.to_string());
        for target in [&spot_store_id, &futures_store_id].into_iter().flatten() {
            let ok = allowed.contains(target)
                && self
                    .find_store(target)
                    .is_some_and(|store| store.kind == StoreKind::Exchange && !store.archived);
            if !ok {
                return Err(Error::InvalidInput(
                    "fills can only go to an exchange sub-account of this wallet".into(),
                ));
            }
        }
        let entry = self
            .doc
            .exchanges
            .bybit
            .iter_mut()
            .find(|entry| entry.store_id == store_id)
            .ok_or_else(|| Error::Missing("exchange key".into()))?;
        entry.spot_store_id = spot_store_id;
        entry.futures_store_id = futures_store_id;
        self.persist()?;
        Ok(self.exchange_key_status(store_id))
    }

    /// Stage fills for user confirmation.
    ///
    /// Fills are keyed by Bybit's `execId`, so re-importing the same history is
    /// idempotent. Nothing here touches `transactions` — the user has to confirm
    /// first, which is what keeps a background import from rewriting history.
    /// Returns how many fills were newly staged.
    pub fn stage_imports(&mut self, imports: Vec<PendingImport>) -> Result<u32> {
        let before = self.doc.pending_imports.len();
        // Staged, booked or discarded before: a later poll sees the same
        // history again and must not stage it twice.
        let mut known = self.known_import_ids();

        for imp in imports {
            if imp.exec_id.is_empty() || known.contains(&imp.exec_id) {
                continue;
            }
            if self.ensure_store(&imp.store_id).is_err() {
                continue;
            }
            known.insert(imp.exec_id.clone());
            self.doc.pending_imports.push(imp);
            if self.doc.pending_imports.len() >= MAX_PENDING_IMPORTS {
                break;
            }
        }

        let staged = (self.doc.pending_imports.len() - before) as u32;
        if staged > 0 {
            self.persist()?;
        }
        Ok(staged)
    }

    /// Every staged fill, newest first.
    pub fn pending_imports(&self) -> Vec<PendingImport> {
        let mut out = self.doc.pending_imports.clone();
        out.sort_by(|left, right| right.occurred_at.cmp(&left.occurred_at));
        out
    }

    /// Where a fill of this category is booked: the sub-account picked for
    /// spot or futures on the exchange key, else the wallet's live sub-account
    /// bound to that market, else the key's own store.
    pub(crate) fn import_target(&self, imp: &PendingImport) -> String {
        if imp.kind == crate::model::vault::ImportKind::EarnIncome {
            return self
                .bybit_account_store(&imp.store_id, crate::exchange::bybit::ACCOUNT_EARN)
                .unwrap_or_else(|| imp.store_id.clone());
        }
        let futures = imp.category != "spot";
        let market = if futures {
            crate::model::store::MarketKind::Futures
        } else {
            crate::model::store::MarketKind::Spot
        };
        self.doc
            .exchanges
            .bybit
            .iter()
            .find(|keys| keys.store_id == imp.store_id)
            .and_then(|keys| {
                if futures {
                    keys.futures_store_id.clone()
                } else {
                    keys.spot_store_id.clone()
                }
            })
            // Bybit trades spot and derivatives in one unified account.
            .or_else(|| {
                self.bybit_account_store(&imp.store_id, crate::exchange::bybit::ACCOUNT_UNIFIED)
            })
            .or_else(|| {
                self.children_of(&imp.store_id)
                    .into_iter()
                    .find(|child| !child.archived && child.binding == Binding::Market(market))
                    .map(|child| child.id.clone())
            })
            .unwrap_or_else(|| imp.store_id.clone())
    }

    /// The Bybit key that serves a store: its own, else its wallet's — a
    /// sub-account never borrows another wallet's key.
    pub fn bybit_key_for(&self, store_id: &str) -> Option<&crate::model::vault::ExchangeKeys> {
        let mut current = self.find_store(store_id)?;
        loop {
            if let Some(keys) = self
                .doc
                .exchanges
                .bybit
                .iter()
                .find(|keys| keys.store_id == current.id)
            {
                return Some(keys);
            }
            current = self.find_store(current.parent_id.as_deref()?)?;
        }
    }

    /// The live sub-account of an exchange wallet that stands for a Bybit
    /// account type: `FUND` → Funding, `UNIFIED` (and the legacy `SPOT`,
    /// `CONTRACT`) → Unified, `EARN`/`INVESTMENT` → Earn.
    pub(crate) fn bybit_account_store(&self, root_id: &str, account: &str) -> Option<String> {
        use crate::model::store::MarketKind;
        // The import settings may send an account to a wallet of the user's choice.
        if let Some(Some(target)) = self.import_route(root_id, account) {
            return Some(target);
        }
        let market = match account.trim().to_ascii_uppercase().as_str() {
            "FUND" => MarketKind::Funding,
            "UNIFIED" | "SPOT" | "CONTRACT" => MarketKind::Unified,
            "EARN" | "INVESTMENT" => MarketKind::Earn,
            _ => return None,
        };
        self.children_of(root_id)
            .into_iter()
            .find(|child| !child.archived && child.binding == Binding::Market(market))
            .map(|child| child.id.clone())
    }

    /// Give a Bybit wallet its three accounts — Funding, Unified Trading and
    /// Earn — creating the ones missing. Returns the ids `(funding, unified, earn)`.
    pub fn ensure_bybit_accounts(
        &mut self,
        store_id: &str,
        names: [&str; 3],
    ) -> Result<[String; 3]> {
        let ids = self.ensure_bybit_accounts_for(store_id, names, [true; 3])?;
        Ok(ids.map(Option::unwrap_or_default))
    }

    /// [`Self::ensure_bybit_accounts`] for the accounts `wanted`
    /// (funding, unified, earn); the others are left alone.
    pub fn ensure_bybit_accounts_for(
        &mut self,
        store_id: &str,
        names: [&str; 3],
        wanted: [bool; 3],
    ) -> Result<[Option<String>; 3]> {
        use crate::model::store::MarketKind;
        let root = self
            .find_store(store_id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
        if root.kind != StoreKind::Exchange || root.parent_id.is_some() {
            return Err(Error::InvalidInput("not an exchange wallet".into()));
        }
        let accounts = [
            (SubType::Funding, MarketKind::Funding),
            (SubType::Unified, MarketKind::Unified),
            (SubType::Earn, MarketKind::Earn),
        ];
        let mut ids: [Option<String>; 3] = Default::default();
        for (index, (sub_type, market)) in accounts.into_iter().enumerate() {
            let existing = self
                .children_of(store_id)
                .into_iter()
                .find(|child| !child.archived && child.binding == Binding::Market(market))
                .map(|child| child.id.clone());
            ids[index] = match existing {
                Some(id) => Some(id),
                None if wanted[index] => Some(
                    self.create_store_with(
                        names[index],
                        StoreKind::Exchange,
                        &root.color,
                        "",
                        Some(store_id.to_string()),
                        StoreExtras {
                            sub_type: Some(sub_type),
                            binding: Some(Binding::Market(market)),
                            ..StoreExtras::default()
                        },
                    )?
                    .id,
                ),
                None => None,
            };
        }
        Ok(ids)
    }

    /// The live Unified Trading account of an exchange wallet: where spot and
    /// futures trades are booked.
    pub fn unified_account_of(&self, store_id: &str) -> Option<String> {
        let root = self.root_of(store_id);
        self.bybit_account_store(&root, crate::exchange::bybit::ACCOUNT_UNIFIED)
    }

    /// What each Bybit account holds per the API next to what the ledger
    /// says its sub-account holds, coin by coin.
    pub fn bybit_reconcile(
        &self,
        store_id: &str,
        balances: &[(String, Vec<crate::exchange::CoinBalance>)],
    ) -> Vec<crate::ipc_contract::ExchangeAccountBalance> {
        balances
            .iter()
            .map(|(account, coins)| {
                let target = self.bybit_account_store(store_id, account);
                let ledger: HashMap<String, Amount> = target
                    .as_deref()
                    .map(|id| {
                        self.doc
                            .holdings
                            .iter()
                            .filter(|holding| {
                                holding.store_id == id && !holding.asset.is_derivative()
                            })
                            .map(|holding| {
                                (holding.asset.symbol.to_ascii_uppercase(), holding.quantity)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let mut symbols: Vec<String> = coins.iter().map(|coin| coin.coin.clone()).collect();
                symbols.extend(ledger.keys().cloned());
                symbols.sort();
                symbols.dedup();
                let rows = symbols
                    .into_iter()
                    .map(|coin| {
                        let exchange = coins
                            .iter()
                            .find(|row| row.coin == coin)
                            .and_then(|row| Amount::parse(&row.amount).ok())
                            .unwrap_or(Amount::ZERO);
                        let booked = ledger.get(&coin).copied().unwrap_or(Amount::ZERO);
                        crate::ipc_contract::ExchangeCoinRow {
                            coin,
                            exchange,
                            ledger: booked,
                            diff: exchange - booked,
                        }
                    })
                    .collect();
                crate::ipc_contract::ExchangeAccountBalance {
                    account: account.clone(),
                    store_id: target,
                    rows,
                }
            })
            .collect()
    }

    pub(crate) fn exchange_profile_mut(&mut self, store_id: &str) -> Result<&mut ExchangeProfile> {
        self.doc
            .stores
            .iter_mut()
            .find(|store| store.id == store_id)
            .ok_or_else(|| Error::Missing(format!("store {store_id}")))?
            .exchange
            .as_mut()
            .ok_or(Error::WalletRule(WalletRule::ProviderRequired))
    }

    /// Background import interval of one exchange; `0` disables it,
    /// otherwise it is kept between a minute and a day.
    pub fn set_import_interval(&mut self, store_id: &str, interval_sec: u32) -> Result<()> {
        let profile = self.exchange_profile_mut(store_id)?;
        profile.import_interval_sec = if interval_sec == 0 {
            0
        } else {
            interval_sec.clamp(60, 86_400)
        };
        self.persist()
    }

    /// Exchanges with a key on this device whose background import is on,
    /// with their interval. Empty below level 3 (a stored key is not used
    /// there) and when the device file does not open. The keys zeroize on drop.
    pub fn import_schedule(&self) -> Vec<(u32, crate::vault_io::device_secrets::DeviceKey)> {
        if !self.keys_on_device() || self.doc.exchanges.bybit.is_empty() {
            return Vec::new();
        }
        let Ok(secrets) = self.device_secrets_for_request() else {
            return Vec::new();
        };
        secrets
            .bybit
            .into_iter()
            .filter(|entry| !entry.keys.api_key.is_empty() && !entry.keys.api_secret.is_empty())
            .filter_map(|entry| {
                let store = self.find_store(&entry.keys.store_id)?;
                let interval = store.exchange.as_ref()?.import_interval_sec;
                (!store.archived && interval > 0).then_some((interval, entry))
            })
            .collect()
    }

    pub fn terminal_state(&self, store_id: &str) -> Result<TerminalState> {
        self.find_store(store_id)
            .ok_or_else(|| Error::Missing(format!("store {store_id}")))?
            .exchange
            .as_ref()
            .map(|profile| profile.terminal.clone())
            .ok_or(Error::WalletRule(WalletRule::ProviderRequired))
    }

    /// Patch the terminal of one exchange; only the given fields change, so a
    /// timeframe switch never overwrites the watchlist or the drawings.
    pub fn update_terminal_state(
        &mut self,
        store_id: &str,
        patch: TerminalPatch,
    ) -> Result<TerminalState> {
        let terminal = &mut self.exchange_profile_mut(store_id)?.terminal;
        let before = terminal.clone();
        if let Some(watchlist) = patch.watchlist {
            let mut seen = HashSet::new();
            terminal.watchlist = watchlist
                .into_iter()
                .map(|symbol| symbol.trim().to_uppercase())
                .filter(|symbol| !symbol.is_empty() && seen.insert(symbol.clone()))
                .take(200)
                .collect();
        }
        if let Some(symbol) = patch.active_symbol {
            terminal.active_symbol =
                Some(symbol.trim().to_uppercase()).filter(|symbol| !symbol.is_empty());
        }
        if let Some(timeframe) = patch.timeframe {
            terminal.timeframe = timeframe;
        }
        if let Some(indicators) = patch.indicators {
            terminal.indicators = indicators;
        }
        if let Some(layout) = patch.layout {
            terminal.layout = layout;
        }
        let out = terminal.clone();
        // The terminal re-sends its state on every symbol or timeframe click.
        if out != before {
            self.persist()?;
        }
        Ok(out)
    }

    /// Replace one symbol's drawings; an empty export removes the entry.
    pub fn save_terminal_drawings(
        &mut self,
        store_id: &str,
        symbol: &str,
        json: &str,
    ) -> Result<()> {
        if json.len() > crate::model::store::MAX_DRAWINGS_JSON {
            return Err(Error::TooLarge("chart drawings".into()));
        }
        let symbol = symbol.trim().to_uppercase();
        if symbol.is_empty() {
            return Err(Error::InvalidInput("symbol is required".into()));
        }
        let terminal = &mut self.exchange_profile_mut(store_id)?.terminal;
        let empty = json.trim().is_empty() || json.trim() == "[]";
        let changed = if empty {
            terminal.drawings.remove(&symbol).is_some()
        } else {
            terminal
                .drawings
                .insert(symbol, json.to_string())
                .as_deref()
                != Some(json)
        };
        if changed {
            self.persist()?;
        }
        Ok(())
    }

    /// The transaction a staged fill becomes: a spot trade, or a futures
    /// open/close for linear and inverse contracts.
    pub(crate) fn transaction_for_import(&self, imp: &PendingImport) -> Transaction {
        use crate::model::vault::ImportKind;
        let mut meta = TxMeta::new(&imp.comment);
        meta.categories = self.import_categories(imp, "trading");
        meta.origin = TxOrigin::Import {
            exec_id: imp.exec_id.clone(),
        };
        if !imp.occurred_at.is_empty() {
            meta.timestamp = imp.occurred_at.clone();
        }
        match imp.kind {
            ImportKind::Fill => {}
            ImportKind::Transfer => {
                meta.categories = self.import_categories(imp, "transfer");
                let side = |account: &Option<String>| {
                    account
                        .as_deref()
                        .and_then(|account| self.bybit_account_store(&imp.store_id, account))
                        .unwrap_or_else(|| imp.store_id.clone())
                };
                return Transaction::Transfer(TransferTx {
                    meta,
                    from: AccountRef::Store(side(&imp.from_account)),
                    to: AccountRef::Store(side(&imp.to_account)),
                    asset: self.asset_for_import(&imp.asset_bought),
                    amount: imp.qty_bought,
                    fee: None,
                    network: None,
                    tx_hash: None,
                    debt: None,
                    price: PriceStamp::default(),
                });
            }
            ImportKind::EarnIncome => {
                meta.categories = self.import_categories(imp, "deposit_interest");
                return Transaction::Topup(TopupTx {
                    meta,
                    store_id: self.import_target(imp),
                    asset: self.asset_for_import(&imp.asset_bought),
                    amount: imp.qty_bought,
                    fee: None,
                    from: None,
                    details: KindDetails::default(),
                    price: PriceStamp::default(),
                });
            }
        }
        let store_id = self.import_target(imp);
        let is_buy = imp.side.eq_ignore_ascii_case("buy");
        let (base_symbol, quote_symbol) = if is_buy {
            (&imp.asset_bought, &imp.asset_sold)
        } else {
            (&imp.asset_sold, &imp.asset_bought)
        };
        let inverse = imp.category == "inverse";
        let settle_symbol = if inverse { base_symbol } else { quote_symbol };
        // Spot fees come in the coin Bybit names; futures fees are always
        // charged in the settle asset.
        let fee_symbol = match imp.category.as_str() {
            "spot" if !imp.fee_asset.is_empty() => imp.fee_asset.as_str(),
            "spot" => quote_symbol.as_str(),
            _ => settle_symbol.as_str(),
        };
        let fee = (!imp.fee.is_zero()).then(|| Fee {
            asset: self.asset_for_import(fee_symbol),
            amount: imp.fee.abs(),
            price: PriceStamp::default(),
        });
        // A stablecoin quote makes the fill price a USD price.
        let usd_quoted = ["USDT", "USDC", "USD", "FDUSD", "BUSD"]
            .contains(&quote_symbol.to_ascii_uppercase().as_str());
        let stamp = |usd: Option<Amount>| PriceStamp {
            price_usd: usd,
            ..PriceStamp::default()
        };
        if imp.category == "spot" {
            let base_stamp = stamp(usd_quoted.then_some(imp.price));
            let quote_stamp = stamp(usd_quoted.then(|| Amount::new(rust_decimal::Decimal::ONE)));
            let (sold_stamp, bought_stamp) = if is_buy {
                (quote_stamp, base_stamp)
            } else {
                (base_stamp, quote_stamp)
            };
            return Transaction::TradeSpot(TradeSpotTx {
                meta,
                store_id,
                sold: TradeSide {
                    asset: self.asset_for_import(&imp.asset_sold),
                    qty: imp.qty_sold,
                    price: sold_stamp,
                },
                bought: TradeSide {
                    asset: self.asset_for_import(&imp.asset_bought),
                    qty: imp.qty_bought,
                    price: bought_stamp,
                },
                fee,
                liquidity: imp.is_maker.map(|maker| {
                    if maker {
                        Liquidity::Maker
                    } else {
                        Liquidity::Taker
                    }
                }),
                order_id: None,
            });
        }
        let closing = imp.closed_size.is_some_and(|size| size.is_positive());
        // Buying closes a short, selling closes a long.
        let direction = match (is_buy, closing) {
            (true, false) | (false, true) => FuturesDirection::Long,
            (false, false) | (true, true) => FuturesDirection::Short,
        };
        let settle = self.asset_for_import(settle_symbol);
        Transaction::TradeFutures(TradeFuturesTx {
            meta,
            store_id,
            contract: AssetRef::perp("bybit", &imp.symbol),
            settle,
            direction,
            action: if closing {
                FuturesAction::Close
            } else {
                FuturesAction::Open
            },
            quantity: if is_buy { imp.qty_bought } else { imp.qty_sold },
            fill_price: imp.price,
            leverage: None,
            fee,
            closed_pnl: if closing { imp.closed_pnl } else { None },
            order_id: None,
            category: if inverse {
                FuturesCategory::Inverse
            } else {
                FuturesCategory::Linear
            },
            price: stamp((usd_quoted && !inverse).then(|| Amount::new(rust_decimal::Decimal::ONE))),
        })
    }

    /// Turn staged fills into real trades.
    ///
    /// An empty `exec_ids` confirms everything staged. Confirmed entries leave
    /// the staging area so they cannot be imported twice; a fill that fails
    /// validation (its store was archived, say) stays staged.
    pub fn confirm_pending_imports(&mut self, exec_ids: &[String]) -> Result<u32> {
        let all = exec_ids.is_empty();
        let wanted: HashSet<&str> = exec_ids.iter().map(String::as_str).collect();
        let mut confirmed = 0u32;
        let mut kept = Vec::new();
        self.ensure_import_category();

        for imp in std::mem::take(&mut self.doc.pending_imports) {
            if !(all || wanted.contains(imp.exec_id.as_str())) {
                kept.push(imp);
                continue;
            }
            if self.find_store(&imp.store_id).is_none() {
                continue; // store vanished: drop the staged fill, don't confirm it
            }
            let tx = self.transaction_for_import(&imp);
            if self.validate_transaction(&tx).is_err() {
                kept.push(imp);
                continue;
            }
            self.doc.transactions.push(tx);
            confirmed += 1;
        }

        self.doc.pending_imports = kept;
        if confirmed > 0 {
            self.recompute_holdings();
        }
        self.persist()?;
        Ok(confirmed)
    }

    /// Drop staged fills without creating transactions.
    /// An empty `exec_ids` discards everything.
    pub fn discard_pending_imports(&mut self, exec_ids: &[String]) -> Result<u32> {
        let all = exec_ids.is_empty();
        let before = self.doc.pending_imports.len();
        let wanted: HashSet<&str> = exec_ids.iter().map(String::as_str).collect();
        let (dropped, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.doc.pending_imports)
            .into_iter()
            .partition(|pending| all || wanted.contains(pending.exec_id.as_str()));
        self.doc.pending_imports = kept;
        self.remember_discarded(dropped.into_iter().map(|pending| pending.exec_id));
        let discarded = (before - self.doc.pending_imports.len()) as u32;
        if discarded > 0 {
            self.persist()?;
        }
        Ok(discarded)
    }

    /// Build an [`AssetRef`] for an imported symbol.
    ///
    /// The chain is inherited from an asset the user already tracks, so an
    /// imported fill lands in the same bucket as manual entries. Unknown
    /// symbols stay chain-less (`None`) rather than being guessed at — the
    /// user can bind a chain later.
    pub(crate) fn asset_for_import(&self, symbol: &str) -> AssetRef {
        let upper = symbol.trim().to_ascii_uppercase();
        match self
            .doc
            .holdings
            .iter()
            .find(|holding| holding.asset.symbol.eq_ignore_ascii_case(symbol))
            .map(|holding| holding.asset.clone())
        {
            // Reuse the identity the user already tracks (name, coingecko id,
            // chain) so the fill lands in the same bucket as manual entries.
            Some(mut asset) => {
                asset.symbol = upper;
                asset
            }
            // Unseen symbol: a well-known coin gets its CoinGecko id (so it is
            // priced like one picked by hand); the chain stays unresolved so
            // the user can bind it later, rather than guessing a network.
            None => {
                let known = crate::exchange::well_known_coin(&upper);
                let name = known.map_or(upper.as_str(), |(_, name)| name);
                let mut asset =
                    AssetRef::crypto(&upper, name, known.map(|(id, _)| id), Chain::Fiat);
                asset.chain = None;
                asset
            }
        }
    }
}
