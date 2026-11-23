//! What the demo person spends: the monthly pattern, the months with a spike
//! and the cash a month. Amounts are US dollars in cents; the pattern follows
//! the `year` scenario of the browser mock.

/// One kind of spending: category, what the payment is called, the range of
/// one payment in cents and how many times a month.
pub struct Spend {
    pub category: &'static str,
    pub comment: &'static str,
    pub low: i64,
    pub high: i64,
    pub times: u32,
}

const fn spend(
    category: &'static str,
    comment: &'static str,
    low: i64,
    high: i64,
    times: u32,
) -> Spend {
    Spend {
        category,
        comment,
        low,
        high,
        times,
    }
}

/// Paid from the card every month.
pub const MONTHLY: &[Spend] = &[
    spend("rent_mortgage", "Аренда квартиры", 45_000, 45_000, 1),
    spend("utilities", "ЖКХ", 5_200, 7_800, 1),
    spend("internet", "Интернет", 900, 900, 1),
    spend("mobile", "Связь", 650, 650, 1),
    spend("subscriptions", "Подписки", 1_200, 1_900, 1),
    spend("groceries", "Продукты", 2_600, 4_400, 8),
    spend("food_delivery", "Доставка еды", 900, 1_800, 4),
    spend("transport", "Метро", 800, 1_200, 3),
    spend("taxi", "Такси", 450, 1_100, 4),
    spend("medicine", "Аптека", 600, 2_600, 1),
    spend("clothing", "Одежда", 2_500, 7_500, 1),
    spend("entertainment", "Кино и кафе", 1_500, 3_200, 2),
];

/// Paid from the cash wallet every month.
pub const CASH_MONTHLY: &[Spend] = &[spend("household", "Рынок", 2_000, 3_500, 2)];

/// Months with a spike (1 = January): December gifts, an August trip that
/// takes the month into the red.
pub fn spikes(month: u32) -> &'static [Spend] {
    const DECEMBER: &[Spend] = &[
        spend("gifts", "Подарки к Новому году", 28_000, 36_000, 1),
        spend("entertainment", "Корпоратив и праздники", 6_000, 9_000, 1),
    ];
    const AUGUST: &[Spend] = &[spend("travel", "Отпуск на море", 165_000, 180_000, 1)];
    match month {
        12 => DECEMBER,
        8 => AUGUST,
        _ => &[],
    }
}

/// Salary range in cents.
pub const SALARY: (i64, i64) = (150_000, 165_000);
/// Moved from the card to the savings every month, in cents.
pub const SAVINGS_TRANSFER: i64 = 15_000;
/// Moved from the card to the cash wallet every month, in cents.
pub const CASH_TRANSFER: i64 = 7_000;
