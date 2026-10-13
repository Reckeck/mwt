//! The second half of the node catalog: dates, text and conversion, choosing
//! and rounding, building lists and records, and loops.

use serde_json::json;

use super::catalog::{
    NodeDef, NodeGroup, ParamKind, Text, flow_in, param, port, select, txt, wired,
};
use super::types::PortType as Ty;

const DATE_UNITS: [(&str, Text); 6] = [
    ("minutes", txt("минут", "minutes")),
    ("hours", txt("часов", "hours")),
    ("days", txt("дней", "days")),
    ("weeks", txt("недель", "weeks")),
    ("months", txt("месяцев", "months")),
    ("years", txt("лет", "years")),
];

const WEEKEND_DAYS: [(&str, Text); 3] = [
    (
        "sat_sun",
        txt("суббота и воскресенье", "Saturday and Sunday"),
    ),
    ("sun", txt("только воскресенье", "Sunday only")),
    ("fri_sat", txt("пятница и суббота", "Friday and Saturday")),
];

const CALENDARS: [(&str, Text); 2] = [
    (
        "ru",
        txt(
            "Россия: выходные и праздники",
            "Russia: weekends and holidays",
        ),
    ),
    ("weekends", txt("только выходные", "weekends only")),
];

const TYPES: [(&str, Text); 6] = [
    ("number", txt("число", "number")),
    ("text", txt("текст", "text")),
    ("bool", txt("да/нет", "yes/no")),
    ("date", txt("дата", "date")),
    ("money", txt("сумма в валюте", "amount in a currency")),
    ("currency", txt("валюта", "currency")),
];

#[allow(clippy::too_many_lines)]
pub(super) fn more_nodes(out: &mut Vec<NodeDef>) {
    dates(out);
    texts(out);
    choosing(out);
    lists(out);
    loops(out);
}

fn date_in(name: &'static str, label: Text) -> super::catalog::PortDef {
    port(name, Ty::Date, label)
}

