# Суммы в обход `<Money>` (опись, задача #203)

Опись без правок кода. Дата: 2026-10-25.

## Как устроена общая сумма

- `entities/currency/ui/money/Money.vue` рисует число и приглушённый знак валюты. Если сумма
  «конвертируемая» (валюта = базовая, любая фиатная или избранная; сумма не 0), на `<span>` ставятся
  `data-money-convertible`, `data-money-amount`, `data-money-currency`, `data-money-at`.
  Ничего больше в разметку не монтируется (правило 17).
- Хосты один на всё приложение, живут в `AppShell.vue`:
  - `MoneyCardHost` ловит **клик** (касание на телефоне) по такой сумме: карточка у суммы на ПК,
    `Drawer` на телефоне; в карточке base по курсу операции (`at`) и сегодняшнему, избранные, «Все курсы»,
    клавиша C копирует.
  - `MoneyTooltipHost` ловит **наведение**, но только для суммы внутри кнопки, ссылки, пункта меню
    и т. п. (`PRESSABLE` в `entities/currency/lib/money-target.ts`), где клик занят.
- Причины, по которым «не на каждой сумме можно навестись»:
  1. 128 вызовов `formatMoney` в 97 файлах (без мока) дают **строку**; строка в шаблоне не `Money`, у неё нет атрибутов.
  2. Сумма вне кнопки открывается кликом, а не наведением: пользователь наводит и ничего не видит (вероятное расхождение ожиданий; решение за пользователем, см. вопросы).
  3. Нулевая сумма и неизвестная валюта (крипто не из избранных) не конвертируются (`convertible`).
  4. `:at` передаётся только в `TransactionRow.vue:209`: везде, где `Money` стоит в карточке операции, истории и платежах, курс «на день операции» не показывается.
  5. `:convert="false"` стоит в 29 местах `Money` намеренно (поля ввода и т. п.).
- Сейчас `<Money>`: 252 использования в 101 файле.

## Условные обозначения решения

| Код | Значение |
|---|---|
| M | `<Money :amount :currency [:at]>` с карточкой (сумма — отдельный элемент в шаблоне: ячейка, итог, строка) |
| S | сумма внутри фразы `t('key', { amount })`: переводим через `<i18n-t keypath>` со слотом `<Money>`; пока нельзя — остаётся текстом, в baseline |
| T | оставить текстом: тост, `aria-label`, `title`, подсказка поля, пункт Ctrl K/меню/выбора, подпись оси, тултип графика (в нём уже есть конверсии) |
| G | график, у которого тултип без конверсий: подключить `useMoneyChartTooltip` |
| X | не денежная сумма (проценты, километры, размеры, значения для полей ввода) |

Для слоёв (диалог, шторка) решение M/S надо проверить: хосты карточки лежат в оболочке, а диалог ставит фокус-ловушку;
карточка поверх `Dialog` должна открываться и закрываться Esc, не закрывая диалог (проверка в `check-layout`, см. `docs/ui-overlays.md`).

## Таблица

Колонка «валюта / дата»: из чего берётся валюта и есть ли дата операции для `at`.

### Оболочка / сущности (entities, shared)

| № | файл:строка | что за сумма | валюта / дата | решение |
|---|---|---|---|---|
| E1 | `entities/store/lib/wallet-option.ts:23` | баланс кошелька в подписи пункта выбора | base; даты нет | T |
| E2 | `entities/store/ui/store-positions/StorePositions.vue:59,69,91` | таблица позиций: средняя цена, цена, стоимость (колонки TanStack, строка из money()) | base; даты нет | M |
| E3 | `entities/transaction/ui/transactions-list/use-row-menu.ts:96` | значение в пункте контекстного меню строки | символ актива; at есть (метка операции) | T |
| E4 | `entities/transaction/ui/tx-preview/TxPreviewCard.vue:18` | количества ног в превью (numberFormat напрямую, не formatMoney) | символ актива; даты нет | M |
| E5 | `entities/transaction/ui/tx-preview/TxPreviewCard.vue:51` | лимит кредитки в предупреждении | base | T |
| E6 | `entities/report/lib/insight-text.ts:64` | сумма в тексте вывода отчёта (строка из lib) | валюта отчёта | S |
| E7 | `entities/currency/model/chart-tooltip.ts:43; conversions.ts:72; ui/money-card/MoneyCard.vue:31-68` | сама карточка и тултип графика (строки конверсий) | — | T (это и есть общий механизм) |

