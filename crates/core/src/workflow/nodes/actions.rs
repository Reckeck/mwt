//! Actions: operations, distribution, notices, reminders, the confirmation
//! queue, stop, events and waits.

use rust_decimal::Decimal;
use serde_json::Value;

use crate::Amount;
use crate::calendar::Calendar;
use crate::model::notify::NoticeCategory;
use crate::model::planning::Reminder;
use crate::model::store::AssetRef;
use crate::model::transaction::{
    AccountRef, ConvertSide, ConvertTx, PaymentTx, TopupTx, Transaction, TransferTx, TxMeta,
    TxOrigin,
};
use crate::workflow::catalog::NodeDef;
use crate::workflow::exec::{
    EmittedEvent, Executor, Fault, Halt, MAX_ACTIONS, MAX_CALL_DEPTH, RunContext, RunMode, Step,
    StepStatus,
};
use crate::workflow::graph::Graph;
use crate::workflow::graph::Node;
use crate::workflow::model::{QueuedActions, WorkflowWait};
use crate::workflow::types::WfValue;

use super::{json_account, json_number};

fn places_for(asset: &AssetRef) -> u32 {
    if asset.is_fiat() { 2 } else { 8 }
}

fn positive(amount: Option<Amount>, what: &str) -> Result<Amount, String> {
    match amount {
        Some(value) if value.is_positive() => Ok(value),
        Some(value) => Err(format!("{what} must be above zero, not {value}")),
        None => Err(format!("set {what}")),
    }
}

fn store_of(account: Option<AccountRef>, what: &str) -> Result<String, String> {
    match account {
        Some(AccountRef::Store(id)) => Ok(id),
        Some(_) => Err(format!("{what} must be one of your accounts")),
        None => Err(format!("choose {what}")),
    }
}