#[allow(clippy::too_many_lines)]
fn dates(out: &mut Vec<NodeDef>) {
    use NodeGroup::Date;
    out.push(NodeDef {
        kind: "date.parts",
        group: Date,
        title: txt("Части даты", "Date parts"),
        description: txt(
            "Год, месяц, день, день недели и время даты.",
            "The year, month, day, weekday and time of a date.",
        ),
        help: txt(
            "Без подключённой даты берёт сейчас. День недели: 0 — понедельник … 6 — воскресенье. «День» — текст вида 2026-11-25, удобный для сравнения и сообщений.",
            "Without a wired date it takes now. Weekday: 0 = Monday … 6 = Sunday. «Day» is text like 2026-11-25, handy to compare and to write.",
        ),
        inputs: vec![date_in("date", txt("Дата", "Date"))],
        outputs: vec![
            port("year", Ty::Number, txt("Год", "Year")),
            port("month", Ty::Number, txt("Месяц", "Month")),
            port("day", Ty::Number, txt("Число", "Day")),
            port("weekday", Ty::Number, txt("День недели", "Weekday")),
            port("hour", Ty::Number, txt("Час", "Hour")),
            port("minute", Ty::Number, txt("Минута", "Minute")),
            port("days_in_month", Ty::Number, txt("Дней в месяце", "Days in the month")),
            port("date_text", Ty::Text, txt("День", "Day as text")),
        ],
        params: vec![],
        advanced: false,
        keywords: vec!["год", "месяц", "число", "день недели", "year", "month", "day", "weekday", "parts"],
    });

    let mut inputs = vec![date_in("base", txt("От какой даты", "Based on"))];
    let params = vec![
        wired(
            "year",
            ParamKind::Number,
            Ty::Number,
            txt("Год (пусто — как у даты)", "Year (empty = the date's)"),
            json!(""),
            &mut inputs,
        ),
        wired(
            "month",
            ParamKind::Number,
            Ty::Number,
            txt("Месяц (пусто — как у даты)", "Month (empty = the date's)"),
            json!(""),
            &mut inputs,
        ),
        wired(
            "day",
            ParamKind::Day,
            Ty::Number,
            txt("Число", "Day"),
            json!(1),
            &mut inputs,
        ),
        param(
            "time",
            ParamKind::Time,
            txt("Время", "Time"),
            json!("10:00"),
        ),
        param(
            "month_shift",
            ParamKind::Number,
            txt(
                "Сдвиг месяца (0 — этот, 1 — следующий)",
                "Month shift (0 = this, 1 = next)",
            ),
            json!("0"),
        ),
    ];
    out.push(NodeDef {
        kind: "date.make",
        group: Date,
        title: txt("Собрать дату", "Make a date"),
        description: txt(
            "Дата из года, месяца и числа: «25-е этого месяца».",
            "A date from a year, month and day: «the 25th of this month».",
        ),
        help: txt(
            "Пустые год и месяц берутся из даты на входе (без неё — сейчас). Число больше длины месяца становится последним днём: 31-е в феврале — 28 или 29. Пример: «По расписанию» 1-го → «Собрать дату» (25) → «Сдвинуть на рабочий день».",
            "An empty year and month come from the input date (without one, now). A day past the month's end becomes its last day: the 31st in February is the 28th or 29th. Example: «On a schedule» on the 1st → «Make a date» (25) → «Shift to a working day».",
        ),
        inputs,
        outputs: vec![port("date", Ty::Date, txt("Дата", "Date"))],
        params,
        advanced: false,
        keywords: vec!["дата", "собрать", "число месяца", "зарплата", "аванс", "make date", "build date"],
    });

    let mut inputs = vec![date_in("date", txt("Дата", "Date"))];
    let params = vec![
        wired(
            "amount",
            ParamKind::Number,
            Ty::Number,
            txt("Сколько (минус — назад)", "How many (minus = back)"),
            json!("1"),
            &mut inputs,
        ),
        param(
            "unit",
            select(&DATE_UNITS),
            txt("Чего", "Of what"),
            json!("days"),
        ),
    ];
    inputs.push(port(
        "duration",
        Ty::Duration,
        txt("Длительность", "Duration"),
    ));
    out.push(NodeDef {
        kind: "date.add",
        group: Date,
        title: txt("Сдвинуть дату", "Shift a date"),
        description: txt(
            "Дата плюс или минус минуты, часы, дни, недели, месяцы, годы.",
            "A date plus or minus minutes, hours, days, weeks, months, years.",
        ),
        help: txt(
            "Отрицательное число сдвигает назад. Месяц к 31 января даёт последний день февраля. Если подключена «Длительность» — прибавляется она.",
            "A negative number goes back. A month after January 31 is the last day of February. A wired «Duration» is added instead.",
        ),
        inputs,
        outputs: vec![port("date", Ty::Date, txt("Дата", "Date"))],
        params,
        advanced: false,
        keywords: vec!["прибавить", "вычесть", "через", "назад", "add", "subtract", "shift date"],
    });

    out.push(NodeDef {
        kind: "date.diff",
        group: Date,
        title: txt("Разница дат", "Between dates"),
        description: txt("Сколько дней и часов от A до B.", "How many days and hours from A to B."),
        help: txt(
            "Положительно, если B позже A. Без подключённой B — до сейчас. Дни считаются по календарю вашего пояса.",
            "Positive when B is later than A. Without a wired B, up to now. Days follow the calendar of your zone.",
        ),
        inputs: vec![date_in("a", txt("A", "A")), date_in("b", txt("B", "B"))],
        outputs: vec![
            port("days", Ty::Number, txt("Дней", "Days")),
            port("hours", Ty::Number, txt("Часов", "Hours")),
            port("duration", Ty::Duration, txt("Длительность", "Duration")),
        ],
        params: vec![],
        advanced: false,
        keywords: vec!["разница", "сколько дней", "осталось", "difference", "days between", "until"],
    });

    let inputs = vec![date_in("date", txt("Дата", "Date"))];
    let params = vec![param(
        "which",
        select(&[
            ("day_start", txt("начало дня", "start of the day")),
            (
                "week_start",
                txt("понедельник этой недели", "Monday of the week"),
            ),
            (
                "month_start",
                txt("первое число месяца", "first of the month"),
            ),
            (
                "month_end",
                txt("последний день месяца", "last day of the month"),
            ),
            (
                "next_month_start",
                txt("первое число следующего месяца", "first of next month"),
            ),
            ("year_start", txt("1 января", "January 1")),
            ("year_end", txt("31 декабря", "December 31")),
        ]),
        txt("Что", "What"),
        json!("month_start"),
    )];
    out.push(NodeDef {
        kind: "date.boundary",
        group: Date,
        title: txt("Начало или конец периода", "Start or end of a period"),
        description: txt(
            "Начало дня, недели, месяца, года или конец месяца, года.",
            "The start of a day, week, month, year or the end of a month, year.",
        ),
        help: txt(
            "Без подключённой даты — от сейчас. Удобно для «последний рабочий день месяца»: конец месяца → «Сдвинуть на рабочий день» назад.",
            "Without a wired date, from now. Handy for «the last working day of the month»: the month's end → «Shift to a working day» back.",
        ),
        inputs,
        outputs: vec![port("date", Ty::Date, txt("Дата", "Date"))],
        params,
        advanced: false,
        keywords: vec!["начало месяца", "конец месяца", "первое число", "последний день", "month start", "month end"],
    });

    let inputs = vec![date_in("date", txt("Дата", "Date"))];
    let params = vec![
        param(
            "direction",
            select(&[
                (
                    "back",
                    txt(
                        "назад, к предыдущему рабочему",
                        "back, to the previous working day",
                    ),
                ),
                (
                    "forward",
                    txt(
                        "вперёд, к следующему рабочему",
                        "forward, to the next working day",
                    ),
                ),
            ]),
            txt("Куда сдвигать выходной", "Where a day off moves"),
            json!("back"),
        ),
        param(
            "calendar",
            select(&CALENDARS),
            txt("Календарь", "Calendar"),
            json!("ru"),
        ),
        param(
            "weekend",
            select(&WEEKEND_DAYS),
            txt("Выходные дни недели", "Weekend days"),
            json!("sat_sun"),
        ),
        param(
            "extra_off",
            ParamKind::Text,
            txt("Ещё выходные (даты)", "Extra days off (dates)"),
            json!(""),
        ),
        param(
            "extra_work",
            ParamKind::Text,
            txt("Рабочие дни (переносы)", "Working days (transfers)"),
            json!(""),
        ),
    ];
    out.push(NodeDef {
        kind: "date.shift_workday",
        group: Date,
        title: txt("Сдвинуть на рабочий день", "Shift to a working day"),
        description: txt(
            "Если дата — выходной или праздник, переносит на ближайший рабочий день назад или вперёд.",
            "If the date is a weekend or a holiday, moves it to the nearest working day back or forward.",
        ),
        help: txt(
            "Так работают зарплата и аванс: выпало на субботу — выплатят в пятницу. Рабочий день остаётся как есть; «Сдвинута» говорит, был ли перенос. Праздники и переносы — как у «Рабочий день?».",
            "That is how pay days work: a Saturday one is paid on Friday. A working day stays as it is; «Moved» says whether it moved. Holidays and transfers as in «Working day?».",
        ),
        inputs,
        outputs: vec![
            port("date", Ty::Date, txt("Дата", "Date")),
            port("shifted", Ty::Bool, txt("Сдвинута", "Moved")),
        ],
        params,
        advanced: false,
        keywords: vec!["рабочий день", "выходной", "перенести", "зарплата", "пятница", "workday", "business day", "payday"],
    });

    let mut inputs = vec![date_in("date", txt("Дата", "Date"))];
    let params = vec![wired(
        "pattern",
        ParamKind::Text,
        Ty::Text,
        txt("Шаблон: ДД.ММ.ГГГГ ЧЧ:мм", "Pattern: DD.MM.YYYY HH:mm"),
        json!("ДД.ММ.ГГГГ"),
        &mut inputs,
    )];
    out.push(NodeDef {
        kind: "date.format",
        group: Date,
        title: txt("Дата текстом", "Date as text"),
        description: txt(
            "Пишет дату по шаблону: 25.11.2026, 2026-11-25, 25 ноября.",
            "Writes a date by a pattern: 25.11.2026, 2026-11-25, 25 ноября.",
        ),
        help: txt(
            "В шаблоне: ГГГГ — год, ММ — месяц числом, МММ — месяц словом, ДД — число, ЧЧ — часы, мм — минуты, ДН — день недели. Латиница тоже понимается: YYYY, MM, MMM, DD, HH, mm.",
            "In the pattern: YYYY year, MM month as a number, MMM month as a word, DD day, HH hours, mm minutes, DW weekday. Cyrillic works too: ГГГГ, ММ, МММ, ДД, ЧЧ, ДН.",
        ),
        inputs,
        outputs: vec![port("text", Ty::Text, txt("Текст", "Text"))],
        params,
        advanced: false,
        keywords: vec!["формат", "дата текстом", "format date", "date to text"],
    });
}