### Главная

| № | файл:строка | что за сумма | валюта / дата | решение |
|---|---|---|---|---|
| H1 | `pages/dashboard/ui/home-figures/HomeFigures.vue:44-56,100` | «в день», перенос, лимит, остаток (limitLine — строка в шаблон) | валюта бюджета | S |
| H2 | `pages/dashboard/ui/home-tasks/HomeTasks.vue:32` | суммы в строке задач | валюта пропса | S |
| H3 | `widgets/month-summary/MonthSummary.vue:29,92` | движение бюджета за месяц | валюта месяца | S |
| H4 | `widgets/sidebar-pinned/lib/pinned-item.ts:10` | значение рядом с закрепом (внутри ссылки) | валюта закрепа | M (внутри ссылки сработает тултип) |
| H5 | `widgets/sidebar-wallets/lib/totals.ts:7,14 (SidebarWalletItem, SidebarWallets)` | итог кошелька и группы в боковой панели (внутри ссылки) | base | M |
| H6 | `pages/stores/ui/WalletsHeader.vue:24; WalletTable.vue:76; WalletAssets.vue:96; BillLineRow.vue:70; MeterFields.vue:93; StoreChart.vue:86` | только знак валюты currencySymbol — не сумма | — | X |

### Взгляды (quick-glance)

| № | файл:строка | что за сумма | валюта / дата | решение |
|---|---|---|---|---|
| G1 | `widgets/quick-glance/ui/CapitalGlance.vue:28-71` | итог капитала, строки групп (money/signed) | base; даты нет | M |
| G2 | `widgets/quick-glance/ui/CategoryGlanceBody.vue:53,59,69` | итог месяца по категории, платёж | base; у платежа at есть | M |
| G3 | `widgets/quick-glance/ui/CategoryGlanceBody.vue:39-42` | «до этого», среднее, лимит | base | S |
| G4 | `widgets/quick-glance/ui/SpentGlance.vue:72,82,92` | потрачено, доли, платёж | base; у платежа at есть | M |
| G5 | `widgets/quick-glance/ui/SpentGlance.vue:44,74` | лимит и «в прошлом месяце» | base | S |
| G6 | `widgets/quick-glance/ui/WalletGlanceBody.vue:47,52,60` | баланс, дочерние, платёж | base / символ ноги; at есть | M |
| G7 | `widgets/quick-glance/ui/AutomationGlance.vue:61,85` | сумма автоматизации | символ ноги | M |
| G8 | `widgets/quick-glance/lib/payments.ts:24 (payment.amount в 3 взглядах)` | сумма платежа строкой | символ актива; at есть | M |

### Операции

| № | файл:строка | что за сумма | валюта / дата | решение |
|---|---|---|---|---|
| O1 | `pages/transactions/ui/filter-total/FilterTotal.vue:36,42` | итог фильтра | base | M |
| O2 | `features/transaction-bulk/ui/transaction-bulk-bar/TransactionBulkBar.vue:28,82,98` | итоги выделенных (по валютам, join) | символы | M |
| O3 | `widgets/transaction-details/ui/TxFacts.vue:37,52` | курс и стоимость в карточке операции | символ; at есть (дата операции) | M (+ at) |
| O4 | `widgets/transaction-details/ui/TxSimilar.vue:37-51` | суммы похожих операций | символ; at есть | M (+ at) |
| O5 | `widgets/transaction-details/ui/TxMore.vue:48-49` | отдано / получено / потеря цепочки | валюта цепочки; at есть | M (+ at) |
| O6 | `widgets/flow-list/FlowList.vue:85-88` | «отдал / получил / потеря» во фразе | валюта потока | S |
| O7 | `widgets/quick-add/lib/saved-text.ts:14` | текст тоста «сохранено» с суммой | символ ноги | T (тост) |
| O8 | `widgets/workflow-editor/model/use-run-summary.ts:26` | сводка прогона автоматизации | символ | T |

### Операции: формы быстрого ввода

