//! Nodes of the capital plan: the emergency fund gap, the key rate and
//! inflation, recurring payments and where the free money should go.

use serde_json::json;

use super::catalog::{NodeDef, NodeGroup, ParamKind, port, txt, wired};
use super::types::PortType as Ty;

pub(super) fn capital_nodes(out: &mut Vec<NodeDef>) {
    use NodeGroup::Data;

    out.push(NodeDef {
        kind: "data.emergency_gap",
        group: Data,
        title: txt("Не хватает в подушке", "Emergency fund gap"),
        description: txt(
            "Сколько из суммы положить в подушку, чтобы дойти до цели, и сколько останется.",
            "How much of an amount goes into the emergency fund to reach its target, and what is left.",
        ),
        help: txt(
            "Цель подушки — «Аналитика → Чекап». «В подушку» — меньшее из суммы и недостачи, «Остаток» — всё остальное. Подушка не настроена или полна — всё идёт в «Остаток». «Счёт подушки» — первый её счёт.",
            "The fund's target lives in «Analytics → Health». «To the fund» is the smaller of the amount and the gap, «Rest» is everything else. With no fund or a full one everything goes to «Rest». «Fund account» is its first account.",
        ),
        inputs: vec![port("amount", Ty::Money, txt("Сумма", "Amount"))],
        outputs: vec![
            port("to_cushion", Ty::Money, txt("В подушку", "To the fund")),
            port("rest", Ty::Money, txt("Остаток", "Rest")),
            port("gap", Ty::Money, txt("Недостача", "Gap")),
            port("store", Ty::Account, txt("Счёт подушки", "Fund account")),
        ],
        params: vec![],
        advanced: false,
        keywords: vec!["подушка", "резерв", "emergency", "cushion", "reserve"],
    });
    out.push(NodeDef {
        kind: "data.key_rate",
        group: Data,
        title: txt("Ключевая ставка", "Key rate"),
        description: txt(
            "Ключевая ставка ЦБ на сегодня, % годовых.",
            "The central bank key rate today, % a year.",
        ),
        help: txt(
            "Приложение загружает ставку раз в сутки. Пока её нет — пусто.",
            "The app loads the rate once a day. Until then it is empty.",
        ),
        inputs: vec![],
        outputs: vec![port("rate", Ty::Number, txt("Ставка, %", "Rate, %"))],
        params: vec![],
        advanced: false,
        keywords: vec!["ключевая", "ставка", "цб", "key rate", "central bank"],
    });
    let mut inputs = vec![];
    let params = vec![wired(
        "currency",
        ParamKind::Currency,
        Ty::Currency,
        txt("Валюта", "Currency"),
        json!("rub"),
        &mut inputs,
    )];
    out.push(NodeDef {
        kind: "data.inflation",
        group: Data,
        title: txt("Инфляция", "Inflation"),
        description: txt(
            "Последняя инфляция валюты год к году, %.",
            "The latest year-on-year inflation of a currency, %.",
        ),
        help: txt(
            "Берёт ряд, который приложение загружает само или из вашего источника (Настройки → Инфляция). Ручное значение из настроек устройства сюда не попадает.",
            "Takes the series the app loads itself or from your source (Settings → Inflation). A hand-entered value from the device settings does not reach here.",
        ),
        inputs,
        outputs: vec![
            port("value", Ty::Number, txt("Инфляция, %", "Inflation, %")),
            port("month", Ty::Text, txt("Месяц", "Month")),
        ],
        params,
        advanced: false,
        keywords: vec!["инфляция", "цены", "inflation", "cpi"],
    });
    out.push(NodeDef {
        kind: "data.recurring",
        group: Data,
        title: txt("Повторяющийся платёж", "Recurring payment"),
        description: txt(
            "Был ли такой платёж раньше и не подорожал ли он.",
            "Whether the payment happened before and whether it got dearer.",
        ),
        help: txt(
            "Ищет прошлый платёж тому же получателю (контакт, кому или комментарий) в той же категории за последние 45 дней. «Новый» — такого не было, «Подорожал» — сумма больше прошлой.",
            "Looks for an earlier payment to the same payee (contact, whom or comment) in the same category within 45 days. «New» means there was none, «Dearer» means the amount is above the last one.",
        ),
        inputs: vec![port(
            "transaction",
            Ty::Transaction,
            txt("Операция", "Operation"),
        )],
        outputs: vec![
            port("new", Ty::Bool, txt("Новый", "New")),
            port("grew", Ty::Bool, txt("Подорожал", "Dearer")),
            port("previous", Ty::Money, txt("Прошлая сумма", "Last amount")),
            port("change_pct", Ty::Number, txt("Рост, %", "Change, %")),
        ],
        params: vec![],
        advanced: false,
        keywords: vec!["подписка", "регулярный", "subscription", "recurring"],
    });
    let mut inputs = vec![];
    let params = vec![wired(
        "amount",
        ParamKind::Number,
        Ty::Number,
        txt(
            "Сумма (0 — накопления прошлого месяца)",
            "Amount (0 is last month's savings)",
        ),
        json!("0"),
        &mut inputs,
    )];
    out.push(NodeDef {
        kind: "data.topup_advice",
        group: Data,
        title: txt("Куда пополнить по долям", "Top-up by shares"),
        description: txt(
            "Делит сумму между корзинами так, чтобы доли шли к целевым.",
            "Splits an amount over the baskets so the shares move towards their targets.",
        ),
        help: txt(
            "Корзины и целевые доли — «Аналитика → Инвестиции». «Текст» — строки «корзина: сумма» для уведомления. Переводов не создаёт.",
            "Baskets and target shares live in «Analytics → Investments». «Text» has «basket: amount» lines for a notification. Creates no transfers.",
        ),
        inputs,
        outputs: vec![
            port("text", Ty::Text, txt("Текст", "Text")),
            port("total", Ty::Money, txt("Сумма", "Amount")),
            port("list", Ty::List, txt("Корзины", "Baskets")),
        ],
        params,
        advanced: false,
        keywords: vec!["доли", "ребалансировка", "корзины", "rebalance", "shares"],
    });
}