#[allow(clippy::too_many_lines)]
fn texts(out: &mut Vec<NodeDef>) {
    use NodeGroup::Text as Group;
    let mut inputs = vec![port("text", Ty::Text, txt("Текст", "Text"))];
    let params = vec![
        wired(
            "pattern",
            ParamKind::Text,
            Ty::Text,
            txt(
                "Что искать (регулярное выражение)",
                "What to find (a regular expression)",
            ),
            json!(r"\d+"),
            &mut inputs,
        ),
        param(
            "group",
            ParamKind::Number,
            txt(
                "Какая скобка (0 — всё совпадение)",
                "Which group (0 = the whole match)",
            ),
            json!("0"),
        ),
        param(
            "ignore_case",
            ParamKind::Bool,
            txt("Без учёта регистра", "Ignore case"),
            json!(true),
        ),
    ];
    out.push(NodeDef {
        kind: "text.extract",
        group: Group,
        title: txt("Найти в тексте", "Find in text"),
        description: txt(
            "Достаёт из строки часть по образцу: номер, сумму, код.",
            "Takes a part of a string by a pattern: a number, an amount, a code.",
        ),
        help: txt(
            "Образец — регулярное выражение. Примеры: «\\d+» — первое число; «карта \\*(\\d{4})» с группой 1 — последние цифры карты; «(\\d+[.,]\\d+) ?₽» — сумма в рублях. «Все» — список всех совпадений. Нашлось ли — на выходе «Найдено».",
            "The pattern is a regular expression. Examples: «\\d+» the first number; «card \\*(\\d{4})» with group 1 the card's last digits; «(\\d+[.,]\\d+) ?₽» an amount in roubles. «All» is the list of every match; «Found» says whether there was one.",
        ),
        inputs,
        outputs: vec![
            port("match", Ty::Text, txt("Найденное", "Match")),
            port("all", Ty::List, txt("Все", "All")),
            port("found", Ty::Bool, txt("Найдено", "Found")),
        ],
        params,
        advanced: false,
        keywords: vec!["найти", "извлечь", "регулярка", "из строки", "regex", "extract", "find", "parse"],
    });

    out.push(NodeDef {
        kind: "text.number",
        group: Group,
        title: txt("Число из текста", "Number from text"),
        description: txt(
            "Первое число в строке: «Списано 1 234,56 ₽» → 1234.56.",
            "The first number in a string: «Charged 1 234,56 ₽» → 1234.56.",
        ),
        help: txt(
            "Понимает пробелы между разрядами, запятую и точку, минус. Если чисел несколько — берёт по номеру (1 — первое).",
            "Understands spaces between digit groups, a comma or a dot, a minus. With several numbers it takes the one by its place (1 = the first).",
        ),
        inputs: vec![port("text", Ty::Text, txt("Текст", "Text"))],
        outputs: vec![
            port("number", Ty::Number, txt("Число", "Number")),
            port("found", Ty::Bool, txt("Найдено", "Found")),
        ],
        params: vec![param("index", ParamKind::Number, txt("Какое по счёту", "Which one"), json!("1"))],
        advanced: false,
        keywords: vec!["число", "сумма из текста", "распознать", "number", "parse number", "amount"],
    });

    let mut inputs = vec![port("text", Ty::Text, txt("Текст", "Text"))];
    let params = vec![wired(
        "separator",
        ParamKind::Text,
        Ty::Text,
        txt("Разделитель", "Separator"),
        json!(","),
        &mut inputs,
    )];
    out.push(NodeDef {
        kind: "text.split",
        group: Group,
        title: txt("Разбить текст", "Split text"),
        description: txt("Список частей строки по разделителю.", "The parts of a string, split by a separator."),
        help: txt(
            "Пробелы по краям частей убираются, пустые части выбрасываются. Дальше — «Для каждого» или «Элемент списка».",
            "Spaces around parts are trimmed, empty parts dropped. Then «For each» or «List item».",
        ),
        inputs,
        outputs: vec![
            port("items", Ty::List, txt("Части", "Parts")),
            port("count", Ty::Number, txt("Сколько", "Count")),
        ],
        params,
        advanced: true,
        keywords: vec!["разбить", "разделить", "split", "separate"],
    });

    let mut inputs = vec![port("text", Ty::Text, txt("Текст", "Text"))];
    let params = vec![
        wired(
            "find",
            ParamKind::Text,
            Ty::Text,
            txt("Что заменить", "What to replace"),
            json!(""),
            &mut inputs,
        ),
        wired(
            "with",
            ParamKind::Text,
            Ty::Text,
            txt("На что", "With what"),
            json!(""),
            &mut inputs,
        ),
        param(
            "regex",
            ParamKind::Bool,
            txt("Это регулярное выражение", "It is a regular expression"),
            json!(false),
        ),
    ];
    out.push(NodeDef {
        kind: "text.replace",
        group: Group,
        title: txt("Заменить в тексте", "Replace in text"),
        description: txt(
            "Заменяет все вхождения одного текста другим.",
            "Replaces every occurrence of one text with another.",
        ),
        help: txt(
            "С регулярным выражением в замене можно ссылаться на скобки: $1.",
            "With a regular expression the replacement may refer to groups: $1.",
        ),
        inputs,
        outputs: vec![port("text", Ty::Text, txt("Текст", "Text"))],
        params,
        advanced: true,
        keywords: vec!["заменить", "replace", "substitute"],
    });

    let params = vec![param(
        "mode",
        select(&[
            ("trim", txt("убрать пробелы по краям", "trim spaces")),
            ("upper", txt("ВСЕ ЗАГЛАВНЫЕ", "UPPER CASE")),
            ("lower", txt("все строчные", "lower case")),
            ("capitalize", txt("С заглавной", "Capitalized")),
        ]),
        txt("Как", "How"),
        json!("trim"),
    )];
    out.push(NodeDef {
        kind: "text.case",
        group: Group,
        title: txt("Регистр и пробелы", "Case and spaces"),
        description: txt(
            "Заглавные, строчные, без лишних пробелов.",
            "Upper, lower case, without stray spaces.",
        ),
        help: txt(
            "Полезно перед сравнением: «Сбер» и «СБЕР » станут одинаковыми.",
            "Handy before comparing: «Sber» and «SBER » become the same.",
        ),
        inputs: vec![port("text", Ty::Text, txt("Текст", "Text"))],
        outputs: vec![
            port("text", Ty::Text, txt("Текст", "Text")),
            port("length", Ty::Number, txt("Длина", "Length")),
        ],
        params,
        advanced: true,
        keywords: vec![
            "регистр",
            "заглавные",
            "строчные",
            "пробелы",
            "длина",
            "upper",
            "lower",
            "trim",
            "length",
        ],
    });

    let mut inputs = vec![port("items", Ty::List, txt("Список", "List"))];
    let params = vec![
        wired(
            "separator",
            ParamKind::Text,
            Ty::Text,
            txt("Разделитель", "Separator"),
            json!(", "),
            &mut inputs,
        ),
        param(
            "field",
            ParamKind::Text,
            txt(
                "Поле записи (пусто — сам элемент)",
                "Record field (empty = the item)",
            ),
            json!(""),
        ),
    ];
    out.push(NodeDef {
        kind: "text.join",
        group: Group,
        title: txt("Склеить список", "Join a list"),
        description: txt(
            "Один текст из элементов списка.",
            "One text from the items of a list.",
        ),
        help: txt(
            "Для списка счетов поле «name» даст «Карта, Наличные, Вклад».",
            "For a list of accounts the field «name» gives «Card, Cash, Deposit».",
        ),
        inputs,
        outputs: vec![port("text", Ty::Text, txt("Текст", "Text"))],
        params,
        advanced: true,
        keywords: vec!["склеить", "объединить", "join", "concatenate"],
    });

    let params = vec![
        param("to", select(&TYPES), txt("Во что", "Into"), json!("number")),
        param(
            "currency",
            ParamKind::Currency,
            txt("Валюта (для суммы)", "Currency (for an amount)"),
            json!(""),
        ),
        param(
            "date_format",
            ParamKind::Text,
            txt(
                "Формат даты в тексте (пусто — сам)",
                "Date format in text (empty = guess)",
            ),
            json!(""),
        ),
    ];
    out.push(NodeDef {
        kind: "convert.to",
        group: Group,
        title: txt("Преобразовать", "Convert type"),
        description: txt(
            "Значение в другой тип: текст в число или дату, число в сумму, что угодно в текст.",
            "A value into another type: text into a number or a date, a number into an amount, anything into text.",
        ),
        help: txt(
            "Если не вышло, на выходе пусто, а «Получилось» — нет: так можно проверить «Ветвлением». Текст в дату понимает 2026-11-25, 25.11.2026 и 25.11.2026 14:30; другой вид задайте форматом (ДД/ММ/ГГГГ). Сумма в одной валюте в другую — это «Конвертировать», не этот узел.",
            "When it fails the output is empty and «Worked» is no, so a «Branch» can check it. Text into a date understands 2026-11-25, 25.11.2026 and 25.11.2026 14:30; set a format for others (DD/MM/YYYY). An amount into another currency is «Convert», not this node.",
        ),
        inputs: vec![port("value", Ty::Any, txt("Значение", "Value"))],
        outputs: vec![
            port("result", Ty::Any, txt("Результат", "Result")),
            port("ok", Ty::Bool, txt("Получилось", "Worked")),
        ],
        params,
        advanced: false,
        keywords: vec!["преобразовать", "тип", "в число", "в текст", "в дату", "конвертер", "convert", "cast", "parse", "to number", "to date"],
    });
}