| № | файл:строка | что за сумма | валюта / дата | решение |
|---|---|---|---|---|
| OF1 | `features/transaction-create/ui/tx-quick-convert/TxQuickConvert.vue:34,48-51,97` | рыночная цена, переплата, переплата в base | символ; даты нет | M |
| OF2 | `features/transaction-create/ui/tx-quick-spot/TxQuickSpot.vue:80,130` | итог сделки в превью | символ котировки | M |
| OF3 | `features/transaction-create/ui/tx-quick-futures/TxQuickFutures.vue:55` | цена ликвидации | base | M |
| OF4 | `features/transaction-create/model/use-tx-budget.ts:57` | превышение бюджета (предупреждение формы) | валюта бюджета | T |
| OF5 | `features/transaction-create/model/use-tx-pricing.ts:113` | подсказка «1 BTC = …» у поля | символ | T |
| OF6 | `features/store-adjust/lib/adjustment-rows.ts:45; model/use-adjustment.ts:162` | строки «было / станет» в диалоге корректировки | символ актива | M |
| OF7 | `features/price-alert-edit/PriceAlertDialog.vue:52` | «сейчас» — подсказка поля | валюта | T |
| OF8 | `features/automation-edit/ui/automation-amount-dialog/AutomationAmountDialog.vue:59` | подсказка поля: оценка | символ | T |
| OF9 | `features/automation-edit/ui/save-share-dialog/SaveShareDialog.vue:52` | «из X — Y» в превью диалога | валюта зарплаты | S |
| OF10 | `features/automation-simulate/AutomationSimulationDialog.vue:58` | строка ноги в симуляции (с названием кошелька) | символ ноги | M |
| OF11 | `features/workflow-manage/ui/confirm-dialog/AutomationConfirmDialog.vue:38,82; 47-48` | сумма в подтверждении; доход / процент во фразе | валюта | M (82), S (47-48) |
| OF12 | `features/goal-delete/ui/GoalDelete.vue:26` | «накоплено» в подтверждении удаления цели | валюта цели | S |
| OF13 | `features/base-currency-change/model/use-base-currency-change.ts:48-49` | «было → станет» в подтверждении смены базовой валюты | обе валюты | S |
| OF14 | `features/property-sell/ui/property-sell-dialog/PropertySellDialog.vue:32-66` | балансовая стоимость, прибыль (colored) | валюта объекта | M |
| OF15 | `features/invest-calculator/lib/advanced-summary.ts:24` | строка-сводка свёрнутой секции | валюта формы | T |
| OF16 | `features/plan-calculator/lib/result-rows.ts:22-31` | строки результата калькулятора плана | base / символы | M |
| OF17 | `features/plan-calculator/model/use-calc-preview.ts:112` | отношение цен (курс, numberFormat) | — | X |
| OF18 | `features/utility-bill/model/line-note.ts:31-51; features/utility-edit/ui/meter-fields/MeterFields.vue:28-46,127` | подписи под строками счёта, пример расчёта тарифа | валюта услуги | S |
| OF19 | `features/budget-edit/ui/BudgetAmountStep.vue:25-47; BudgetTargetStep.vue:23; BudgetDialog.vue:108` | траты, «обычно», лимит, подтверждение | валюта бюджета | M (47), S (остальное) |
| OF20 | `features/goal-edit/ui/GoalWhereStep.vue:47-77` | «уже накоплено» (77), «после» и «внести сейчас» (59,72) | валюта цели | M (77), S |
| OF21 | `features/emergency-fund-edit/EmergencyFundDialog.vue:31-46` | подушка: нужно, цель, разрыв | валюта | S |

### Кошельки