impl Executor<'_> {
    fn account_name(&self, account: &AccountRef) -> String {
        match account {
            AccountRef::Store(id) => self
                .vault
                .find_store(id)
                .map_or_else(|| id.clone(), |store| store.name.clone()),
            AccountRef::Contact { contact_id, .. } => self
                .vault
                .doc
                .contacts
                .iter()
                .find(|contact| contact.id == *contact_id)
                .map_or_else(|| contact_id.clone(), |contact| contact.name.clone()),
            AccountRef::Raw(text) => text.clone(),
        }
    }

    /// An operation in a few words, in both languages.
    fn describe(&self, tx: &Transaction) -> (String, String) {
        let store = |id: &str| self.account_name(&AccountRef::Store(id.to_string()));
        match tx {
            Transaction::Topup(inner) => {
                let what = format!("{} {}", inner.amount, inner.asset.symbol);
                (
                    format!("пополнение «{}» на {what}", store(&inner.store_id)),
                    format!("top-up of «{}» by {what}", store(&inner.store_id)),
                )
            }
            Transaction::Payment(inner) => {
                let what = format!("{} {}", inner.amount, inner.asset.symbol);
                (
                    format!("оплата {what} с «{}»", store(&inner.store_id)),
                    format!("payment of {what} from «{}»", store(&inner.store_id)),
                )
            }
            Transaction::Transfer(inner) => {
                let what = format!("{} {}", inner.amount, inner.asset.symbol);
                let (from, to) = (self.account_name(&inner.from), self.account_name(&inner.to));
                (
                    format!("перевод {what}: «{from}» → «{to}»"),
                    format!("transfer of {what}: «{from}» → «{to}»"),
                )
            }
            Transaction::Convert(inner) => {
                let give = format!("{} {}", inner.from.amount, inner.from.asset.symbol);
                let get = format!("{} {}", inner.to.amount, inner.to.asset.symbol);
                (
                    format!("обмен {give} → {get}"),
                    format!("exchange {give} → {get}"),
                )
            }
            other => (format!("{:?}", other.kind()), format!("{:?}", other.kind())),
        }
    }

    fn meta(&self, comment: &str, category: Option<String>) -> TxMeta {
        let comment = if comment.trim().is_empty() {
            self.ctx.workflow_name.clone()
        } else {
            comment.to_string()
        };
        let mut meta = TxMeta::new(&comment);
        meta.timestamp = crate::calendar::iso(self.ctx.now);
        meta.categories = category.into_iter().collect();
        meta.origin = TxOrigin::Workflow {
            workflow_id: self.ctx.workflow_id.clone(),
            run_id: self.ctx.run_id.clone(),
        };
        meta
    }

    fn category_input(&mut self, node: &Node) -> Result<Option<String>, String> {
        Ok(match self.input(node, "category")? {
            WfValue::Category(key) | WfValue::Text(key) if !key.trim().is_empty() => Some(key),
            _ => None,
        })
    }

    /// Make the operations, or queue them after «Ждать подтверждения». All of
    /// them are kept or none.
    fn commit(
        &mut self,
        txs: Vec<Transaction>,
        summary: &str,
    ) -> Result<(Vec<Transaction>, bool), Fault> {
        self.actions += 1;
        if self.actions > MAX_ACTIONS {
            return Err(Halt::Fail(format!("more than {MAX_ACTIONS} actions in one run")).into());
        }
        let value = txs
            .iter()
            .fold(Amount::ZERO, |sum, tx| sum + self.vault.wf_outflow_base(tx));
        if let Some(cap) = self.ctx.max_amount
            && self.moved + value > cap
        {
            let base = self.vault.base_currency().to_uppercase();
            return Err(Halt::Fail(format!(
                "the run would move {} {base}, over its cap of {cap} {base}",
                (self.moved + value).round(2)
            ))
            .into());
        }
        if self.queue_mode {
            let checked = self.vault.wf_check(txs)?;
            self.queued.push(QueuedActions {
                id: uuid::Uuid::new_v4().to_string(),
                workflow_id: self.ctx.workflow_id.clone(),
                run_id: self.ctx.run_id.clone(),
                created_at: crate::calendar::iso(self.ctx.now),
                summary: summary.to_string(),
                transactions: checked.clone(),
            });
            self.moved += value;
            return Ok((checked, true));
        }
        let saved = self.vault.wf_insert(txs)?;
        self.moved += value;
        self.created.extend(saved.iter().cloned());
        Ok((saved, false))
    }

    fn build_transaction(&mut self, node: &Node) -> Result<Transaction, String> {
        let kind = self.text_param(node, "tx_kind");
        let account = self.account_input(node, "account")?;
        let to = self.account_input(node, "to")?;
        let amount_value = self.input(node, "amount")?;
        let amount = positive(amount_value.as_number(), "the amount")?;
        let asset = match self.asset_input(node, "asset")? {
            Some(asset) => asset,
            None => match amount_value.currency() {
                Some(code) => self.vault.wf_asset(&code),
                None => return Err("choose the asset".into()),
            },
        };
        let category = self.category_input(node)?;
        let comment = self.input(node, "comment")?.as_text();
        let meta = self.meta(&comment, category);
        Ok(match kind.as_str() {
            "topup" => Transaction::Topup(TopupTx {
                meta,
                store_id: store_of(account, "the account to top up")?,
                asset,
                amount,
                fee: None,
                from: to,
                details: Default::default(),
                price: Default::default(),
            }),
            "payment" => Transaction::Payment(PaymentTx {
                meta,
                store_id: store_of(account, "the account to pay from")?,
                asset,
                amount,
                fee: None,
                to,
                details: Default::default(),
                price: Default::default(),
            }),
            "convert" => {
                let from = account.ok_or("choose the account to exchange from")?;
                let to_asset = self
                    .asset_input(node, "to_asset")?
                    .ok_or("choose what to receive")?;
                let to_amount =
                    positive(self.number_input(node, "to_amount")?, "the amount received")?;
                Transaction::Convert(ConvertTx {
                    meta,
                    from: ConvertSide {
                        account: from.clone(),
                        asset,
                        amount,
                        price: Default::default(),
                    },
                    to: ConvertSide {
                        account: to.unwrap_or(from),
                        asset: to_asset,
                        amount: to_amount,
                        price: Default::default(),
                    },
                    fee: None,
                    rate_override: None,
                })
            }
            _ => Transaction::Transfer(TransferTx {
                meta,
                from: account.ok_or("choose where the money comes from")?,
                to: to.ok_or("choose where the money goes")?,
                asset,
                amount,
                fee: None,
                network: None,
                tx_hash: None,
                debt: None,
                price: Default::default(),
            }),
        })
    }

    fn build_distribution(&mut self, node: &Node) -> Result<Vec<Transaction>, String> {
        let amount_value = self.input(node, "amount")?;
        let total = positive(amount_value.as_number(), "the amount")?;
        let asset = match amount_value.currency() {
            Some(code) => self.vault.wf_asset(&code),
            None => return Err("wire an amount with its currency".into()),
        };
        let from = self
            .account_input(node, "from")?
            .ok_or("choose where the money comes from")?;
        let places = places_for(&asset);
        let fixed = self.text_param(node, "mode") == "fixed";
        let targets: Vec<(AccountRef, Amount)> = match self.raw_param(node, "targets") {
            Value::Array(items) => items
                .iter()
                .filter_map(|item| {
                    let account = json_account(item.get("account")?)?;
                    let share = item.get("share").and_then(json_number)?;
                    share.is_positive().then_some((account, share))
                })
                .collect(),
            _ => Vec::new(),
        };
        if targets.is_empty() {
            return Err("add where the money goes".into());
        }
        let hundred = Amount::new(Decimal::ONE_HUNDRED);
        let shares_total = targets
            .iter()
            .fold(Amount::ZERO, |sum, (_, share)| sum + *share);
        let whole = !fixed && shares_total == hundred;
        let mut parts: Vec<(AccountRef, Amount)> = Vec::new();
        let mut given = Amount::ZERO;
        let last = targets.len() - 1;
        for (index, (account, share)) in targets.into_iter().enumerate() {
            let part = if fixed {
                share
            } else if whole && index == last {
                total - given
            } else {
                (total * share / hundred).round(places)
            };
            given += part;
            if part.is_positive() {
                parts.push((account, part));
            }
        }
        if given > total {
            return Err(format!("the parts add up to {given}, more than {total}"));
        }
        let category = match self.param(node, "category") {
            WfValue::Category(key) => Some(key),
            _ => None,
        };
        let comment = self.text_param(node, "comment");
        Ok(parts
            .into_iter()
            .map(|(to, amount)| {
                Transaction::Transfer(TransferTx {
                    meta: self.meta(&comment, category.clone()),
                    from: from.clone(),
                    to,
                    asset: asset.clone(),
                    amount,
                    fee: None,
                    network: None,
                    tx_hash: None,
                    debt: None,
                    price: Default::default(),
                })
            })
            .collect())
    }

    fn said_commit(&self, def: &NodeDef, txs: &[Transaction], queued: bool) -> Step {
        let (ru, en): (Vec<String>, Vec<String>) = txs.iter().map(|tx| self.describe(tx)).unzip();
        let (ru, en) = (ru.join("; "), en.join("; "));
        if queued {
            Step::next(
                "out",
                format!("«{}»: в очередь на подтверждение — {ru}", def.title.ru),
                format!("«{}»: queued for confirmation: {en}", def.title.en),
            )
            .with_status(StepStatus::Queued)
        } else {
            Step::next(
                "out",
                format!("«{}»: {ru}", def.title.ru),
                format!("«{}»: {en}", def.title.en),
            )
        }
    }

    pub(crate) fn run_action(&mut self, node: &Node, def: &NodeDef) -> Result<Step, Fault> {
        let now = self.ctx.now;
        match node.kind.as_str() {
            "action.transaction" => {
                let tx = self.build_transaction(node)?;
                let summary = self.describe(&tx).0;
                let (saved, queued) = self.commit(vec![tx], &summary)?;
                if let Some(first) = saved.first() {
                    self.set_output(
                        node,
                        "transaction",
                        WfValue::Transaction(Box::new(first.clone())),
                    );
                }
                Ok(self.said_commit(def, &saved, queued))
            }
            "action.distribute" => {
                let txs = self.build_distribution(node)?;
                let summary = txs
                    .iter()
                    .map(|tx| self.describe(tx).0)
                    .collect::<Vec<_>>()
                    .join("; ");
                let (saved, queued) = self.commit(txs, &summary)?;
                let list = saved
                    .iter()
                    .map(|tx| WfValue::Transaction(Box::new(tx.clone())))
                    .collect();
                self.set_output(node, "created", WfValue::List(list));
                Ok(self.said_commit(def, &saved, queued))
            }
            "action.notify" => {
                let text = self.input(node, "text")?.as_text();
                if text.trim().is_empty() {
                    return Err(Fault::Error("write the notice's text".into()));
                }
                let title = self.text_param(node, "title");
                let title = if title.trim().is_empty() {
                    self.ctx.workflow_name.clone()
                } else {
                    title
                };
                let key = format!("wf:{}:{}:{}", self.ctx.run_id, node.id, self.notices.len());
                self.notices.push(crate::service::notice(
                    NoticeCategory::Automation,
                    key,
                    &title,
                    text.clone(),
                    Some(format!("workflow-edit:{}", self.ctx.workflow_id)),
                    crate::calendar::iso(now),
                ));
                Ok(Step::next(
                    "out",
                    format!("«{}»: {text}", def.title.ru),
                    format!("«{}»: {text}", def.title.en),
                ))
            }
            "action.reminder" => {
                let text = self.input(node, "text")?.as_text();
                if text.trim().is_empty() {
                    return Err(Fault::Error("write what to remind about".into()));
                }
                let days = json_number(&self.raw_param(node, "days"))
                    .and_then(|days| days.into_inner().trunc().to_string().parse::<i64>().ok())
                    .unwrap_or(0)
                    .clamp(0, 3650);
                let time = self.text_param(node, "time");
                let mut parts = time.splitn(2, ':');
                let hour = parts
                    .next()
                    .and_then(|part| part.trim().parse().ok())
                    .unwrap_or(10u32)
                    .min(23);
                let minute = parts
                    .next()
                    .and_then(|part| part.trim().parse().ok())
                    .unwrap_or(0u32)
                    .min(59);
                let cal = Calendar::new(&self.ctx.zone);
                let date = cal.date_of(now) + chrono::Duration::days(days);
                let due = crate::calendar::iso(cal.at_local_hm(date, hour, minute));
                self.vault.doc.reminders.push(Reminder::new(&text, &due));
                Ok(Step::next(
                    "out",
                    format!("«{}»: «{text}» на {due}", def.title.ru),
                    format!("«{}»: «{text}» for {due}", def.title.en),
                ))
            }
            "action.confirm" => {
                self.queue_mode = true;
                Ok(Step::next(
                    "out",
                    format!("«{}»: дальше операции ждут подтверждения", def.title.ru),
                    format!(
                        "«{}»: operations from here wait for confirmation",
                        def.title.en
                    ),
                ))
            }
            "action.stop" => {
                let reason = self.text_param(node, "reason");
                let reason = if reason.trim().is_empty() {
                    "stopped by the «Stop» node".to_string()
                } else {
                    reason
                };
                Err(Halt::Stop(reason).into())
            }
            "action.emit" => {
                let name = self.text_param(node, "name").trim().to_string();
                if name.is_empty() {
                    return Err(Fault::Error("name the event".into()));
                }
                let payload = self.input(node, "payload")?;
                self.events.push(EmittedEvent {
                    name: name.clone(),
                    payload,
                });
                Ok(Step::next(
                    "out",
                    format!("«{}»: «{name}»", def.title.ru),
                    format!("«{}»: «{name}»", def.title.en),
                ))
            }
            "action.wait" => {
                let hours = self
                    .number_input(node, "hours")?
                    .unwrap_or(Amount::ZERO)
                    .max(Amount::ZERO);
                let resume_at = crate::calendar::iso(now + (hours.to_f64() * 3600.0) as i64);
                if self.ctx.mode == RunMode::DryRun {
                    return Ok(Step::next(
                        "out",
                        format!(
                            "«{}»: здесь запуск уснул бы до {resume_at}; в эмуляции идём дальше сразу",
                            def.title.ru
                        ),
                        format!(
                            "«{}»: the run would sleep until {resume_at}; the emulation goes on at once",
                            def.title.en
                        ),
                    ));
                }
                Err(Halt::Wait(Box::new(WorkflowWait {
                    id: uuid::Uuid::new_v4().to_string(),
                    workflow_id: self.ctx.workflow_id.clone(),
                    run_id: self.ctx.run_id.clone(),
                    node_id: node.id.clone(),
                    resume_at,
                    memo: self.memo_json(),
                    queue_mode: self.queue_mode,
                    trigger: self.ctx.trigger_label.clone(),
                }))
                .into())
            }
            "action.call" => self.call_workflow(node, def),
            "action.return" => {
                let value = self.input(node, "value")?;
                Err(Halt::Return(Box::new(value)).into())
            }
            other => Err(Fault::Error(format!("unknown node {other}"))),
        }
    }

    /// «Вызвать схему»: run a helper workflow inside this run and take its answer.
    fn call_workflow(&mut self, node: &Node, def: &NodeDef) -> Result<Step, Fault> {
        if self.ctx.call_depth >= MAX_CALL_DEPTH {
            return Err(Fault::Error(format!(
                "calls go deeper than {MAX_CALL_DEPTH}"
            )));
        }
        let target = self.text_param(node, "workflow");
        let callee = self
            .vault
            .doc
            .workflows
            .iter()
            .find(|workflow| workflow.id == target)
            .cloned()
            .ok_or("choose the workflow to call")?;
        let graph = Graph::parse(&callee.graph).map_err(|err| err.to_string())?;
        let entry = graph
            .nodes
            .iter()
            .find(|candidate| candidate.kind == "trigger.called" && !candidate.disabled)
            .map(|candidate| candidate.id.clone())
            .ok_or_else(|| format!("«{}» has no «Called by a workflow» trigger", callee.name))?;
        let payload = self.input(node, "payload")?;
        let mut ctx = RunContext::new(
            &callee.id,
            &callee.name,
            &entry,
            self.ctx.now,
            &self.ctx.zone,
        );
        ctx.run_id.clone_from(&self.ctx.run_id);
        ctx.mode = self.ctx.mode;
        ctx.max_amount = self.ctx.max_amount;
        ctx.call_depth = self.ctx.call_depth + 1;
        ctx.trigger_label = format!("call from {}", self.ctx.workflow_name);
        ctx.trigger_values = std::collections::BTreeMap::from([("payload".to_string(), payload)]);

        let (actions, queue_mode, moved) = (self.actions, self.queue_mode, self.moved);
        let mut child = Executor::new(&mut *self.vault, &graph, &ctx);
        child.actions = actions;
        child.queue_mode = queue_mode;
        child.moved = moved;
        let result = child.start();
        let steps = child.take_trace();
        let Executor {
            actions,
            moved,
            created,
            queued,
            notices,
            events,
            ..
        } = child;
        self.actions = actions;
        self.moved = moved;
        self.created.extend(created);
        self.queued.extend(queued);
        self.notices.extend(notices);
        self.events.extend(events);
        // The helper's steps show under the calling node.
        let prefix = (
            format!("↳ «{}»: ", callee.name),
            format!("↳ «{}»: ", callee.name),
        );
        self.extend_trace(
            steps
                .into_iter()
                .map(|mut step| {
                    step.node_id.clone_from(&node.id);
                    step.text_ru = format!("{}{}", prefix.0, step.text_ru);
                    step.text_en = format!("{}{}", prefix.1, step.text_en);
                    step
                })
                .collect(),
        );
        let answer = match result {
            Ok(()) => WfValue::Null,
            Err(Halt::Return(value)) => *value,
            Err(Halt::Stop(_)) => WfValue::Null,
            Err(Halt::Fail(message)) => {
                return Err(Fault::Error(format!("«{}»: {message}", callee.name)));
            }
            Err(Halt::Wait(_)) => return Err(Fault::Error("a called workflow cannot wait".into())),
        };
        let text = answer.as_text();
        self.set_output(node, "result", answer);
        Ok(Step::next(
            "out",
            format!("«{}»: «{}» ответила {text}", def.title.ru, callee.name),
            format!("«{}»: «{}» answered {text}", def.title.en, callee.name),
        ))
    }
}