#[allow(clippy::too_many_lines)]
fn choosing(out: &mut Vec<NodeDef>) {
    use NodeGroup::Logic;
    out.push(NodeDef {
        kind: "logic.select",
        group: Logic,
        title: txt("Выбрать значение", "Pick a value"),
        description: txt("Если условие да — A, иначе B.", "If the condition is yes, A, else B."),
        help: txt(
            "Как «Ветвление», но для данных: не нужно рисовать две ветки ради одного числа. Пример: выходной? → сумма 0 : сумма платежа.",
            "Like «Branch» but for data: no need for two ways for one number. Example: a day off? → 0 : the payment.",
        ),
        inputs: vec![
            port("condition", Ty::Bool, txt("Условие", "Condition")),
            port("a", Ty::Any, txt("A (если да)", "A (if yes)")),
            port("b", Ty::Any, txt("B (если нет)", "B (if no)")),
        ],
        outputs: vec![port("result", Ty::Any, txt("Результат", "Result"))],
        params: vec![],
        advanced: false,
        keywords: vec!["выбрать", "если иначе", "тернарный", "select", "choose", "if else"],
    });

    let mut inputs = vec![port("value", Ty::Any, txt("Значение", "Value"))];
    let params = vec![
        wired(
            "places",
            ParamKind::Number,
            Ty::Number,
            txt("Знаков после запятой", "Decimal places"),
            json!("0"),
            &mut inputs,
        ),
        param(
            "mode",
            select(&[
                ("round", txt("по правилам", "to nearest")),
                ("floor", txt("вниз", "down")),
                ("ceil", txt("вверх", "up")),
            ]),
            txt("Как", "How"),
            json!("round"),
        ),
    ];
    out.push(NodeDef {
        kind: "logic.round",
        group: Logic,
        title: txt("Округлить", "Round"),
        description: txt(
            "Округляет число или сумму.",
            "Rounds a number or an amount.",
        ),
        help: txt(
            "Сумма сохраняет валюту. «Вниз» с 0 знаков — отбросить копейки.",
            "An amount keeps its currency. «Down» with 0 places drops the cents.",
        ),
        inputs,
        outputs: vec![port("result", Ty::Any, txt("Результат", "Result"))],
        params,
        advanced: false,
        keywords: vec!["округлить", "вниз", "вверх", "round", "floor", "ceil"],
    });

    let mut outputs: Vec<_> = (1..=4)
        .map(|index| {
            let (name, ru, en): (&'static str, &'static str, &'static str) = match index {
                1 => ("case1", "Вариант 1", "Case 1"),
                2 => ("case2", "Вариант 2", "Case 2"),
                3 => ("case3", "Вариант 3", "Case 3"),
                _ => ("case4", "Вариант 4", "Case 4"),
            };
            port(name, Ty::Flow, txt(ru, en))
        })
        .collect();
    outputs.push(port("other", Ty::Flow, txt("Иначе", "Otherwise")));
    out.push(NodeDef {
        kind: "logic.switch",
        group: Logic,
        title: txt("Выбор по значению", "Switch"),
        description: txt(
            "Идёт по ветке того варианта, которому равно значение, иначе — «Иначе».",
            "Goes the way of the case the value equals, else «Otherwise».",
        ),
        help: txt(
            "Сравнивает как текст без учёта регистра; для чисел — как числа. Пустой вариант не участвует. Пример: день недели → 0 «пн», 4 «пт», иначе.",
            "Compares as text ignoring case; numbers as numbers. An empty case takes no part. Example: weekday → 0 «Mon», 4 «Fri», otherwise.",
        ),
        inputs: vec![flow_in(), port("value", Ty::Any, txt("Значение", "Value"))],
        outputs,
        params: vec![
            param("case1", ParamKind::Text, txt("Вариант 1", "Case 1"), json!("")),
            param("case2", ParamKind::Text, txt("Вариант 2", "Case 2"), json!("")),
            param("case3", ParamKind::Text, txt("Вариант 3", "Case 3"), json!("")),
            param("case4", ParamKind::Text, txt("Вариант 4", "Case 4"), json!("")),
        ],
        advanced: true,
        keywords: vec!["выбор", "варианты", "switch", "case", "match"],
    });
}

