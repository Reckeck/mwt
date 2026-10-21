//! The node catalog: every node there is, its ports, their types and its
//! parameters, with titles, descriptions and help in Russian and English.
//! The editor draws nodes from it and the executor runs by it, so a new node
//! is added here and in `nodes/*` only.

use std::sync::OnceLock;

use serde::Serialize;

use super::types::PortType;

/// A text in both languages.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, ts_rs::TS)]
#[ts(rename = "LocalizedText")]
pub struct Text {
    pub ru: &'static str,
    pub en: &'static str,
}

pub(super) const fn txt(ru: &'static str, en: &'static str) -> Text {
    Text { ru, en }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum NodeGroup {
    Trigger,
    Data,
    Logic,
    Variable,
    Action,
    List,
    /// Days and moments: parts, shifting, working days.
    Date,
    /// Text, numbers in text, converting a value to another type.
    Text,
}

#[derive(Debug, Clone, PartialEq, Serialize, ts_rs::TS)]
pub struct PortDef {
    pub name: &'static str,
    #[serde(rename = "type")]
    pub ty: PortType,
    pub label: Text,
}

/// What kind of field edits a parameter.
#[derive(Debug, Clone, PartialEq, Serialize, ts_rs::TS)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ParamKind {
    Number,
    Text,
    LongText,
    Bool,
    /// One of the user's wallets (a store id).
    Store,
    /// A wallet, a contact or text (`AccountRef`).
    Account,
    Asset,
    Currency,
    Category,
    /// A day of the month, 1–31.
    Day,
    /// 0 = Monday … 6 = Sunday.
    Weekday,
    /// `HH:MM`.
    Time,
    Date,
    Budget,
    Goal,
    /// Another workflow (its id).
    Workflow,
    /// One of the user's data sources (its id).
    Source,
    Select {
        options: Vec<(&'static str, Text)>,
    },
    /// Recipients of «Распределить»: `[{ account, share }]`.
    Targets,
}

#[derive(Debug, Clone, PartialEq, Serialize, ts_rs::TS)]
pub struct ParamDef {
    pub name: &'static str,
    pub kind: ParamKind,
    pub label: Text,
    /// JSON of the default value.
    #[ts(type = "unknown")]
    pub default: serde_json::Value,
    /// An input of the same name exists: the field can be «taken from a wire».
    pub wirable: bool,
    /// The node cannot work with the field empty and no wire into it: an
    /// active workflow is refused until it is filled (`REQUIRED_PARAMS`).
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, ts_rs::TS)]
pub struct NodeDef {
    pub kind: &'static str,
    pub group: NodeGroup,
    pub title: Text,
    pub description: Text,
    /// Longer help: what it does, an example, common mistakes.
    pub help: Text,
    pub inputs: Vec<PortDef>,
    pub outputs: Vec<PortDef>,
    pub params: Vec<ParamDef>,
    /// Hidden in the simple mode of the editor.
    pub advanced: bool,
    /// Words the search finds it by, besides its title.
    pub keywords: Vec<&'static str>,
}

impl NodeDef {
    pub fn is_trigger(&self) -> bool {
        self.group == NodeGroup::Trigger
    }

    pub fn input(&self, name: &str) -> Option<&PortDef> {
        self.inputs.iter().find(|port| port.name == name)
    }

    pub fn output(&self, name: &str) -> Option<&PortDef> {
        self.outputs.iter().find(|port| port.name == name)
    }

    pub fn param(&self, name: &str) -> Option<&ParamDef> {
        self.params.iter().find(|param| param.name == name)
    }

    /// Whether the node takes part in the flow (has a flow input or output).
    pub fn has_flow(&self) -> bool {
        self.inputs
            .iter()
            .chain(&self.outputs)
            .any(|port| port.ty == PortType::Flow)
    }
}

pub(super) fn port(name: &'static str, ty: PortType, label: Text) -> PortDef {
    PortDef { name, ty, label }
}

pub(super) fn flow_in() -> PortDef {
    port("in", PortType::Flow, txt("Вход", "In"))
}

pub(super) fn flow_out() -> PortDef {
    port("out", PortType::Flow, txt("Дальше", "Next"))
}

pub(super) fn error_out() -> PortDef {
    port("error", PortType::Flow, txt("Ошибка", "Error"))
}

pub(super) fn param(
    name: &'static str,
    kind: ParamKind,
    label: Text,
    default: serde_json::Value,
) -> ParamDef {
    ParamDef {
        name,
        kind,
        label,
        default,
        wirable: false,
        required: false,
    }
}

/// A parameter that also has an input of the same name and type.
pub(super) fn wired(
    name: &'static str,
    kind: ParamKind,
    ty: PortType,
    label: Text,
    default: serde_json::Value,
    inputs: &mut Vec<PortDef>,
) -> ParamDef {
    inputs.push(port(name, ty, label));
    ParamDef {
        name,
        kind,
        label,
        default,
        wirable: true,
        required: false,
    }
}

/// Fields a node does nothing useful without: a price trigger with no asset
/// never fires, a payment with no account fails every run. Conditional ones
/// (the «to» of a transfer) are checked in `validate`.
pub const REQUIRED_PARAMS: &[(&str, &str)] = &[
    ("trigger.price", "asset"),
    ("trigger.balance", "store"),
    ("trigger.source", "source"),
    ("data.source", "source"),
    ("data.balance", "account"),
    ("data.price", "asset"),
    ("action.transaction", "account"),
    ("action.distribute", "from"),
    ("action.distribute", "targets"),
];

fn mark_required(nodes: &mut [NodeDef]) {
    for node in nodes {
        for param in &mut node.params {
            param.required = REQUIRED_PARAMS.contains(&(node.kind, param.name));
        }
    }
}

