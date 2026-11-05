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
    /// 123: tx ids that were linked to several events; only the earliest link stays.
    pub unlinked_extra: Vec<String>,
    /// 123: tx ids linked twice in one event, merged into one link.
    pub merged_links: Vec<String>,
    /// R7a: (event id, title) of events whose currency went back.
    pub restored_currency: Vec<(String, String)>,
    /// R7b: (event id, title) of events whose organizer went back.
    pub restored_organizer: Vec<(String, String)>,
    /// R7a: purchases at rate 1 in a currency other than the event's, with no
    /// single currency to go back to. A computed flag, not a change.
    pub needs_rate: Vec<String>,
    /// R7b: events with a settlement that names the organizer as its
    /// participant but carries no `organizer` field (an older build wrote it).
    /// A computed flag, not a change.
    pub organizer_flags: Vec<String>,
    /// R9: exec ids dropped from the staged queue because the import is
    /// already booked or discarded. Silent: the decision was already made.
    pub dropped_imports: Vec<String>,
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
        if !self.unlinked_extra.is_empty() {
            parts.push(
                (if ru {
                    "операция была привязана к двум событиям, лишняя привязка снята"
                } else {
                    "an operation was linked to two events, the extra link was removed"
                })
                .into(),
            );
        }
        if !self.merged_links.is_empty() {
            parts.push(
                (if ru {
                    "операция была привязана к событию дважды, привязки объединены"
                } else {
                    "an operation was linked to one event twice, the links were merged"
                })
                .into(),
            );
        }
        if !self.restored_currency.is_empty() {
            let list = names(&self.restored_currency);
            parts.push(if ru {
                format!("валюта события возвращена к той, в которой записаны покупки: {list}")
            } else {
                format!("event currency set back to the one its purchases were made in: {list}")
            });
        }
        if !self.restored_organizer.is_empty() {
            let list = names(&self.restored_organizer);
            parts.push(if ru {
                format!("организатор события возвращён к тому, с кем уже велись расчёты: {list}")
            } else {
                format!(
                    "event organizer set back to the one the settlements were made with: {list}"
                )
            });
        }
        if !self.unrestored.is_empty() {
            parts.push(
                (if ru {
                    "некоторых удалённых записей, которые ещё используются, нет на этом устройстве; их вернёт устройство, на котором они есть"
                } else {
                    "some deleted records that are still in use are not on this device; the device that has them will bring them back"
                })
                .into(),
            );
        }
        if parts.is_empty() {
            return None;
        }
        Some(format!("{}: {}.", self.notice_title(ru), parts.join("; ")))
    }

    /// The title of the notice: "Fixed after sync", or, when nothing was
    /// changed here and only records are awaited from another device
    /// (`unrestored`), a neutral one.
    pub(super) fn notice_title(&self, ru: bool) -> &'static str {
        // Fields that never reach the notice text (silent rules, computed flags)
        // are cleared too; any other list, a new one included, keeps "Fixed".
        let rest = RepairReport {
            unrestored: Vec::new(),
            records: Vec::new(),
            needs_rate: Vec::new(),
            organizer_flags: Vec::new(),
            dropped_imports: Vec::new(),
            ..self.clone()
        };
        match (rest == RepairReport::default(), ru) {
            (true, true) => "Ждём другое устройство",
            (true, false) => "Waiting for another device",
            (false, true) => "Исправлено после синхронизации",
            (false, false) => "Fixed after sync",
        }
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
            "repair:restored:{}:accounts:{}:envelopes:{}:dups:{}:cash:{}:links:{}:shares:{}:linked:{}:{}:currency:{}:organizer:{}:unrestored:{}",
            ids(&self.restored),
            ids(&self.detached_accounts),
            ids(&self.dropped_envelopes),
            self.merged_duplicates.join(","),
            self.merged_cash.join(","),
            self.dropped_class_links,
            self.normalized_shares,
            self.unlinked_extra.join(","),
            self.merged_links.join(","),
            ids(&self.restored_currency),
            ids(&self.restored_organizer),
            self.unrestored.join(",")
        )
    }
}