#[allow(clippy::too_many_lines)]
fn lists(out: &mut Vec<NodeDef>) {
    use NodeGroup::List;
    out.push(NodeDef {
        kind: "list.make",
        group: List,
        title: txt("Собрать список", "Make a list"),
        description: txt("Список из подключённых значений.", "A list of the wired values."),
        help: txt(
            "Пустые входы пропускаются. Дальше — «Для каждого», «Сумма списка» или «Склеить список».",
            "Empty inputs are skipped. Then «For each», «Sum of a list» or «Join a list».",
        ),
        inputs: vec![
            port("a", Ty::Any, txt("1", "1")),
            port("b", Ty::Any, txt("2", "2")),
            port("c", Ty::Any, txt("3", "3")),
            port("d", Ty::Any, txt("4", "4")),
        ],
        outputs: vec![port("items", Ty::List, txt("Список", "List"))],
        params: vec![],
        advanced: true,
        keywords: vec!["список", "собрать", "массив", "make list", "array"],
    });

    let mut inputs = vec![port("items", Ty::List, txt("Список", "List"))];
    let params = vec![wired(
        "index",
        ParamKind::Number,
        Ty::Number,
        txt(
            "Номер (1 — первый, −1 — последний)",
            "Place (1 = first, −1 = last)",
        ),
        json!("1"),
        &mut inputs,
    )];
    out.push(NodeDef {
        kind: "list.at",
        group: List,
        title: txt("Элемент списка", "List item"),
        description: txt("Элемент по номеру.", "The item at a place."),
        help: txt(
            "Нет такого — на выходе пусто, «Есть» — нет.",
            "No such item: the output is empty and «Exists» is no.",
        ),
        inputs,
        outputs: vec![
            port("item", Ty::Any, txt("Элемент", "Item")),
            port("exists", Ty::Bool, txt("Есть", "Exists")),
        ],
        params,
        advanced: true,
        keywords: vec![
            "элемент",
            "первый",
            "последний",
            "item",
            "first",
            "last",
            "index",
        ],
    });

    let params = vec![param(
        "field",
        ParamKind::Text,
        txt(
            "Поле записи (пусто — сам элемент)",
            "Record field (empty = the item)",
        ),
        json!(""),
    )];
    out.push(NodeDef {
        kind: "list.sum",
        group: List,
        title: txt("Сумма списка", "Sum of a list"),
        description: txt(
            "Сумма, наименьшее и наибольшее из чисел или сумм списка.",
            "The total, the least and the greatest of a list's numbers or amounts.",
        ),
        help: txt(
            "Суммы в разных валютах не складываются — будет ошибка. Для списка счетов поле «value» даст их общую стоимость.",
            "Amounts in different currencies are not added up: that is an error. For a list of accounts the field «value» gives their total worth.",
        ),
        inputs: vec![port("items", Ty::List, txt("Список", "List"))],
        outputs: vec![
            port("total", Ty::Any, txt("Сумма", "Total")),
            port("min", Ty::Any, txt("Наименьшее", "Least")),
            port("max", Ty::Any, txt("Наибольшее", "Greatest")),
        ],
        params,
        advanced: true,
        keywords: vec!["сумма", "итого", "минимум", "максимум", "sum", "total", "min", "max"],
    });

    let mut inputs = vec![port("items", Ty::List, txt("Список", "List"))];
    let params = vec![
        param(
            "field",
            ParamKind::Text,
            txt(
                "Поле записи (пусто — сам элемент)",
                "Record field (empty = the item)",
            ),
            json!(""),
        ),
        param(
            "op",
            select(&[
                ("gt", txt("больше", "greater than")),
                ("gte", txt("не меньше", "at least")),
                ("lt", txt("меньше", "less than")),
                ("lte", txt("не больше", "at most")),
                ("eq", txt("равно", "equal to")),
                ("ne", txt("не равно", "not equal to")),
                ("contains", txt("содержит", "contains")),
            ]),
            txt("Условие", "Condition"),
            json!("gt"),
        ),
        wired(
            "value",
            ParamKind::Text,
            Ty::Any,
            txt("Значение", "Value"),
            json!("0"),
            &mut inputs,
        ),
    ];
    out.push(NodeDef {
        kind: "list.filter",
        group: List,
        title: txt("Отобрать из списка", "Filter a list"),
        description: txt(
            "Только элементы, подходящие под условие.",
            "Only the items that meet a condition.",
        ),
        help: txt(
            "Пример: счета, у которых «value» больше 0. Сравнение — как у «Сравнения».",
            "Example: accounts whose «value» is above 0. Compares like «Compare».",
        ),
        inputs,
        outputs: vec![
            port("items", Ty::List, txt("Список", "List")),
            port("count", Ty::Number, txt("Сколько", "Count")),
        ],
        params,
        advanced: true,
        keywords: vec!["фильтр", "отобрать", "где", "filter", "where"],
    });

    let mut inputs = Vec::new();
    let params = vec![
        param(
            "key1",
            ParamKind::Text,
            txt("Поле 1", "Field 1"),
            json!("name"),
        ),
        wired(
            "v1",
            ParamKind::Text,
            Ty::Any,
            txt("Значение 1", "Value 1"),
            json!(""),
            &mut inputs,
        ),
        param("key2", ParamKind::Text, txt("Поле 2", "Field 2"), json!("")),
        wired(
            "v2",
            ParamKind::Text,
            Ty::Any,
            txt("Значение 2", "Value 2"),
            json!(""),
            &mut inputs,
        ),
        param("key3", ParamKind::Text, txt("Поле 3", "Field 3"), json!("")),
        wired(
            "v3",
            ParamKind::Text,
            Ty::Any,
            txt("Значение 3", "Value 3"),
            json!(""),
            &mut inputs,
        ),
    ];
    out.push(NodeDef {
        kind: "list.record",
        group: List,
        title: txt("Собрать запись", "Make a record"),
        description: txt("Запись из полей с именами.", "A record of named fields."),
        help: txt(
            "Поле с пустым именем не попадает в запись. Читается «Полем записи»; удобно передать несколько значений в «Вызвать схему».",
            "A field with an empty name is left out. «Record field» reads it; handy to pass several values to «Call a workflow».",
        ),
        inputs,
        outputs: vec![port("record", Ty::Record, txt("Запись", "Record"))],
        params,
        advanced: true,
        keywords: vec!["запись", "объект", "поля", "record", "object", "struct"],
    });
}

