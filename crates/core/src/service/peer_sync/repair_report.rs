//! What one pass of `repair_invariants` did, and the sentence the user reads.

/// What one pass did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RepairReport {
    /// R2: names of the accounts made wallets of their own.
    pub detached_accounts: Vec<(String, String)>,
    /// R4: names of the envelopes of goals that are gone.
    pub dropped_envelopes: Vec<(String, String)>,
    /// Natural keys: ids of the duplicate records folded into another one.
    pub merged_duplicates: Vec<String>,
    /// R1: ids of the root Cash wallets merged into the winner.
    pub merged_cash: Vec<String>,
    /// R5: links to an asset class that is gone, dropped.
    pub dropped_class_links: usize,
    /// Shares that did not add up to 100 were scaled.
    pub normalized_shares: bool,
    /// R3: (id, name) of the records put back because live money names them.
    pub restored: Vec<(String, String)>,
    /// R3: ids that money names but no previous value is known here; the
    /// device that has it restores it. Not a change, so not in `is_empty`.
    pub unrestored: Vec<String>,
    /// Keys of every record the pass changed (R8 included).
    pub records: Vec<String>,
}

impl RepairReport {
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The sentence the user reads; `None` when no rule that is worth telling
    /// about fired (R8 stays silent, as it always was).
    pub(super) fn notice_body(&self, ru: bool) -> Option<String> {
        let names = |list: &[(String, String)]| {
            list.iter()
                .map(|(_, name)| format!("«{name}»"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let mut parts = Vec::new();
        if !self.restored.is_empty() {
            let list = names(&self.restored);
            parts.push(if ru {
                format!("восстановлено: {list} — на них ссылаются операции")
            } else {
                format!("restored: {list} — operations still use them")
            });
        }
        if !self.detached_accounts.is_empty() {
            let list = names(&self.detached_accounts);
            parts.push(if ru {
                format!(
                    "счета без подходящего родителя стали отдельными архивными кошельками: {list}"
                )
            } else {
                format!("accounts left without a valid parent became archived wallets: {list}")
            });
        }
        if !self.dropped_envelopes.is_empty() {
            let list = names(&self.dropped_envelopes);
            parts.push(if ru {
                format!("удалены конверты удалённых целей (деньги остались на счёте): {list}")
            } else {
                format!(
                    "removed envelopes of deleted goals (the money stays on the account): {list}"
                )
            });
        }
        if !self.merged_duplicates.is_empty() {
            let count = self.merged_duplicates.len();
            parts.push(if ru {
                format!("объединены дубликаты записей: {count}")
            } else {
                format!("merged duplicate records: {count}")
            });
        }
        if !self.merged_cash.is_empty() {
            parts.push(
                (if ru {
                    "объединены два кошелька «Наличные»"
                } else {
                    "two Cash wallets were merged into one"
                })
                .into(),
            );
        }
        if self.dropped_class_links > 0 {
            let count = self.dropped_class_links;
            parts.push(if ru {
                format!("сброшены ссылки на удалённые классы активов: {count}")
            } else {
                format!("cleared links to deleted asset classes: {count}")
            });
        }
        if self.normalized_shares {
            parts.push(if ru {
                "доли классов активов приведены к 100 %".to_string()
            } else {
                "asset class shares scaled to 100 %".to_string()
            });
        }
        if parts.is_empty() {
            return None;
        }
        let head = if ru {
            "Исправлено после синхронизации"
        } else {
            "Fixed after sync"
        };
        Some(format!("{head}: {}.", parts.join("; ")))
    }

    /// Same repair, same key: two devices that made it share one notice.
    pub(super) fn notice_key(&self) -> String {
        let ids = |list: &[(String, String)]| {
            list.iter()
                .map(|(id, _)| id.as_str())
                .collect::<Vec<_>>()
                .join(",")
        };
        format!(
            "repair:restored:{}:accounts:{}:envelopes:{}:dups:{}:cash:{}:links:{}:shares:{}",
            ids(&self.restored),
            ids(&self.detached_accounts),
            ids(&self.dropped_envelopes),
            self.merged_duplicates.join(","),
            self.merged_cash.join(","),
            self.dropped_class_links,
            self.normalized_shares
        )
    }
}