| № | файл:строка | что за сумма | валюта / дата | решение |
|---|---|---|---|---|
| W1 | `pages/wallet/ui/wallet-header/WalletHeader.vue:71` | курс в шапке (только ПК) | валюта фигуры | M |
| W2 | `pages/wallet/ui/utility-item/UtilityItemSheet.vue:41-49` | шторка услуги: тариф, среднее, прогноз (пары метка-значение) | валюта услуги; at — месяц | M |
| W3 | `pages/wallet/ui/utility-item/UtilityReadingCard.vue:78-84,162` | расчёт по показанию | валюта услуги | M |
| W4 | `pages/wallet/lib/utility-item.ts:28-38 (UtilityItemHistory.vue:54)` | история платежей услуги, колонка суммы | валюта; at — период | M (+ at) |
| W5 | `pages/wallet/lib/utility-line.ts:51-70 (UtilityBillLines.vue:45,81; UtilityBillCard.vue:36)` | строки счёта: сумма (45,81), оценка, тариф | валюта услуги | M (сумма), S (тариф) |
| W6 | `pages/wallet/ui/utilities-tab/UtilityBillCard.vue:67` | среднее во фразе | валюта | S |
| W7 | `widgets/utility-list/UtilityCard.vue:39,91; PropertySummaryCard.vue:23,78-79` | в месяц, коммуналка, ипотека | валюта | S |
| W8 | `widgets/goal-details/GoalDetailsSheet.vue:39-111` | шторка цели: накоплено / осталось (80), фразы (46,48), prop :saved (111) | валюта цели | M (80), S, T (111) |
| W9 | `widgets/goal-list/GoalList.vue:56-63; GoalCalendarTable.vue:24-64` | предупреждения и нехватка по месяцам | валюта | S |
| W10 | `widgets/budget-list/BudgetList.vue:142-143` | осталось / перерасход | валюта бюджета | S |
| W11 | `widgets/fixed-costs/FixedCosts.vue:48` | итог постоянных расходов (join по валютам) | валюты | M |
| W12 | `widgets/bonus-programs/model/use-program-rows.ts:40-41` | «1 балл = X ₽» | валюта | S |
| W13 | `widgets/capital-health/lib/reason-text.ts:18` | причины оценки здоровья капитала (строка из lib) | валюта | S |

### Деньги (экран «Деньги»)

| № | файл:строка | что за сумма | валюта / дата | решение |
|---|---|---|---|---|
| M1 | `pages/money/ui/spending/SpendingAnswer.vue:31-75` | главный ответ: доход (70), осталось (72), всего (75), среднее/потрачено во фразах | валюта баланса | M (70,72,75), S |
| M2 | `pages/money/ui/spending/SpendingMonths.vue:25-66` | «остаток» и среднее; aria-label 66 | валюта; at — месяц | M, T (66) |
| M3 | `pages/money/ui/spending/SpendingCategories.vue:29-55,128,149` | итого / среднее / лимит в таблице и карточках; фразы «из лимита» | валюта; at — месяц | M (128,149), S |
| M4 | `pages/money/ui/month-balance/MonthNow.vue:17-46` | доход, расход, перенос, итого; фразы | валюта баланса | M (41-46), S (23-31) |
| M5 | `pages/money/ui/month-balance/MonthBalanceCard.vue:22-91` | остаток месяца (91), итог (89) | валюта | M, S |
| M6 | `pages/money/ui/month-balance/MonthDaysChart.vue:40` | подпись графика «в день» | валюта | T |
| M7 | `pages/money/ui/category-sheet/CategorySheet.vue:53-80; CategoryMonths.vue:17-42` | шторка категории: итого (80), по месяцам (42), среднее/обычно | валюта; at — месяц | M, S |
| M8 | `pages/money/ui/where/WhereAnswer.vue:11-27; MoneyWhere.vue:32,65` | заголовок «доход/потрачено/осталось» | валюта отчёта | S |
| M9 | `pages/money/ui/where/WhereTable.vue:40-77; lib/where-view.ts:5; WhereOpened.vue:39,53` | суммы в таблице «Куда ушли» и раскрытых строках | валюта отчёта | M |
| M10 | `pages/money/ui/plan/PlanNext.vue:21` | ближайшие платежи | символ актива; at — дата платежа | M (+ at) |
| M11 | `pages/money/ui/plan/PlanGoals.vue:26-64` | накоплено / цель (51), остаток, подсказка | валюта цели | M (51), S |
| M12 | `pages/money/ui/plan/PlanLimits.vue:50-118` | потрачено / лимит (102), осталось, перерасход | валюта бюджета | M (102), S |
| M13 | `pages/money/ui/wealth/WealthTable.vue:15,31,42` | капитал по группам | валюта | M |
| M14 | `pages/money/ui/wealth/WealthHead.vue:24-28` | большое число капитала (разбито на число и знак вручную) | валюта | M (compact) |
| M15 | `pages/money/ui/wealth/WealthChart.vue:53` | подпись столбца графика | валюта | T |
| M16 | `pages/money/ui/investments/InvestTable.vue:34-45; lib/invest-view.ts:7` | вложено, прибыль | валюта | M |
| M17 | `pages/money/lib/health-view.ts:54-72` | карточки здоровья: долги, подушка; подсказки | валюта | M (значение 72), S |

### Аналитика и графики (widgets/analytics-reports)