#[allow(clippy::too_many_lines)]
fn loops(out: &mut Vec<NodeDef>) {
    use NodeGroup::Logic;
    let loop_outputs = |value: super::catalog::PortDef| {
        vec![
            port("each", Ty::Flow, txt("Каждый раз", "Each time")),
            port("done", Ty::Flow, txt("Готово", "Done")),
            port("index", Ty::Number, txt("Номер (с 0)", "Index (from 0)")),
            value,
        ]
    };

    let mut inputs = vec![flow_in()];
    let params = vec![wired(
        "count",
        ParamKind::Number,
        Ty::Number,
        txt("Сколько раз", "How many times"),
        json!("3"),
        &mut inputs,
    )];
    out.push(NodeDef {
        kind: "loop.repeat",
        group: Logic,
        title: txt("Повторить N раз", "Repeat N times"),
        description: txt(
            "Выполняет ветку «Каждый раз» N раз, потом — «Готово».",
            "Runs the «Each time» branch N times, then «Done».",
        ),
        help: txt(
            "Не больше 200 повторов за запуск. «Номер» — 0, 1, 2… Досрочно выйти — узел «Прервать цикл».",
            "At most 200 times per run. «Index» is 0, 1, 2… To leave early use «Break the loop».",
        ),
        inputs,
        outputs: loop_outputs(port("value", Ty::Number, txt("Номер с 1", "Number from 1"))),
        params,
        advanced: false,
        keywords: vec!["повторить", "цикл", "раз", "repeat", "loop", "times"],
    });

    let mut inputs = vec![flow_in()];
    let params = vec![
        wired(
            "from",
            ParamKind::Number,
            Ty::Number,
            txt("От", "From"),
            json!("1"),
            &mut inputs,
        ),
        wired(
            "to",
            ParamKind::Number,
            Ty::Number,
            txt("До (включительно)", "To (inclusive)"),
            json!("10"),
            &mut inputs,
        ),
        wired(
            "step",
            ParamKind::Number,
            Ty::Number,
            txt("Шаг", "Step"),
            json!("1"),
            &mut inputs,
        ),
    ];
    out.push(NodeDef {
        kind: "loop.range",
        group: Logic,
        title: txt("Цикл по числам", "Count from … to"),
        description: txt(
            "Проходит числа от и до с шагом: 1, 2, … 10.",
            "Goes through the numbers from … to by a step: 1, 2, … 10.",
        ),
        help: txt(
            "Шаг может быть отрицательным (10, 9, … 1). Не больше 200 значений. Пример: дни месяца 1…«Дней в месяце» → «Собрать дату».",
            "The step may be negative (10, 9, … 1). At most 200 values. Example: the days of a month 1…«Days in the month» → «Make a date».",
        ),
        inputs,
        outputs: loop_outputs(port("value", Ty::Number, txt("Число", "Number"))),
        params,
        advanced: false,
        keywords: vec!["цикл", "от до", "по порядку", "for", "range", "loop", "count"],
    });

    let inputs = vec![
        flow_in(),
        port("condition", Ty::Bool, txt("Пока да", "While yes")),
    ];
    let params = vec![param(
        "max",
        ParamKind::Number,
        txt("Не больше раз", "At most times"),
        json!("50"),
    )];
    out.push(NodeDef {
        kind: "loop.while",
        group: Logic,
        title: txt("Пока условие да", "While"),
        description: txt(
            "Повторяет ветку, пока условие да; условие проверяется перед каждым разом.",
            "Repeats the branch while the condition is yes; it is checked before each time.",
        ),
        help: txt(
            "Условие считается заново каждый раз, так что внутри ветки можно менять переменную, от которой оно зависит. Ограничение «Не больше раз» (до 200) защищает от бесконечного цикла.",
            "The condition is worked out anew each time, so the branch may change a variable it depends on. «At most times» (up to 200) guards against an endless loop.",
        ),
        inputs,
        outputs: loop_outputs(port("value", Ty::Number, txt("Номер с 1", "Number from 1"))),
        params,
        advanced: true,
        keywords: vec!["пока", "цикл", "while", "loop", "until"],
    });

    out.push(NodeDef {
        kind: "loop.break",
        group: Logic,
        title: txt("Прервать цикл", "Break the loop"),
        description: txt(
            "Выходит из ближайшего цикла: дальше — его «Готово».",
            "Leaves the nearest loop: its «Done» comes next.",
        ),
        help: txt(
            "Работает внутри «Повторить», «Цикл по числам», «Пока» и «Для каждого». Вне цикла просто заканчивает ветку.",
            "Works inside «Repeat», «Count from … to», «While» and «For each». Outside a loop it just ends the branch.",
        ),
        inputs: vec![flow_in()],
        outputs: vec![],
        params: vec![],
        advanced: true,
        keywords: vec!["прервать", "выйти", "стоп цикла", "break", "exit loop"],
    });
}