pub(super) fn select(options: &[(&'static str, Text)]) -> ParamKind {
    ParamKind::Select {
        options: options.to_vec(),
    }
}

const CONDITIONS: [(&str, Text); 2] = [
    ("above", txt("выше или равно", "at or above")),
    ("below", txt("ниже или равно", "at or below")),
];

const SOURCE_CONDITIONS: [(&str, Text); 3] = [
    ("changed", txt("изменилось", "changed")),
    ("above", txt("выше или равно", "at or above")),
    ("below", txt("ниже или равно", "at or below")),
];

use serde_json::json;

#[allow(clippy::too_many_lines, clippy::vec_init_then_push)]
fn build() -> Vec<NodeDef> {
    use NodeGroup::*;
    use PortType as Ty;
    let mut out = Vec::new();

    // -- triggers -------------------------------------------------------------
    out.push(NodeDef {
        kind: "trigger.manual",
        group: Trigger,
        title: txt("Запуск вручную", "Run by hand"),
        description: txt(
            "Схема запускается кнопкой «Запустить» в редакторе или в списке.",
            "The workflow runs from the «Run» button in the editor or the list.",
        ),
        help: txt(
            "Сам по себе не срабатывает никогда — только по кнопке «Запустить» (и он для неё по умолчанию). Нужен для схем, которые работают лишь по вашему решению. Другие триггеры кнопка тоже умеет запустить — стрелка «Запустить от…», чтобы проверить их сейчас. Пример: «Запуск вручную» → «Распределить» остаток карты по конвертам.",
            "Never fires by itself — only from the «Run» button (it is the button's default). For workflows that run only when you decide. The button can start the other triggers too — the «Run from…» arrow, to try them now. Example: «Run by hand» → «Distribute» a card's balance into envelopes.",
        ),
        inputs: vec![],
        outputs: vec![flow_out(), port("at", Ty::Date, txt("Когда", "When"))],
        params: vec![],
        advanced: false,
        keywords: vec!["кнопка", "вручную", "manual", "button", "start"],
    });
    out.push(NodeDef {
        kind: "trigger.schedule",
        group: Trigger,
        title: txt("По расписанию", "On a schedule"),
        description: txt(
            "Каждый день, неделю или месяц в заданное время.",
            "Every day, week or month at a set time.",
        ),
        help: txt(
            "«Каждый месяц, 10-е, 09:00» — для зарплаты и платежей; «каждую неделю, пятница» — для DCA. «Выходной или праздник»: дата, выпавшая на субботу, воскресенье или праздник России, переносится на рабочий день — так платят зарплату. Пропущенные, пока vault был заблокирован, даты выполнятся при разблокировке (не больше 12).",
            "«Every month, the 10th, 09:00» for a salary or payments; «every week, Friday» for DCA. «A day off or a holiday»: a date on a Saturday, a Sunday or a Russian public holiday moves to a working day, the way salaries are paid. Dates missed while the vault was locked run at unlock (12 at most).",
        ),
        inputs: vec![],
        outputs: vec![flow_out(), port("at", Ty::Date, txt("Когда", "When"))],
        params: vec![
            param(
                "every",
                select(&[
                    ("day", txt("каждый день", "every day")),
                    ("week", txt("каждую неделю", "every week")),
                    ("month", txt("каждый месяц", "every month")),
                ]),
                txt("Как часто", "How often"),
                json!("month"),
            ),
            param("interval", ParamKind::Number, txt("Через сколько", "Every how many"), json!("1")),
            param("day", ParamKind::Day, txt("День месяца", "Day of the month"), json!(1)),
            param("weekday", ParamKind::Weekday, txt("День недели", "Weekday"), json!(4)),
            param("time", ParamKind::Time, txt("Время", "Time"), json!("09:00")),
            param(
                "workday",
                select(&[
                    ("none", txt("не переносить", "keep the date")),
                    ("back", txt("на рабочий день раньше", "to the working day before")),
                    ("forward", txt("на рабочий день позже", "to the working day after")),
                ]),
                txt("Выходной или праздник", "A day off or a holiday"),
                json!("none"),
            ),
        ],
        advanced: false,
        keywords: vec![
            "расписание", "время", "каждый", "cron", "schedule", "daily", "weekly", "monthly", "выходной",
            "рабочий",
        ],
    });
    out.push(NodeDef {
        kind: "trigger.transaction",
        group: Trigger,
        title: txt("Добавлена операция", "Operation added"),
        description: txt(
            "Срабатывает на новую операцию, подходящую под фильтр.",
            "Fires on a new operation that matches the filter.",
        ),
        help: txt(
            "Фильтр сужает: вид, кошелёк, категория, сумма от, текст в комментарии. Пример: категория «Зарплата» → «Распределить» 10 % в накопления. Операции, созданные схемами, по умолчанию не запускают её — так не бывает бесконечных цепочек.",
            "The filter narrows it: kind, wallet, category, minimum amount, text in the comment. Example: category «Salary» → «Distribute» 10% into savings. Operations made by workflows do not fire it by default, so chains never loop.",
        ),
        inputs: vec![],
        outputs: vec![
            flow_out(),
            port("transaction", Ty::Transaction, txt("Операция", "Operation")),
            port("amount", Ty::Money, txt("Сумма", "Amount")),
            port("account", Ty::Account, txt("Счёт", "Account")),
            port("category", Ty::Category, txt("Категория", "Category")),
        ],
        params: vec![
            param(
                "tx_kind",
                select(&[
                    ("any", txt("любая", "any")),
                    ("topup", txt("пополнение", "top-up")),
                    ("payment", txt("оплата", "payment")),
                    ("transfer", txt("перевод", "transfer")),
                    ("convert", txt("обмен", "exchange")),
                    ("trade_spot", txt("спот-сделка", "spot trade")),
                ]),
                txt("Вид", "Kind"),
                json!("any"),
            ),
            param("store", ParamKind::Store, txt("Кошелёк (с субсчетами)", "Wallet (with sub-accounts)"), json!(null)),
            param("category", ParamKind::Category, txt("Категория", "Category"), json!(null)),
            param("min_amount", ParamKind::Number, txt("Сумма от", "Amount from"), json!(null)),
            param("text", ParamKind::Text, txt("Текст в комментарии", "Text in the comment"), json!("")),
            param(
                "include_workflows",
                ParamKind::Bool,
                txt("И операции схем", "Workflows' operations too"),
                json!(false),
            ),
        ],
        advanced: false,
        keywords: vec!["операция", "новая", "зарплата", "трата", "transaction", "income", "on add"],
    });
    out.push(NodeDef {
        kind: "trigger.balance",
        group: Trigger,
        title: txt("Баланс пересёк порог", "Balance crossed a threshold"),
        description: txt(
            "Когда баланс счёта становится выше или ниже порога.",
            "When an account's balance goes above or below a threshold.",
        ),
        help: txt(
            "Срабатывает один раз при пересечении, не на каждой проверке. Пример: баланс карты ниже 5 000 ₽ → уведомление и перевод с накопительного.",
            "Fires once per crossing, not on every check. Example: card balance below 5 000 ₽ → a notice and a transfer from savings.",
        ),
        inputs: vec![],
        outputs: vec![flow_out(), port("balance", Ty::Money, txt("Баланс", "Balance"))],
        params: vec![
            param("store", ParamKind::Store, txt("Счёт", "Account"), json!(null)),
            param("asset", ParamKind::Asset, txt("Актив", "Asset"), json!(null)),
            param("condition", select(&CONDITIONS), txt("Условие", "Condition"), json!("below")),
            param("value", ParamKind::Number, txt("Порог", "Threshold"), json!("0")),
        ],
        advanced: false,
        keywords: vec!["баланс", "остаток", "порог", "мало", "balance", "low", "threshold"],
    });
    out.push(NodeDef {
        kind: "trigger.price",
        group: Trigger,
        title: txt("Цена пересекла порог", "Price crossed a threshold"),
        description: txt(
            "Когда цена актива в выбранной валюте выше или ниже порога.",
            "When an asset's price in a currency is above or below a threshold.",
        ),
        help: txt(
            "Цены обновляются раз в несколько минут, пока приложение открыто. Срабатывает один раз при пересечении.",
            "Prices refresh every few minutes while the app is open. Fires once per crossing.",
        ),
        inputs: vec![],
        outputs: vec![flow_out(), port("price", Ty::Money, txt("Цена", "Price"))],
        params: vec![
            param("asset", ParamKind::Asset, txt("Актив", "Asset"), json!(null)),
            param("condition", select(&CONDITIONS), txt("Условие", "Condition"), json!("above")),
            param("value", ParamKind::Number, txt("Порог", "Threshold"), json!("0")),
            param("currency", ParamKind::Currency, txt("Валюта", "Currency"), json!("usd")),
        ],
        advanced: false,
        keywords: vec!["цена", "курс", "алерт", "price", "alert"],
    });
    out.push(NodeDef {
        kind: "trigger.source",
        group: Trigger,
        title: txt("Новое значение источника", "Data source changed"),
        description: txt(
            "Когда ваш источник данных принёс новое значение или пересёк порог.",
            "When one of your data sources brings a new value or crosses a threshold.",
        ),
        help: txt(
            "Источники настраиваются в «Планирование → Источники» и обновляются в фоне по своему интервалу. «Изменилось» срабатывает на каждое новое значение, порог — один раз при пересечении.",
            "Sources are set up in «Planning → Sources» and refresh in the background on their own interval. «Changed» fires on every new value, a threshold once per crossing.",
        ),
        inputs: vec![],
        outputs: vec![
            flow_out(),
            port("value", Ty::Number, txt("Значение", "Value")),
            port("text", Ty::Text, txt("Текст", "Text")),
        ],
        params: vec![
            param("source", ParamKind::Source, txt("Источник", "Source"), json!(null)),
            param(
                "condition",
                select(&SOURCE_CONDITIONS),
                txt("Условие", "Condition"),
                json!("changed"),
            ),
            param("value", ParamKind::Number, txt("Порог", "Threshold"), json!("0")),
        ],
        advanced: false,
        keywords: vec!["источник", "сайт", "инфляция", "ставка", "source", "website", "scrape"],
    });
    out.push(NodeDef {
        kind: "trigger.event",
        group: Trigger,
        title: txt("Событие схемы", "Workflow event"),
        description: txt(
            "Срабатывает, когда другая схема отправила событие с этим именем.",
            "Fires when another workflow sends an event with this name.",
        ),
        help: txt(
            "Так одна схема продолжает другую: «Отправить событие „ипотека-оплачена“» → здесь «Событие схемы „ипотека-оплачена“». Данные события приходят в выход «Данные».",
            "This is how one workflow continues another: «Send event “mortgage-paid”» → here «Workflow event “mortgage-paid”». The event's data comes out of «Data».",
        ),
        inputs: vec![],
        outputs: vec![flow_out(), port("payload", Ty::Any, txt("Данные", "Data"))],
        params: vec![param("name", ParamKind::Text, txt("Имя события", "Event name"), json!(""))],
        advanced: true,
        keywords: vec!["событие", "сигнал", "event", "signal"],
    });

    out.push(NodeDef {
        kind: "trigger.called",
        group: Trigger,
        title: txt("Вызов из другой схемы", "Called by a workflow"),
        description: txt(
            "Схема-помощник: её запускает узел «Вызвать схему» и ждёт результата.",
            "A helper workflow: a «Call a workflow» node runs it and waits for its result.",
        ),
        help: txt(
            "Так общую часть пишут один раз: например, «перевести на накопительный с проверкой лимита». Данные вызова приходят в выход «Данные», ответ отдаёт узел «Вернуть значение». Такую схему не нужно включать — она работает только по вызову.",
            "Write a shared part once: for instance «move to savings within the limit». The call's data comes out of «Data», the answer goes back through «Return a value». Such a workflow need not be switched on: it runs only when called.",
        ),
        inputs: vec![],
        outputs: vec![flow_out(), port("payload", Ty::Any, txt("Данные", "Data"))],
        params: vec![],
        advanced: true,
        keywords: vec!["вызов", "подсхема", "функция", "помощник", "call", "subflow", "function", "helper"],
    });

    // -- data -----------------------------------------------------------------
    let mut inputs = Vec::new();
    let params = vec![
        wired(
            "account",
            ParamKind::Account,
            Ty::Account,
            txt("Счёт", "Account"),
            json!(null),
            &mut inputs,
        ),
        wired(
            "asset",
            ParamKind::Asset,
            Ty::Asset,
            txt("Актив", "Asset"),
            json!(null),
            &mut inputs,
        ),
    ];
    out.push(NodeDef {
        kind: "data.balance",
        group: Data,
        title: txt("Баланс счёта", "Account balance"),
        description: txt("Сколько актива на счёте сейчас.", "How much of an asset an account holds now."),
        help: txt(
            "Счёт с субсчетами считается целиком. Без актива — баланс в базовой валюте по всем активам.",
            "An account with sub-accounts counts whole. Without an asset: the balance in the base currency over all assets.",
        ),
        inputs,
        outputs: vec![port("value", Ty::Money, txt("Баланс", "Balance"))],
        params,
        advanced: false,
        keywords: vec!["баланс", "остаток", "сколько", "balance", "holdings"],
    });
    let mut inputs = Vec::new();
    let params = vec![
        wired(
            "asset",
            ParamKind::Asset,
            Ty::Asset,
            txt("Актив", "Asset"),
            json!(null),
            &mut inputs,
        ),
        wired(
            "currency",
            ParamKind::Currency,
            Ty::Currency,
            txt("Валюта", "Currency"),
            json!("usd"),
            &mut inputs,
        ),
    ];
    out.push(NodeDef {
        kind: "data.price",
        group: Data,
        title: txt("Цена актива", "Asset price"),
        description: txt(
            "Последняя известная цена актива в валюте.",
            "The latest known price of an asset in a currency.",
        ),
        help: txt(
            "Берётся из кэша цен; без сети — последняя сохранённая.",
            "From the price cache; offline, the last one saved.",
        ),
        inputs,
        outputs: vec![port("price", Ty::Money, txt("Цена", "Price"))],
        params,
        advanced: false,
        keywords: vec!["цена", "курс", "стоимость", "price", "quote"],
    });
    let mut inputs = Vec::new();
    let params = vec![
        wired(
            "from",
            ParamKind::Currency,
            Ty::Currency,
            txt("Из", "From"),
            json!("usd"),
            &mut inputs,
        ),
        wired(
            "to",
            ParamKind::Currency,
            Ty::Currency,
            txt("В", "To"),
            json!("rub"),
            &mut inputs,
        ),
    ];
    out.push(NodeDef {
        kind: "data.fx",
        group: Data,
        title: txt("Курс валют", "Exchange rate"),
        description: txt(
            "Сколько единиц «В» за одну «Из».",
            "How many «To» for one «From».",
        ),
        help: txt("Из кэша курсов.", "From the FX cache."),
        inputs,
        outputs: vec![port("rate", Ty::Number, txt("Курс", "Rate"))],
        params,
        advanced: false,
        keywords: vec!["курс", "валюта", "fx", "rate", "exchange"],
    });
    out.push(NodeDef {
        kind: "data.constant",
        group: Data,
        title: txt("Значение", "Value"),
        description: txt("Число, сумма, текст, счёт или актив, заданные здесь.", "A number, an amount, text, an account or an asset set here."),
        help: txt(
            "Для порогов и сумм, которые используются в нескольких местах; общую для всех схем величину лучше держать в переменной.",
            "For thresholds and sums used in several places; a value shared by all workflows is better kept in a variable.",
        ),
        inputs: vec![],
        outputs: vec![port("value", Ty::Any, txt("Значение", "Value"))],
        params: vec![
            param(
                "value_type",
                select(&[
                    ("number", txt("число", "number")),
                    ("money", txt("сумма", "amount")),
                    ("text", txt("текст", "text")),
                    ("bool", txt("да/нет", "yes/no")),
                    ("account", txt("счёт", "account")),
                    ("asset", txt("актив", "asset")),
                    ("category", txt("категория", "category")),
                ]),
                txt("Тип", "Type"),
                json!("number"),
            ),
            param("number", ParamKind::Number, txt("Число", "Number"), json!("0")),
            param("currency", ParamKind::Currency, txt("Валюта суммы", "Currency"), json!("rub")),
            param("text", ParamKind::Text, txt("Текст", "Text"), json!("")),
            param("bool", ParamKind::Bool, txt("Да/нет", "Yes/no"), json!(true)),
            param("account", ParamKind::Account, txt("Счёт", "Account"), json!(null)),
            param("asset", ParamKind::Asset, txt("Актив", "Asset"), json!(null)),
            param("category", ParamKind::Category, txt("Категория", "Category"), json!(null)),
        ],
        advanced: false,
        keywords: vec!["значение", "константа", "число", "value", "constant", "number"],
    });
    out.push(NodeDef {
        kind: "data.sum",
        group: Data,
        title: txt("Сумма операций за период", "Sum over a period"),
        description: txt(
            "Доходы, траты или всё за текущий день, неделю, месяц или год, в базовой валюте.",
            "Income, spending or everything for the current day, week, month or year, in the base currency.",
        ),
        help: txt(
            "Те же цифры, что в отчётах: переводы между своими счетами и корректировки не считаются.",
            "The same figures as the reports: moves between your own accounts and corrections do not count.",
        ),
        inputs: vec![],
        outputs: vec![port("total", Ty::Money, txt("Итого", "Total"))],
        params: vec![
            param(
                "period",
                select(&[
                    ("day", txt("сегодня", "today")),
                    ("week", txt("эта неделя", "this week")),
                    ("month", txt("этот месяц", "this month")),
                    ("year", txt("этот год", "this year")),
                ]),
                txt("Период", "Period"),
                json!("month"),
            ),
            param(
                "flow",
                select(&[
                    ("income", txt("доходы", "income")),
                    ("expense", txt("траты", "spending")),
                ]),
                txt("Что", "What"),
                json!("expense"),
            ),
            param("category", ParamKind::Category, txt("Категория", "Category"), json!(null)),
        ],
        advanced: false,
        keywords: vec!["сумма", "итого", "траты", "доход", "месяц", "sum", "total", "spent"],
    });
    let mut inputs = vec![];
    let params = vec![
        param(
            "count",
            ParamKind::Number,
            txt("Сколько последних", "How many latest"),
            json!("3"),
        ),
        wired(
            "fallback",
            ParamKind::Number,
            Ty::Number,
            txt("Если операций ещё нет", "When there are none yet"),
            json!("0"),
            &mut inputs,
        ),
    ];
    out.push(NodeDef {
        kind: "data.recent_average",
        group: Data,
        title: txt("Средняя сумма прошлых операций", "Average of past operations"),
        description: txt(
            "Среднее по последним операциям, которые сделала эта схема.",
            "The average of the latest operations this workflow made.",
        ),
        help: txt(
            "Для зарплаты, которая каждый раз разная: берёт суммы последних операций этой схемы, кроме ещё не уточнённых, и считает среднее. Пока операций нет — «Если операций ещё нет».",
            "For a salary that differs every time: takes the latest operations of this workflow, except the ones not corrected yet, and averages them. With none yet it gives «When there are none yet».",
        ),
        inputs,
        outputs: vec![port("amount", Ty::Money, txt("Сумма", "Amount"))],
        params,
        advanced: false,
        keywords: vec!["среднее", "зарплата", "примерно", "average", "salary", "estimate"],
    });
    out.push(NodeDef {
        kind: "data.source",
        group: Data,
        title: txt("Значение источника", "Data source value"),
        description: txt(
            "Последнее значение вашего источника данных.",
            "The latest value of one of your data sources.",
        ),
        help: txt(
            "Источник — страница, JSON или файл, откуда приложение берёт число («Планирование → Источники»). Ряд приходит списком чисел.",
            "A source is a page, JSON or file the app reads a number from («Planning → Sources»). A series comes as a list of numbers.",
        ),
        inputs: vec![],
        outputs: vec![
            port("value", Ty::Number, txt("Значение", "Value")),
            port("text", Ty::Text, txt("Текст", "Text")),
            port("series", Ty::List, txt("Ряд", "Series")),
        ],
        params: vec![param(
            "source",
            ParamKind::Source,
            txt("Источник", "Source"),
            json!(null),
        )],
        advanced: false,
        keywords: vec!["источник", "сайт", "парсинг", "source", "website", "scrape"],
    });
    out.push(NodeDef {
        kind: "data.budget",
        group: Data,
        title: txt("Остаток бюджета", "Budget left"),
        description: txt(
            "Сколько осталось по лимиту в текущем периоде.",
            "What is left of a limit this period.",
        ),
        help: txt(
            "Выберите лимит из «Планирование → Лимиты».",
            "Pick a limit from «Planning → Limits».",
        ),
        inputs: vec![],
        outputs: vec![
            port("remaining", Ty::Money, txt("Осталось", "Left")),
            port("used", Ty::Number, txt("Израсходовано, %", "Used, %")),
            port(
                "elapsed",
                Ty::Number,
                txt("Прошло периода, %", "Period passed, %"),
            ),
        ],
        params: vec![param(
            "budget",
            ParamKind::Budget,
            txt("Лимит", "Limit"),
            json!(null),
        )],
        advanced: false,
        keywords: vec!["бюджет", "лимит", "budget", "limit"],
    });
    out.push(NodeDef {
        kind: "data.goal",
        group: Data,
        title: txt("Прогресс цели", "Goal progress"),
        description: txt(
            "Сколько накоплено и сколько осталось.",
            "What is saved and what is left.",
        ),
        help: txt(
            "Выберите цель из «Планирование → Цели».",
            "Pick a goal from «Planning → Goals».",
        ),
        inputs: vec![],
        outputs: vec![
            port("saved", Ty::Money, txt("Накоплено", "Saved")),
            port("left", Ty::Money, txt("Осталось", "Left")),
            port("pct", Ty::Number, txt("Процент", "Percent")),
        ],
        params: vec![param(
            "goal",
            ParamKind::Goal,
            txt("Цель", "Goal"),
            json!(null),
        )],
        advanced: false,
        keywords: vec!["цель", "накопления", "goal", "saving"],
    });
    out.push(NodeDef {
        kind: "data.now",
        group: Data,
        title: txt("Сейчас", "Now"),
        description: txt(
            "Текущие дата, день месяца и день недели.",
            "The current date, day of month and weekday.",
        ),
        help: txt(
            "День недели: 0 — понедельник … 6 — воскресенье.",
            "Weekday: 0 is Monday … 6 is Sunday.",
        ),
        inputs: vec![],
        outputs: vec![
            port("now", Ty::Date, txt("Дата", "Date")),
            port("day", Ty::Number, txt("День месяца", "Day of month")),
            port("weekday", Ty::Number, txt("День недели", "Weekday")),
        ],
        params: vec![],
        advanced: false,
        keywords: vec!["сейчас", "дата", "сегодня", "now", "today", "date"],
    });
    out.push(NodeDef {
        kind: "data.tx_fields",
        group: Data,
        title: txt("Разобрать операцию", "Split an operation"),
        description: txt("Сумма, актив, счёт, категория, комментарий и дата операции.", "An operation's amount, asset, account, category, comment and date."),
        help: txt(
            "Подключите выход «Операция» триггера или действия, чтобы взять её поля по отдельности.",
            "Wire an «Operation» output of a trigger or an action to take its fields one by one.",
        ),
        inputs: vec![port("transaction", Ty::Transaction, txt("Операция", "Operation"))],
        outputs: vec![
            port("amount", Ty::Money, txt("Сумма", "Amount")),
            port("asset", Ty::Asset, txt("Актив", "Asset")),
            port("account", Ty::Account, txt("Счёт", "Account")),
            port("category", Ty::Category, txt("Категория", "Category")),
            port("comment", Ty::Text, txt("Комментарий", "Comment")),
            port("date", Ty::Date, txt("Дата", "Date")),
        ],
        params: vec![],
        advanced: false,
        keywords: vec!["разобрать", "поля", "операция", "fields", "split", "parse"],
    });

    let mut inputs = Vec::new();
    let date = wired(
        "date",
        ParamKind::Date,
        Ty::Date,
        txt("Дата (пусто — сегодня)", "Date (empty: today)"),
        json!(null),
        &mut inputs,
    );
    out.push(NodeDef {
        kind: "data.workday",
        group: Data,
        title: txt("Рабочий день?", "Working day?"),
        description: txt(
            "Рабочий ли день, выходной или праздник, и ближайший рабочий день.",
            "Whether a day is a working day, a weekend or a holiday, and the next working day.",
        ),
        help: txt(
            "Без подключённой даты проверяет сегодняшний день. Календарь «Россия» знает выходные и государственные праздники (1–8 января, 23 февраля, 8 марта, 1 и 9 мая, 12 июня, 4 ноября). Переносы выходных правительство объявляет каждый год: добавьте их сами — дни отдыха в «Ещё выходные», рабочие субботы в «Рабочие дни». Даты пишите через запятую: 2026-01-09, 2026-12-31. Пример: «По расписанию» 10-го → «Рабочий день?» → «Ветвление»: нет — перенести платёж на «Ближайший рабочий».",
            "Without a wired date it checks today. The «Russia» calendar knows weekends and public holidays (January 1–8, February 23, March 8, May 1 and 9, June 12, November 4). Day-off transfers are announced every year, so add them yourself: rest days in «Extra days off», working Saturdays in «Working days». Write dates with commas: 2026-01-09, 2026-12-31. Example: «On a schedule» on the 10th → «Working day?» → «Branch»: no, move the payment to «Next working day».",
        ),
        inputs,
        outputs: vec![
            port("workday", Ty::Bool, txt("Рабочий", "Working")),
            port("weekend", Ty::Bool, txt("Выходной (суббота/воскресенье)", "Weekend")),
            port("holiday", Ty::Bool, txt("Праздник", "Holiday")),
            port("next_workday", Ty::Date, txt("Ближайший рабочий", "Next working day")),
        ],
        params: vec![
            date,
            param(
                "calendar",
                select(&[
                    ("ru", txt("Россия: выходные и праздники", "Russia: weekends and holidays")),
                    ("weekends", txt("только выходные", "weekends only")),
                ]),
                txt("Календарь", "Calendar"),
                json!("ru"),
            ),
            param(
                "weekend",
                select(&[
                    ("sat_sun", txt("суббота и воскресенье", "Saturday and Sunday")),
                    ("sun", txt("только воскресенье", "Sunday only")),
                    ("fri_sat", txt("пятница и суббота", "Friday and Saturday")),
                ]),
                txt("Выходные дни недели", "Weekend days"),
                json!("sat_sun"),
            ),
            param("extra_off", ParamKind::Text, txt("Ещё выходные (даты)", "Extra days off (dates)"), json!("")),
            param("extra_work", ParamKind::Text, txt("Рабочие дни (переносы)", "Working days (transfers)"), json!("")),
        ],
        advanced: false,
        keywords: vec!["рабочий", "выходной", "праздник", "будни", "перенос", "workday", "weekend", "holiday", "business day"],
    });

    // -- logic and math -----------------------------------------------------------
    out.push(NodeDef {
        kind: "logic.compare",
        group: Logic,
        title: txt("Сравнение", "Compare"),
        description: txt("Сравнивает A и B: больше, меньше, равно.", "Compares A and B: greater, less, equal."),
        help: txt(
            "Суммы в разных валютах сравнить нельзя — сначала «Конвертировать». Если B не подключено, берётся значение из поля.",
            "Amounts in different currencies cannot be compared: «Convert» first. If B is not wired, the field's value is used.",
        ),
        inputs: vec![
            port("a", Ty::Any, txt("A", "A")),
            port("b", Ty::Any, txt("B", "B")),
        ],
        outputs: vec![port("result", Ty::Bool, txt("Да/нет", "Yes/no"))],
        params: vec![
            param(
                "op",
                select(&[
                    ("gt", txt("A > B", "A > B")),
                    ("gte", txt("A ≥ B", "A ≥ B")),
                    ("lt", txt("A < B", "A < B")),
                    ("lte", txt("A ≤ B", "A ≤ B")),
                    ("eq", txt("A = B", "A = B")),
                    ("ne", txt("A ≠ B", "A ≠ B")),
                    ("contains", txt("A содержит B", "A contains B")),
                ]),
                txt("Как", "How"),
                json!("gt"),
            ),
            param("b", ParamKind::Number, txt("B, если не подключено", "B when not wired"), json!("0")),
        ],
        advanced: false,
        keywords: vec!["если", "больше", "меньше", "равно", "сравнить", "if", "compare", "greater", "less", ">", "<"],
    });
    for (kind, title, ru_desc, en_desc) in [
        (
            "logic.and",
            txt("И", "And"),
            "Да, когда да оба.",
            "Yes when both are yes.",
        ),
        (
            "logic.or",
            txt("ИЛИ", "Or"),
            "Да, когда да хотя бы одно.",
            "Yes when at least one is yes.",
        ),
    ] {
        out.push(NodeDef {
            kind,
            group: Logic,
            title,
            description: txt(ru_desc, en_desc),
            help: txt(
                "Соединяйте условия «Сравнения».",
                "Joins conditions from «Compare».",
            ),
            inputs: vec![
                port("a", Ty::Bool, txt("A", "A")),
                port("b", Ty::Bool, txt("B", "B")),
            ],
            outputs: vec![port("result", Ty::Bool, txt("Да/нет", "Yes/no"))],
            params: vec![],
            advanced: false,
            keywords: vec!["и", "или", "and", "or", "both"],
        });
    }
    out.push(NodeDef {
        kind: "logic.not",
        group: Logic,
        title: txt("НЕ", "Not"),
        description: txt("Да становится нет и наоборот.", "Yes becomes no and back."),
        help: txt("Переворачивает условие.", "Turns a condition around."),
        inputs: vec![port("a", Ty::Bool, txt("A", "A"))],
        outputs: vec![port("result", Ty::Bool, txt("Да/нет", "Yes/no"))],
        params: vec![],
        advanced: false,
        keywords: vec!["не", "наоборот", "not"],
    });
    out.push(NodeDef {
        kind: "logic.branch",
        group: Logic,
        title: txt("Ветвление", "Branch"),
        description: txt("Если условие да — по одной ветке, иначе по другой.", "If the condition is yes, one way, else the other."),
        help: txt(
            "Пример: «Сравнение» баланс ≥ платежа → «да»: перевод на кредит; «нет»: уведомление «не хватает».",
            "Example: «Compare» balance ≥ payment → «yes»: transfer to the loan; «no»: a «not enough» notice.",
        ),
        inputs: vec![flow_in(), port("condition", Ty::Bool, txt("Условие", "Condition"))],
        outputs: vec![
            port("true", Ty::Flow, txt("Да", "Yes")),
            port("false", Ty::Flow, txt("Нет", "No")),
        ],
        params: vec![],
        advanced: false,
        keywords: vec!["если", "иначе", "условие", "ветка", "if", "else", "branch"],
    });
    out.push(NodeDef {
        kind: "logic.guard",
        group: Logic,
        title: txt("Защита", "Guard"),
        description: txt(
            "Продолжает, только если условие да; иначе останавливает с причиной.",
            "Goes on only if the condition is yes; otherwise stops with a reason.",
        ),
        help: txt(
            "Причина попадёт в журнал — видно, почему схема ничего не сделала.",
            "The reason goes to the journal, so you see why the workflow did nothing.",
        ),
        inputs: vec![
            flow_in(),
            port("condition", Ty::Bool, txt("Условие", "Condition")),
        ],
        outputs: vec![flow_out()],
        params: vec![param(
            "reason",
            ParamKind::Text,
            txt("Причина остановки", "Reason to stop"),
            json!(""),
        )],
        advanced: false,
        keywords: vec!["защита", "проверка", "только если", "guard", "check"],
    });
    out.push(NodeDef {
        kind: "logic.math",
        group: Logic,
        title: txt("Математика", "Math"),
        description: txt("Сложить, вычесть, умножить, разделить, процент, мин, макс.", "Add, subtract, multiply, divide, percent, min, max."),
        help: txt(
            "«% от»: B процентов от A — 10 % от зарплаты. Валюта суммы сохраняется; две суммы в разных валютах складывать нельзя.",
            "«% of»: B percent of A — 10% of the salary. An amount keeps its currency; two amounts in different currencies cannot be added.",
        ),
        inputs: vec![
            port("a", Ty::Any, txt("A", "A")),
            port("b", Ty::Any, txt("B", "B")),
        ],
        outputs: vec![port("result", Ty::Any, txt("Результат", "Result"))],
        params: vec![
            param(
                "op",
                select(&[
                    ("add", txt("A + B", "A + B")),
                    ("sub", txt("A − B", "A − B")),
                    ("mul", txt("A × B", "A × B")),
                    ("div", txt("A ÷ B", "A ÷ B")),
                    ("percent", txt("B % от A", "B% of A")),
                    ("min", txt("меньшее", "smaller")),
                    ("max", txt("большее", "larger")),
                ]),
                txt("Действие", "Operation"),
                json!("percent"),
            ),
            param("b", ParamKind::Number, txt("B, если не подключено", "B when not wired"), json!("10")),
            param("round", ParamKind::Number, txt("Знаков после запятой", "Decimal places"), json!("2")),
        ],
        advanced: false,
        keywords: vec!["процент", "сумма", "плюс", "минус", "умножить", "разделить", "math", "percent", "%", "+", "-"],
    });
    let mut inputs = vec![port("amount", Ty::Money, txt("Сумма", "Amount"))];
    let params = vec![wired(
        "currency",
        ParamKind::Currency,
        Ty::Currency,
        txt("В валюту", "Into"),
        json!("rub"),
        &mut inputs,
    )];
    out.push(NodeDef {
        kind: "logic.convert",
        group: Logic,
        title: txt("Конвертировать", "Convert"),
        description: txt(
            "Сумма в другой валюте по текущему курсу.",
            "An amount in another currency at the current rate.",
        ),
        help: txt(
            "Нужна, чтобы сравнить или сложить суммы в разных валютах.",
            "Needed to compare or add amounts in different currencies.",
        ),
        inputs,
        outputs: vec![port("result", Ty::Money, txt("Сумма", "Amount"))],
        params,
        advanced: false,
        keywords: vec![
            "конвертировать",
            "валюта",
            "перевести в",
            "convert",
            "currency",
        ],
    });
    out.push(NodeDef {
        kind: "logic.text",
        group: Logic,
        title: txt("Собрать текст", "Build text"),
        description: txt(
            "Текст по шаблону: {a}, {b}, {c} заменяются входами.",
            "Text from a template: {a}, {b}, {c} are replaced by the inputs.",
        ),
        help: txt(
            "Пример: «Пополнение {a} с карты {b}» — для комментария операции или уведомления.",
            "Example: «Top-up of {a} from card {b}» — for an operation's comment or a notice.",
        ),
        inputs: vec![
            port("a", Ty::Any, txt("{a}", "{a}")),
            port("b", Ty::Any, txt("{b}", "{b}")),
            port("c", Ty::Any, txt("{c}", "{c}")),
        ],
        outputs: vec![port("text", Ty::Text, txt("Текст", "Text"))],
        params: vec![param(
            "template",
            ParamKind::LongText,
            txt("Шаблон", "Template"),
            json!("{a}"),
        )],
        advanced: false,
        keywords: vec![
            "текст",
            "шаблон",
            "сообщение",
            "text",
            "template",
            "message",
        ],
    });
    out.push(NodeDef {
        kind: "logic.throttle",
        group: Logic,
        title: txt("Не чаще раза в", "At most once in"),
        description: txt(
            "Пропускает дальше, только если с прошлого раза прошло N часов.",
            "Lets through only if N hours passed since last time.",
        ),
        help: txt(
            "Против повторов: уведомление «мало денег» не чаще раза в сутки.",
            "Against repeats: a «money is low» notice at most once a day.",
        ),
        inputs: vec![flow_in()],
        outputs: vec![
            flow_out(),
            port("skipped", Ty::Flow, txt("Пропущено", "Skipped")),
        ],
        params: vec![param(
            "hours",
            ParamKind::Number,
            txt("Часов", "Hours"),
            json!("24"),
        )],
        advanced: false,
        keywords: vec!["раз в", "частота", "повтор", "throttle", "once", "debounce"],
    });

    // -- variables ----------------------------------------------------------------
    let scope = || {
        param(
            "scope",
            select(&[
                ("workflow", txt("этой схемы", "this workflow")),
                ("global", txt("общая", "shared")),
            ]),
            txt("Где", "Where"),
            json!("workflow"),
        )
    };
    out.push(NodeDef {
        kind: "var.get",
        group: Variable,
        title: txt("Получить переменную", "Get a variable"),
        description: txt("Значение, сохранённое раньше этой или любой схемой.", "A value saved before by this or any workflow."),
        help: txt(
            "Общие переменные видны всем схемам: «Доля накоплений = 10 %» меняется в одном месте. Их список — во вкладке «Переменные».",
            "Shared variables are seen by every workflow: «Savings share = 10%» changes in one place. They are listed under «Variables».",
        ),
        inputs: vec![],
        outputs: vec![port("value", Ty::Any, txt("Значение", "Value"))],
        params: vec![param("name", ParamKind::Text, txt("Имя", "Name"), json!("")), scope()],
        advanced: false,
        keywords: vec!["переменная", "получить", "variable", "get"],
    });
    out.push(NodeDef {
        kind: "var.set",
        group: Variable,
        title: txt("Записать переменную", "Set a variable"),
        description: txt(
            "Сохраняет значение между запусками.",
            "Keeps a value between runs.",
        ),
        help: txt(
            "Например, дату последнего пополнения. «Дальше» подключать не обязательно, а если не подключить и «Вход» — узел выполнится сам в конце запуска. Узлы, читающие переменную после записи, видят новое значение.",
            "For instance, the date of the last top-up. «Next» is optional, and with no wire into «In» the node runs by itself at the end of the run. Nodes reading the variable after the write see the new value.",
        ),
        inputs: vec![flow_in(), port("value", Ty::Any, txt("Значение", "Value"))],
        outputs: vec![flow_out()],
        params: vec![
            param("name", ParamKind::Text, txt("Имя", "Name"), json!("")),
            ParamDef {
                wirable: true,
                ..param(
                    "value",
                    ParamKind::Text,
                    txt("Значение, если не подключено", "Value when not wired"),
                    json!(""),
                )
            },
            scope(),
        ],
        advanced: false,
        keywords: vec![
            "переменная",
            "записать",
            "сохранить",
            "variable",
            "set",
            "save",
        ],
    });
    out.push(NodeDef {
        kind: "var.add",
        group: Variable,
        title: txt("Увеличить переменную", "Increase a variable"),
        description: txt(
            "Прибавляет к числу в переменной.",
            "Adds to the number in a variable.",
        ),
        help: txt(
            "Счётчик: сколько раз купил BTC. Без связи во «Вход» выполнится сам в конце запуска; «Дальше» не обязателен.",
            "A counter: how many times BTC was bought. With no wire into «In» it runs by itself at the end of the run; «Next» is optional.",
        ),
        inputs: vec![
            flow_in(),
            port("delta", Ty::Number, txt("На сколько", "By")),
        ],
        outputs: vec![flow_out(), port("value", Ty::Number, txt("Стало", "Now"))],
        params: vec![
            param("name", ParamKind::Text, txt("Имя", "Name"), json!("")),
            param(
                "delta",
                ParamKind::Number,
                txt("На сколько, если не подключено", "By, when not wired"),
                json!("1"),
            ),
            scope(),
        ],
        advanced: false,
        keywords: vec!["счётчик", "увеличить", "прибавить", "counter", "increment"],
    });

    // -- actions ------------------------------------------------------------------
    let mut inputs = vec![flow_in()];
    let params = vec![
        param(
            "tx_kind",
            select(&[
                ("topup", txt("пополнение", "top-up")),
                ("payment", txt("оплата", "payment")),
                ("transfer", txt("перевод", "transfer")),
                ("convert", txt("обмен", "exchange")),
            ]),
            txt("Вид", "Kind"),
            json!("transfer"),
        ),
        wired(
            "account",
            ParamKind::Account,
            Ty::Account,
            txt(
                "Счёт (откуда / куда пополнить)",
                "Account (from / to top up)",
            ),
            json!(null),
            &mut inputs,
        ),
        wired(
            "to",
            ParamKind::Account,
            Ty::Account,
            txt("Куда / кому", "To / whom"),
            json!(null),
            &mut inputs,
        ),
        wired(
            "asset",
            ParamKind::Asset,
            Ty::Asset,
            txt("Актив", "Asset"),
            json!(null),
            &mut inputs,
        ),
        wired(
            "amount",
            ParamKind::Number,
            Ty::Number,
            txt("Сумма", "Amount"),
            json!("0"),
            &mut inputs,
        ),
        wired(
            "to_asset",
            ParamKind::Asset,
            Ty::Asset,
            txt("Обмен: получаю", "Exchange: receive"),
            json!(null),
            &mut inputs,
        ),
        wired(
            "to_amount",
            ParamKind::Number,
            Ty::Number,
            txt("Обмен: сколько получаю", "Exchange: how much"),
            json!("0"),
            &mut inputs,
        ),
        wired(
            "category",
            ParamKind::Category,
            Ty::Category,
            txt("Категория", "Category"),
            json!(null),
            &mut inputs,
        ),
        wired(
            "comment",
            ParamKind::Text,
            Ty::Text,
            txt("Комментарий", "Comment"),
            json!(""),
            &mut inputs,
        ),
        wired(
            "date",
            ParamKind::Date,
            Ty::Date,
            txt("Дата операции", "Date of the operation"),
            json!(null),
            &mut inputs,
        ),
        param(
            "estimated",
            ParamKind::Bool,
            txt(
                "Сумма примерная — уточню потом",
                "The amount is approximate, I will correct it",
            ),
            json!(false),
        ),
        param(
            "template",
            ParamKind::LongText,
            txt("Шаблон операции (JSON)", "Operation template (JSON)"),
            json!(""),
        ),
        param(
            "on_shortfall",
            select(&[
                ("fail", txt("ошибка", "an error")),
                (
                    "reject",
                    txt(
                        "отказ с «Повторить» и «Пропустить»",
                        "a refusal to retry or skip",
                    ),
                ),
            ]),
            txt("Если не хватает денег", "If the money is short"),
            json!("fail"),
        ),
    ];
    out.push(NodeDef {
        kind: "action.transaction",
        group: Action,
        title: txt("Создать операцию", "Create an operation"),
        description: txt("Пополнение, оплата, перевод или обмен.", "A top-up, payment, transfer or exchange."),
        help: txt(
            "Проходит те же проверки, что операция из формы. Если денег не хватит или правило нарушено — выход «Ошибка» (или запуск отклоняется). «Если не хватает денег: отказ» — запуск ждёт в журнале, пока вы не нажмёте «Повторить» или «Пропустить». «Дата операции» — подключите «Когда» расписания, чтобы операция встала на день по плану. «Сумма примерная» — операция помечается «уточните сумму». Чтобы спросить вас перед операцией, поставьте перед ней «Ждать подтверждения».",
            "Goes through the same checks as an operation from the form. Not enough money or a broken rule takes the «Error» way (or rejects the run). «If the money is short: a refusal» keeps the run in the journal until you press «Retry» or «Skip». «Date of the operation»: wire the schedule's «When» so the operation lands on the planned day. «The amount is approximate» marks it «correct the amount». To be asked first, put a «Wait for confirmation» node before it.",
        ),
        inputs,
        outputs: vec![
            flow_out(),
            error_out(),
            port("transaction", Ty::Transaction, txt("Операция", "Operation")),
            port("reason", Ty::Text, txt("Причина ошибки", "Error reason")),
        ],
        params,
        advanced: false,
        keywords: vec!["операция", "перевод", "оплата", "пополнить", "купить", "transaction", "transfer", "pay", "buy"],
    });
    let mut inputs = vec![flow_in(), port("amount", Ty::Money, txt("Сумма", "Amount"))];
    let params = vec![
        wired(
            "from",
            ParamKind::Account,
            Ty::Account,
            txt("Откуда", "From"),
            json!(null),
            &mut inputs,
        ),
        param(
            "targets",
            ParamKind::Targets,
            txt("Куда и сколько", "Where and how much"),
            json!([]),
        ),
        param(
            "mode",
            select(&[
                ("percent", txt("доли в %", "shares in %")),
                ("fixed", txt("фиксированные суммы", "fixed amounts")),
            ]),
            txt("Как делить", "How to split"),
            json!("percent"),
        ),
        param(
            "category",
            ParamKind::Category,
            txt("Категория", "Category"),
            json!(null),
        ),
        param(
            "comment",
            ParamKind::Text,
            txt("Комментарий", "Comment"),
            json!(""),
        ),
    ];
    out.push(NodeDef {
        kind: "action.distribute",
        group: Action,
        title: txt("Распределить", "Distribute"),
        description: txt(
            "Делит сумму между счетами по долям или суммам — переводами одной цепочкой.",
            "Splits an amount between accounts by shares or sums: transfers in one chain.",
        ),
        help: txt(
            "Пример: зарплата → 10 % «Накопительный», 5 % «Отпуск». Остаток от округления уходит последнему. Все переводы сохраняются вместе или ни один.",
            "Example: salary → 10% «Savings», 5% «Holiday». The rounding rest goes to the last one. All transfers are saved together or none.",
        ),
        inputs,
        outputs: vec![flow_out(), error_out(), port("created", Ty::List, txt("Операции", "Operations"))],
        params,
        advanced: false,
        keywords: vec!["распределить", "разделить", "доли", "процент", "distribute", "split", "share"],
    });
    let mut inputs = vec![flow_in()];
    let params = vec![
        wired(
            "text",
            ParamKind::LongText,
            Ty::Text,
            txt("Текст", "Text"),
            json!(""),
            &mut inputs,
        ),
        param(
            "title",
            ParamKind::Text,
            txt("Заголовок", "Title"),
            json!(""),
        ),
    ];
    out.push(NodeDef {
        kind: "action.notify",
        group: Action,
        title: txt("Уведомление", "Notification"),
        description: txt(
            "Показывает уведомление — в колокольчике и в системе.",
            "Shows a notice, in the bell and from the system.",
        ),
        help: txt(
            "Текст можно собрать из данных узлом «Собрать текст». Без связи во «Вход» выполнится сам в конце запуска; «Дальше» не обязателен.",
            "Build the text from data with «Build text». With no wire into «In» it runs by itself at the end of the run; «Next» is optional.",
        ),
        inputs,
        outputs: vec![flow_out()],
        params,
        advanced: false,
        keywords: vec![
            "уведомление",
            "сообщить",
            "notify",
            "notification",
            "message",
        ],
    });
    let mut inputs = vec![flow_in()];
    let params = vec![
        wired(
            "text",
            ParamKind::LongText,
            Ty::Text,
            txt("Текст", "Text"),
            json!(""),
            &mut inputs,
        ),
        wired(
            "date",
            ParamKind::Date,
            Ty::Date,
            txt(
                "Дата (вместо «через сколько дней»)",
                "Date (instead of «in how many days»)",
            ),
            json!(""),
            &mut inputs,
        ),
        param(
            "days",
            ParamKind::Number,
            txt("Через сколько дней", "In how many days"),
            json!("0"),
        ),
        param(
            "time",
            ParamKind::Time,
            txt("Время", "Time"),
            json!("10:00"),
        ),
    ];
    out.push(NodeDef {
        kind: "action.reminder",
        group: Action,
        title: txt("Создать напоминание", "Create a reminder"),
        description: txt(
            "Напоминание на дату и время.",
            "A reminder for a date and time.",
        ),
        help: txt(
            "Оно появится в «Напоминаниях» и придёт уведомлением. День — из входа «Дата» (например, после «Сдвинуть на рабочий день»), иначе сегодня плюс «через сколько дней»; время — из поля. Узлу не нужен «Дальше»: можно закончить им ветку.",
            "It shows in «Reminders» and arrives as a notice. The day comes from the «Date» input (say, after «Shift to a working day»), else today plus «in how many days»; the time from its field. It needs no «Next»: a branch may end with it.",
        ),
        inputs,
        outputs: vec![flow_out()],
        params,
        advanced: false,
        keywords: vec!["напоминание", "напомнить", "reminder", "remind"],
    });
    let mut inputs = vec![flow_in()];
    let params = vec![
        wired(
            "question",
            ParamKind::Text,
            Ty::Text,
            txt("Что спросить", "What to ask"),
            json!(""),
            &mut inputs,
        ),
        param(
            "timeout_hours",
            ParamKind::Number,
            txt(
                "Ждать не дольше, часов (0 — без срока)",
                "Wait at most, hours (0 = no limit)",
            ),
            json!(0),
        ),
    ];
    out.push(NodeDef {
        kind: "action.confirm",
        group: Action,
        title: txt("Ждать подтверждения", "Wait for confirmation"),
        description: txt(
            "Останавливает запуск и спрашивает вас; по «Да» идёт одной веткой, по «Нет» — другой.",
            "Pauses the run and asks you; «Yes» goes one way, «No» the other.",
        ),
        help: txt(
            "Запуск засыпает, а вопрос появляется во вкладке «Очередь» и в уведомлении. Ответ «Да» или «Нет» продолжает запуск по своей ветке. Если задан срок и вы не ответили — считается «Нет». В эмуляции вопрос задаётся сразу в окне.",
            "The run sleeps and the question shows under «Queue» and in a notice. Answering «Yes» or «No» goes on along that way. With a time limit and no answer it counts as «No». In emulation the question is asked at once in a dialog.",
        ),
        inputs,
        outputs: vec![
            port("true", Ty::Flow, txt("Да", "Yes")),
            port("false", Ty::Flow, txt("Нет", "No")),
        ],
        params,
        advanced: false,
        keywords: vec!["подтверждение", "проверить", "очередь", "спросить", "жди меня", "confirm", "approve", "ask", "queue"],
    });
    out.push(NodeDef {
        kind: "action.stop",
        group: Action,
        title: txt("Стоп", "Stop"),
        description: txt(
            "Останавливает запуск с причиной.",
            "Stops the run with a reason.",
        ),
        help: txt(
            "Причина видна в журнале.",
            "The reason shows in the journal.",
        ),
        inputs: vec![flow_in()],
        outputs: vec![],
        params: vec![param(
            "reason",
            ParamKind::Text,
            txt("Причина", "Reason"),
            json!(""),
        )],
        advanced: false,
        keywords: vec!["стоп", "остановить", "конец", "stop", "end"],
    });
    let mut inputs = vec![flow_in()];
    let params = vec![
        param(
            "name",
            ParamKind::Text,
            txt("Имя события", "Event name"),
            json!(""),
        ),
        wired(
            "payload",
            ParamKind::Text,
            Ty::Any,
            txt("Данные", "Data"),
            json!(""),
            &mut inputs,
        ),
    ];
    out.push(NodeDef {
        kind: "action.emit",
        group: Action,
        title: txt("Отправить событие", "Send an event"),
        description: txt(
            "Запускает схемы с триггером «Событие схемы» с тем же именем.",
            "Runs workflows with a «Workflow event» trigger of the same name.",
        ),
        help: txt(
            "Цепочка событий не глубже 5: схема не может бесконечно запускать сама себя. Без связи во «Вход» выполнится сам в конце запуска; «Дальше» не обязателен.",
            "An event chain goes no deeper than 5: a workflow cannot run itself forever. With no wire into «In» it runs by itself at the end of the run; «Next» is optional.",
        ),
        inputs,
        outputs: vec![flow_out()],
        params,
        advanced: true,
        keywords: vec![
            "событие",
            "сигнал",
            "запустить схему",
            "event",
            "emit",
            "signal",
        ],
    });
    let mut inputs = vec![flow_in()];
    let params = vec![wired(
        "hours",
        ParamKind::Number,
        Ty::Number,
        txt("Часов", "Hours"),
        json!("24"),
        &mut inputs,
    )];
    out.push(NodeDef {
        kind: "action.wait",
        group: Action,
        title: txt("Подождать", "Wait"),
        description: txt("Продолжает запуск через N часов.", "Continues the run after N hours."),
        help: txt(
            "Запуск засыпает и сохраняется в vault; если vault заблокирован, продолжится при разблокировке. Данные, вычисленные до узла, сохраняются.",
            "The run sleeps and is kept in the vault; if the vault is locked it goes on at unlock. Data worked out before the node is kept.",
        ),
        inputs,
        outputs: vec![flow_out()],
        params,
        advanced: true,
        keywords: vec!["подождать", "пауза", "через", "задержка", "wait", "delay", "later"],
    });

    let mut inputs = vec![flow_in()];
    let params = vec![
        param(
            "workflow",
            ParamKind::Workflow,
            txt("Какую схему", "Which workflow"),
            json!(null),
        ),
        wired(
            "payload",
            ParamKind::Text,
            Ty::Any,
            txt("Данные", "Data"),
            json!(""),
            &mut inputs,
        ),
    ];
    out.push(NodeDef {
        kind: "action.call",
        group: Action,
        title: txt("Вызвать схему", "Call a workflow"),
        description: txt(
            "Выполняет схему-помощник с триггером «Вызов из другой схемы» и берёт её ответ.",
            "Runs a helper workflow with a «Called by a workflow» trigger and takes its answer.",
        ),
        help: txt(
            "Помощник выполняется внутри этого же запуска: его операции, лимиты и откат при ошибке общие. Ошибка помощника идёт по ветке «Ошибка». Вложенность — не глубже 5.",
            "The helper runs inside this same run: its operations, limits and rollback on error are shared. A helper's error takes the «Error» way. Nesting goes no deeper than 5.",
        ),
        inputs,
        outputs: vec![flow_out(), error_out(), port("result", Ty::Any, txt("Ответ", "Answer"))],
        params,
        advanced: true,
        keywords: vec!["вызвать", "подсхема", "функция", "call", "subflow", "function", "reuse"],
    });
    out.push(NodeDef {
        kind: "action.return",
        group: Action,
        title: txt("Вернуть значение", "Return a value"),
        description: txt(
            "Завершает схему-помощник и отдаёт значение тому, кто её вызвал.",
            "Ends a helper workflow and hands a value to its caller.",
        ),
        help: txt(
            "В обычной схеме просто завершает запуск. Без подключённого значения отдаёт пустое.",
            "In an ordinary workflow it just ends the run. With nothing wired it returns nothing.",
        ),
        inputs: vec![flow_in(), port("value", Ty::Any, txt("Значение", "Value"))],
        outputs: vec![],
        params: vec![],
        advanced: true,
        keywords: vec![
            "вернуть",
            "ответ",
            "результат",
            "return",
            "result",
            "answer",
        ],
    });

    // -- lists --------------------------------------------------------------------
    out.push(NodeDef {
        kind: "list.accounts",
        group: List,
        title: txt("Список счетов", "List of accounts"),
        description: txt(
            "Счета кошелька (или все кошельки).",
            "A wallet's accounts (or every wallet).",
        ),
        help: txt(
            "Подключите к «Для каждого», чтобы пройти по счетам.",
            "Wire into «For each» to go through the accounts.",
        ),
        inputs: vec![],
        outputs: vec![port("items", Ty::List, txt("Счета", "Accounts"))],
        params: vec![param(
            "store",
            ParamKind::Store,
            txt("Кошелёк (пусто — все)", "Wallet (empty: all)"),
            json!(null),
        )],
        advanced: true,
        keywords: vec!["счета", "список", "accounts", "list"],
    });
    out.push(NodeDef {
        kind: "list.goals",
        group: List,
        title: txt("Список целей", "List of goals"),
        description: txt(
            "Активные цели: имя, накоплено, осталось.",
            "Active goals: name, saved, left.",
        ),
        help: txt(
            "Каждая — запись с полями name, saved, left, pct.",
            "Each is a record with fields name, saved, left, pct.",
        ),
        inputs: vec![],
        outputs: vec![port("items", Ty::List, txt("Цели", "Goals"))],
        params: vec![],
        advanced: true,
        keywords: vec!["цели", "список", "goals", "list"],
    });
    out.push(NodeDef {
        kind: "list.for_each",
        group: List,
        title: txt("Для каждого", "For each"),
        description: txt(
            "Выполняет ветку «Каждый» для каждого элемента списка, потом — «Готово».",
            "Runs the «Each» branch for every item, then «Done».",
        ),
        help: txt(
            "Не больше 200 элементов за запуск.",
            "At most 200 items per run.",
        ),
        inputs: vec![flow_in(), port("items", Ty::List, txt("Список", "List"))],
        outputs: vec![
            port("each", Ty::Flow, txt("Каждый", "Each")),
            port("done", Ty::Flow, txt("Готово", "Done")),
            port("item", Ty::Any, txt("Элемент", "Item")),
            port("index", Ty::Number, txt("Номер", "Index")),
        ],
        params: vec![],
        advanced: true,
        keywords: vec![
            "для каждого",
            "цикл",
            "перебрать",
            "for each",
            "loop",
            "foreach",
        ],
    });
    out.push(NodeDef {
        kind: "flow.queue",
        group: Logic,
        title: txt("Очередь", "Queue"),
        description: txt(
            "Собирает все срабатывания триггера за одну проверку и передаёт их дальше по одному.",
            "Gathers every firing of the trigger in one check and hands them on one by one.",
        ),
        help: txt(
            "Пришло пять операций сразу — вместо пяти запусков будет один: ветка «Каждое» пройдёт пять раз, у каждого раза свои данные триггера (и «Элемент» — все они записью), потом — «Готово». «По очереди»: сбой одного отменяет весь запуск. «Независимо»: каждое срабатывание — своя ветка, сбой отменяет только её. При ручном запуске и в эмуляции — пример из последних срабатываний. Не больше 200 за запуск.",
            "Five operations came at once — one run instead of five: the «Each» way goes five times, each with its own trigger data (and «Item» — all of it as a record), then «Done». «In turn»: one failing undoes the whole run. «Independently»: each firing is a branch of its own, a failure undoes only it. Run by hand and in emulation it takes an example from the latest firings. At most 200 per run.",
        ),
        inputs: vec![flow_in()],
        outputs: vec![
            port("each", Ty::Flow, txt("Каждое", "Each")),
            port("done", Ty::Flow, txt("Готово", "Done")),
            port("item", Ty::Any, txt("Элемент", "Item")),
            port("index", Ty::Number, txt("Номер", "Index")),
            port("count", Ty::Number, txt("Сколько всего", "How many")),
        ],
        params: vec![param(
            "mode",
            select(&[
                ("sequence", txt("По очереди", "In turn")),
                ("independent", txt("Независимо", "Independently")),
            ]),
            txt("Как передавать", "How to hand on"),
            json!("sequence"),
        )],
        advanced: false,
        keywords: vec!["очередь", "пакет", "все сразу", "накопить", "собрать", "queue", "batch", "collect"],
    });
    out.push(NodeDef {
        kind: "list.count",
        group: List,
        title: txt("Количество", "Count"),
        description: txt("Сколько элементов в списке.", "How many items a list has."),
        help: txt("Пустой список — 0.", "An empty list is 0."),
        inputs: vec![port("items", Ty::List, txt("Список", "List"))],
        outputs: vec![port("count", Ty::Number, txt("Количество", "Count"))],
        params: vec![],
        advanced: true,
        keywords: vec!["количество", "сколько", "count", "length"],
    });
    out.push(NodeDef {
        kind: "list.field",
        group: List,
        title: txt("Поле записи", "Record field"),
        description: txt(
            "Берёт поле из записи (элемента списка).",
            "Takes a field of a record (a list item).",
        ),
        help: txt(
            "Имена полей видны в справке узла, который отдал запись.",
            "Field names are in the help of the node that gave the record.",
        ),
        inputs: vec![port("record", Ty::Any, txt("Запись", "Record"))],
        outputs: vec![port("value", Ty::Any, txt("Значение", "Value"))],
        params: vec![param(
            "field",
            ParamKind::Text,
            txt("Поле", "Field"),
            json!("name"),
        )],
        advanced: true,
        keywords: vec!["поле", "запись", "field", "record", "property"],
    });

    super::catalog_more::more_nodes(&mut out);
    super::catalog_capital::capital_nodes(&mut out);
    with_error_ways(&mut out);
    out
}

/// Nodes that end the run themselves: nothing to catch after them.
const NO_ERROR_WAY: [&str; 2] = ["action.stop", "action.return"];

/// Every action and variable node can fail; each gets an «Ошибка» way and the
/// reason, so a failure can be caught there instead of undoing the run.
fn with_error_ways(defs: &mut [NodeDef]) {
    for def in defs.iter_mut().filter(|def| {
        matches!(def.group, NodeGroup::Action | NodeGroup::Variable)
            && def.input("in").is_some()
            && !NO_ERROR_WAY.contains(&def.kind)
    }) {
        if def.output("error").is_none() {
            def.outputs.push(error_out());
        }
        if def.output("reason").is_none() {
            def.outputs.push(port(
                "reason",
                PortType::Text,
                txt("Причина ошибки", "Error reason"),
            ));
        }
    }
}

/// The catalog as JSON for the browser mock, under `src/shared/api/` (the mock
/// lives in `mock/`, loaded only by `pnpm dev`);
/// `mise run gen-types` rewrites it.
pub const MOCK_CATALOG_FILE: &str = "mock/workflow-catalog.json";

static CATALOG: OnceLock<Vec<NodeDef>> = OnceLock::new();

/// Every node there is.
pub fn catalog() -> &'static [NodeDef] {
    CATALOG.get_or_init(|| {
        let mut nodes = build();
        mark_required(&mut nodes);
        nodes
    })
}

pub fn node_def(kind: &str) -> Option<&'static NodeDef> {
    catalog().iter().find(|def| def.kind == kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_required_param_has_an_empty_default() {
        for (kind, name) in REQUIRED_PARAMS {
            let def = node_def(kind).unwrap_or_else(|| panic!("{kind} is not in the catalog"));
            let param = def
                .param(name)
                .unwrap_or_else(|| panic!("{kind} has no field {name}"));
            assert!(param.required, "{kind}.{name} is not marked");
            // A filled default would hide the empty field from the check.
            assert!(
                param.default.is_null() || param.default == serde_json::json!([]),
                "{kind}.{name} defaults to {}",
                param.default
            );
        }
    }

    #[test]
    fn the_browser_mock_has_the_current_catalog() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../src/shared/api")
            .join(MOCK_CATALOG_FILE);
        let stored: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_default())
                .unwrap_or_default();
        let current = serde_json::to_value(catalog()).unwrap();
        assert!(
            stored == current,
            "{} is stale: run `mise run gen-types`",
            path.display()
        );
    }

    #[test]
    fn every_action_that_can_fail_has_an_error_way() {
        for def in catalog().iter().filter(|def| {
            matches!(def.group, NodeGroup::Action | NodeGroup::Variable)
                && def.input("in").is_some()
                && !NO_ERROR_WAY.contains(&def.kind)
        }) {
            assert!(
                def.output("error").is_some(),
                "{} has no error way",
                def.kind
            );
            assert!(
                def.output("reason").is_some(),
                "{} has no error reason",
                def.kind
            );
        }
    }

    #[test]
    fn every_node_is_described_once_and_its_ports_are_unique() {
        let mut kinds = HashSet::new();
        for def in catalog() {
            assert!(kinds.insert(def.kind), "{} twice", def.kind);
            assert!(
                !def.title.ru.is_empty() && !def.title.en.is_empty(),
                "{}",
                def.kind
            );
            assert!(!def.help.ru.is_empty(), "{} has no help", def.kind);
            let mut ports = HashSet::new();
            for port in def.inputs.iter() {
                assert!(
                    ports.insert(format!("in:{}", port.name)),
                    "{} input {}",
                    def.kind,
                    port.name
                );
            }
            for port in def.outputs.iter() {
                assert!(
                    ports.insert(format!("out:{}", port.name)),
                    "{} output {}",
                    def.kind,
                    port.name
                );
            }
            for param in &def.params {
                if param.wirable {
                    assert!(
                        def.input(param.name).is_some(),
                        "{} wirable {} has no input",
                        def.kind,
                        param.name
                    );
                }
            }
            if def.is_trigger() {
                assert!(
                    def.output("out").is_some(),
                    "{} trigger without out",
                    def.kind
                );
            }
        }
    }
}