| № | файл:строка | что за сумма | валюта / дата | решение |
|---|---|---|---|---|
| A1 | `widgets/analytics-reports/views/CategoriesView.vue:24,133` | ячейки таблицы категорий | валюта отчёта; at — месяц | M |
| A2 | `widgets/allocation-pie/AllocationPie.vue:39` | тултип пирога БЕЗ конверсий (комментарий «hover принадлежит цифрам рядом») | валюта | G |
| A3 | `widgets/analytics-reports/views/PortfolioView.vue:87` | тултип пирога БЕЗ конверсий (свой formatter поверх useMoneyChartTooltip) | валюта | G |
| A4 | `графики с useMoneyChartTooltip: Cashflow, Pnl, Networth, Saldo, Inflation, Losses, Portfolio(линия), StoreChart, MonthDaysChart, WealthChart, InvestResult, UtilityStats` | тултип с конверсиями (+ «≈» под значением) | валюта графика | T (уже верно) |
| A5 | `shared/lib/chart-theme.ts:65 (axisMoney: StoreChart, WealthChart, views/chart.ts, result-chart.ts, echarts.ts)` | подписи осей | валюта | T |
| A6 | `widgets/trading-chart/*, topup-advisor/advisor-chart.ts, capital-health/ui/SavingsCard.vue, inflation-settings/ui/InflationSpark.vue` | цены свечей (.toFixed), проценты — не денежные суммы | — | X |
| A7 | `widgets/invest-result/ui/ResultAnswer.vue:31-80; ResultHint.vue:24-29; lib/answer.ts:39` | ответ калькулятора инвестиций: 7 фраз с суммами | валюта формы | S |

### Автоматизации

| № | файл:строка | что за сумма | валюта / дата | решение |
|---|---|---|---|---|
| AU1 | `pages/automations/lib/mine-view.ts:19` | сумма в карточке «мои автоматизации» | валюта ноги | M |
| AU2 | `widgets/automation-details/lib/card-view.ts:12` | сумма в карточке автоматизации | валюта ноги | M |
| AU3 | `widgets/automation-list/AutomationList.vue:59` | сумма ноги в списке (с «~») | символ | M |

### Ctrl K

| № | файл:строка | что за сумма | валюта / дата | решение |
|---|---|---|---|---|
| K1 | `pages/app-shell/ui/command-palette/PaletteSearch.vue:64` | «сумма за месяц» в пункте результата | base | T (строка внутри пункта списка) |
| K2 | `pages/app-shell/ui/command-palette/PaletteConvert.vue:30-34` | строки конвертера (курс сегодня / на дату) | валюты строк | T |

### Не суммы (учтены для полноты)

| № | файл:строка | что за сумма | валюта / дата | решение |
|---|---|---|---|---|
| X1 | `toFixed/numberFormat без валюты: transaction-create/lib/form/linked.ts, use-tx-autofill.ts; utility-bill/model/price-row.ts, UtilityBillDialog.vue:86; command-palette/convert-query.ts:74; entities/attachment/lib/attachment-format.ts; widgets/vehicle-card/VehicleCard.vue:18; CurrencySettings.vue:66; shared/ui/sparkline, amount-input, lib/decimal.ts` | значения полей ввода, размеры файлов, километры, цвета | — | X |

## Сводка

Единица счёта — блок «файл или группа файлов с одной логикой» (строки таблицы); всего вызовов `formatMoney` без мока — 128 в 97 файлах, ещё `numberFormat`/`toFixed` для денег — единицы.

| | Блоков |
|---|---|
| Всего блоков в таблице | 93 |
| не денежные суммы (X) | 4 |
| Денежных блоков | 89 |
| переводим в `<Money>` (M) | 37 |
| частично `<Money>`, частично во фразе (M+S) | 13 |
| во фразе, переводим через `<i18n-t>` (S) | 20 |
| оставить текстом (T: тост, подсказка, ось, Ctrl K, меню, тултип графика) | 17 |
| график без конверсий в тултипе (G) | 2 |

Итого переводим: 70 блоков из 89 (M 37 + M+S 13 + S 20), оставляем 17, чиним график 2.
Больше всего долга: экран «Деньги» (17 блоков), взгляды (8), формы быстрого ввода и подтверждения (21).

## Открытые вопросы пользователю

