//! Twelve months of operations: the salary, the fixed bills, groceries and
//! the rest, a saving every month, and the two spikes.

use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;

use crate::Amount;
use crate::model::store::AssetRef;
use crate::model::transaction::{
    AccountRef, KindDetails, PaymentTx, PriceStamp, TopupTx, Transaction, TransferTx, TxMeta,
};

use super::rng::SeedRng;
use super::spends::{self, Spend};
use super::wallets::{Wallets, usd, usdt};

/// What the wallets start the year with, in cents: card, savings, cash, stablecoin.
const OPENING: [i64; 4] = [400_000, 800_000, 30_000, 150_000];

/// An operation waiting for its turn: the list is sorted by time before it is booked.
pub struct Planned {
    at: String,
    tx: Transaction,
}

struct Builder<'a> {
    wallets: &'a Wallets,
    rng: SeedRng,
    planned: Vec<Planned>,
}

fn amount(cents: i64) -> Amount {
    Amount::new(Decimal::new(cents, 2))
}

fn stamp(date: NaiveDate, hour: u32) -> String {
    let time = date.and_hms_opt(hour, 0, 0).unwrap_or_default();
    time.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn first_of_month(today: NaiveDate, back: i32) -> NaiveDate {
    let index = today.year() * 12 + i32::try_from(today.month0()).unwrap_or(0) - back;
    let month = u32::try_from(index.rem_euclid(12)).unwrap_or(0) + 1;
    NaiveDate::from_ymd_opt(index.div_euclid(12), month, 1).unwrap_or(today)
}

fn days_in_month(first: NaiveDate) -> u32 {
    let next = first_of_month(first, -1);
    u32::try_from((next - first).num_days()).unwrap_or(28)
}

impl Builder<'_> {
    fn meta(&self, at: &str, comment: &str, categories: &[&str]) -> TxMeta {
        let mut meta = TxMeta::new(comment);
        meta.timestamp = at.to_string();
        meta.categories = categories.iter().map(|key| (*key).to_string()).collect();
        meta
    }

    fn topup(
        &mut self,
        at: String,
        store: &str,
        asset: AssetRef,
        cents: i64,
        (comment, category): (&str, &str),
    ) {
        let meta = self.meta(&at, comment, &[category]);
        let tx = Transaction::Topup(TopupTx {
            meta,
            store_id: store.to_string(),
            asset,
            amount: amount(cents),
            fee: None,
            from: None,
            details: KindDetails::default(),
            price: PriceStamp::default(),
        });
        self.planned.push(Planned { at, tx });
    }

    fn pay(&mut self, at: String, store: &str, cents: i64, spend: &Spend) {
        let meta = self.meta(&at, spend.comment, &[spend.category]);
        let tx = Transaction::Payment(PaymentTx {
            meta,
            store_id: store.to_string(),
            asset: usd(),
            amount: amount(cents),
            fee: None,
            to: Some(AccountRef::Raw(spend.comment.to_string())),
            details: KindDetails::default(),
            price: PriceStamp::default(),
        });
        self.planned.push(Planned { at, tx });
    }

    fn transfer(&mut self, at: String, (from, to): (&str, &str), cents: i64, comment: &str) {
        let meta = self.meta(&at, comment, &[]);
        let tx = Transaction::Transfer(TransferTx {
            meta,
            from: AccountRef::Store(from.to_string()),
            to: AccountRef::Store(to.to_string()),
            asset: usd(),
            amount: amount(cents),
            fee: None,
            network: None,
            tx_hash: None,
            debt: None,
            price: PriceStamp::default(),
        });
        self.planned.push(Planned { at, tx });
    }

    /// One month: `first` is its first day; days from `today` on are left out.
    fn month(&mut self, first: NaiveDate, today: NaiveDate) {
        let days = days_in_month(first);
        let day = |number: u32| first.with_day(number.min(days)).unwrap_or(first);
        let wallets = self.wallets;
        let (low, high) = spends::SALARY;
        let salary = self.rng.cents_between(low, high, 1_000);
        if day(5) < today {
            self.topup(
                stamp(day(5), 10),
                &wallets.card,
                usd(),
                salary,
                ("Зарплата", "salary"),
            );
            self.transfer(
                stamp(day(6), 12),
                (&wallets.card, &wallets.savings),
                spends::SAVINGS_TRANSFER,
                "В накопления",
            );
            self.transfer(
                stamp(day(7), 12),
                (&wallets.card, &wallets.cash),
                spends::CASH_TRANSFER,
                "Снял наличные",
            );
        }
        let regular = spends::MONTHLY.iter().map(|spend| (spend, &wallets.card));
        let cash = spends::CASH_MONTHLY
            .iter()
            .map(|spend| (spend, &wallets.cash));
        let spikes = spends::spikes(first.month())
            .iter()
            .map(|spend| (spend, &wallets.card));
        for (spend, store) in regular.chain(cash).chain(spikes) {
            for index in 0..spend.times {
                let slot = u64::from(days / spend.times);
                let offset = u64::from(index) * slot + self.rng.below(slot.max(1));
                let date = day(1 + u32::try_from(offset).unwrap_or(0));
                if date >= today {
                    continue;
                }
                let hour = 9 + u32::try_from(self.rng.below(12)).unwrap_or(0);
                let cents = self.rng.cents_between(spend.low, spend.high, 10);
                self.pay(stamp(date, hour), store, cents, spend);
            }
        }
    }
}

/// The twelve months up to `today`, in the order they happened.
pub fn plan(wallets: &Wallets, today: NaiveDate, seed: u64) -> Vec<Planned> {
    let mut builder = Builder {
        wallets,
        rng: SeedRng::new(seed),
        planned: Vec::new(),
    };
    let start = first_of_month(today, 11);
    let places = [
        (&wallets.card, usd()),
        (&wallets.savings, usd()),
        (&wallets.cash, usd()),
        (&wallets.token, usdt()),
    ];
    for ((store, asset), cents) in places.into_iter().zip(OPENING) {
        builder.topup(
            stamp(start, 0),
            store,
            asset,
            cents,
            ("Остаток на начало года", "opening_balance"),
        );
    }
    for back in (0..12).rev() {
        builder.month(first_of_month(today, back), today);
    }
    // Stable: operations of one moment keep the order they were planned in.
    builder
        .planned
        .sort_by(|left, right| left.at.cmp(&right.at));
    builder.planned
}

impl Planned {
    pub fn into_transaction(self, number: u128) -> Transaction {
        let mut tx = self.tx;
        tx.meta_mut().id = super::ids::transaction(number);
        tx
    }
}