1. Сумма вне кнопки открывается кликом (карточка), наведением — только внутри кнопок и ссылок. Хотите наведение везде на ПК (тултип, как внутри кнопки) в дополнение к клику?
2. Фразы (S): везде переводить на `<i18n-t>` или только в основном тексте экранов (ответ «Деньги», взгляды), а подсказки и подтверждения оставить?
3. `at` для карточки операции, платежа и месяца: передавать везде, где дата известна (O3-O5, G2, G4, G6, G8, W4, M10)?

## Предложение: правило для `tests/checks/ui.check.ts`

Прежнее (удалено, покрыто новыми) `format-money` ловит только `{{ formatMoney(…) }}` и `:value="formatMoney("` прямо в шаблоне. Почти вся обходная выдача идёт
через локальный помощник (`const money = (v) => formatMoney(v, …)`), через `computed`, через `lib/*.ts`, возвращающий строку,
поэтому правило не срабатывает ни разу на текущем коде.

Предлагаем два новых правила в том же формате `{ id, vueOnly, find, say }` с учётом `check-ui.baseline.json` (долг фиксируется числом по `правило файл` и только уменьшается).

### 1. `money-text` (.vue)

1. В `<script setup>` собрать имена «денежных помощников»: `const X = (…) => … formatMoney(` / `moneyParts(` / `useMoney().money`, а также `const X = computed(() => … formatMoney(`.
2. В `<template>` искать мустачи, где **всё содержимое** — вызов помощника или его имя: `{{ money(a) }}`, `{{ total }}`, `{{ money(a) }} / {{ money(b) }}`; тот же поиск среди атрибутов `:label`, `:text`, `:value`.
3. Разрешено без метки (не срабатывает): мустач внутри вызова `t(…)`/`te(…)` (это S, отдельное правило-предупреждение), атрибуты `:title`, `:aria-label`, `:placeholder`, `:hint`, `:description`, текст внутри `<option>`, `<SelectItem>`, `<CommandItem>`, `<Tooltip*>`, `<Toast`.
4. Метка исключения: комментарий `<!-- money: plain — причина -->` перед элементом или `// money: plain — причина` перед объявлением помощника. Причина обязательна, пустая — нарушение.
5. Белый список файлов: `entities/currency/**`, `shared/lib/chart-theme.ts`, `**/model/*chart*.ts`, `shared/api/mock/**`, `**/command-palette/**` (Ctrl K — только текст).

### 2. `money-lib` (.ts в `lib/` и `model/`)

`formatMoney(` в `pages|widgets|features|entities/**/lib|model/*.ts`, **кроме** белого списка выше и файлов с отметкой `// money: plain — …` в первой строке: функция возвращает строку, которую шаблон потом выводит как есть. Такие файлы переходят на возврат структуры `{ amount, currency, at }`, а шаблон рисует `<Money>`.

### Примеры ложных срабатываний

- `:aria-label="`${month}: ${money(row.spent)}`"` (SpendingMonths.vue:66): мустач нет, атрибут `aria-label` в разрешённых.
- `notify.success(t('saved', { amount: money(x) }))` в `<script>`: тост, не шаблон.
- `const label = computed(() => money(x).replace(/\s*\D+$/, ''))` (`bare` в PlanGoals, GoalDetailsSheet, GoalWhereStep): число без знака для пары «накоплено / цель»; после перевода на `Money` пара станет `<Money :currency="…"/>` без знака, поэтому правило должно разрешать `bare`, если рядом есть вторая `Money` (метка `money: plain`).
- `{{ currencySymbol(x) }}`: только знак валюты (WalletTable, MeterFields) — не `formatMoney`, правило не должно ловить `currencySymbol`.
- `formatMoney(0, { decimals: 0, currency }).replace(/^[\d\s,.]+/, '')` (WealthHead.vue:28): берёт только знак валюты из нуля; обходной приём, заменить `currencySymbol`.
- Значение `:value="money(x)"` для `StatCard` с `Money` в слоте: правило не должно срабатывать, если в слоте есть `<Money`.
- Помощник `money`, вызванный только внутри `t(…)` в шаблоне (S): не ошибка, а предупреждение `money-phrase` (считается, в baseline не растёт).
- Локальная функция `money`, возвращающая число (`const money = (v: string) => Number(v)`): отсекать по тому, что тело вызывает `formatMoney`.

Метрика прогресса: `mise run check-ui` печатает число `money-text` и `money-phrase` в baseline; цель по долгу — 0 / 0.

## Сделано (этап 6¾, часть 1)

- Правила `money-text` (35 файлов, 57 нарушений) и `money-lib` (24 файла, 30 нарушений) в `tests/checks/rules/ui/rules-money.mjs`, долг в `check-ui.baseline.json`; `format-money` удалено (покрыто `money-text`). Метка исключения: `money: plain — <причина>`.
- Наведение на любую `<Money>` (ПК) показывает тултип, клик открывает карточку; с `at` тултип и «Избранные» в карточке — по курсу на дату операции. Касание — карточка; в кнопке/ссылке — удержание 500 мс; в строке операции — пункт меню «Сумма в других валютах». С клавиатуры — фокус на кнопке с суммой.

## Сделано (этап 6¾, часть 3)

- **Один механизм фраз с суммами.** Остался `src/shared/lib/money-parts.ts`: `composeSums()` даёт `mark(сумма | SumPart[])` (маркер для `t(…)` или строки) и `parts(текст)` (слова и суммы → `SumPart[]`), плюс `isSum`, `partsText` (для `title`/`aria-label`). `fillPhrase` и `money-compose.ts` удалены, все потребители переведены. Маркер можно подставлять в один текст несколько раз.
- **Общий рендер.** `<MoneyPhrase :parts>` в `entities/currency/ui/money-phrase` (экспорт из `index.ts`) заменил `pages/money/ui/SumParts.vue` и все `v-for` + `isSum` / `'text' in part`. Для разметки из одной-двух сумм в шаблоне — `<i18n-t keypath>` со слотами `<Money>`.
- **`bare` у `Money`**: число без знака валюты, тултип и карточка как обычно. Пары «накоплено / цель» («36 из 50 $», «1 539 из 5 000 $») — первая сумма `bare` (`BudgetAmountStep`, `GoalWhereStep`, `PlanGoals`, `PlanLimits`, `SpendingMonths`, `MonthBalanceCard`, `SpendingCategories`, `GoalDetailsSheet`). CSS-хак `[&>span:nth-child(2)]:hidden` удалён.
- **`at` принимает месяц `YYYY-MM`** (первое число месяца, `entities/currency/lib/money-moment.ts`; `Money` кладёт в `data-money-at` уже нормализованное, `useConversionsAt` нормализует сам). Ручной `-01T00:00:00Z` убран (`utility-item.ts`).
- **`SumPart`** получил `bare` и `convert` (`convert: false` — число, которое не сумма: курс пары в `TxSimilar`). У `FormField` появился слот `hint` (подсказка с суммой).
- **Пробелы правила `money-text`.** Почему пропускало: (1) сумма внутри `t('ключ', { … formatMoney(…) })` вырезалась как «аргумент фразы» (`withoutPhrases`) — через неё ушли `MoneyWhere`, `WhereAnswer`, `MonthDaysChart`, `UtilityBillCard:67`, `TxMore` и др.; (2) мусташи с двумя уровнями фигурных скобок (`{{ t('k', { a: formatMoney(x, { currency }) }) }}`) не разбирались регулярным выражением (`UtilityBillCard`); (3) списки, собранные помощником (`v-for="row in rows"`, `UtilityItemSheet`, `SpendingCategories`) и (4) текстовые props `:hint|:line|:description|:subtitle|:caption|:message|:summary|:detail` (`PriceAlertDialog`, `AutomationList`, `TxSimilar`) не смотрелись; (5) `money-lib` проверял только `lib/` и `model/`, а не любой `.ts` слайса (`use-row-menu.ts`). Исправлено: вырезание `t(…)` убрано, мусташи разбираются со сбалансированными скобками, `v-for` над помощником с суммой и расширенный список атрибутов ловятся, `money-lib` смотрит все `.ts` слайсов.
- Новые находки (23 файла) переведены на `<Money>` / `<MoneyPhrase>` / `<i18n-t>`; графики оставлены на `useMoneyChartTooltip`, подписи осей — текст.
- Оставшиеся вызовы `formatMoney(` (без мока): см. отчёт задачи; все либо в белом списке (`entities/currency`, `shared/lib`, палитра), либо с меткой `money: plain — <причина>` (тосты, `aria-label`, строковые props `SwitchRow`/`ActionSheet`/`FormField`, курс обмена не сумма).
