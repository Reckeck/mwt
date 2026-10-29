# Amounts that bypass `<Money>` (inventory, task #203)

An inventory without code changes. Date: 2026-10-25.

## How the shared amount works

- `entities/currency/ui/money/Money.vue` renders the number and a muted currency sign. If the amount is
  "convertible" (currency = base, any fiat or favorite; the amount is not 0), the `<span>` gets
  `data-money-convertible`, `data-money-amount`, `data-money-currency`, `data-money-at`.
  Nothing else is mounted into the markup (rule 17).
- There is one set of hosts for the whole app, living in `AppShell.vue`:
  - `MoneyCardHost` catches a **click** (a tap on a phone) on such an amount: a card by the amount on desktop,
    a `Drawer` on a phone; the card has the base value at the transaction's rate (`at`) and today's, favorites, "All rates",
    the C key copies.
  - `MoneyTooltipHost` catches **hover**, but only for an amount inside a button, link, menu item
    and the like (`PRESSABLE` in `entities/currency/lib/money-target.ts`), where the click is taken.
- Reasons why "not every amount can be hovered":
  1. 128 `formatMoney` calls in 97 files (excluding the mock) yield a **string**; a string in a template is not `Money`, it has no attributes.
  2. An amount outside a button opens by click, not by hover: the user hovers and sees nothing (a likely mismatch of expectations; the decision is the user's, see the questions).
  3. A zero amount and an unknown currency (crypto not among the favorites) are not converted (`convertible`).
  4. `:at` is passed only in `TransactionRow.vue:209`: everywhere `Money` sits in a transaction card, history and payments, the "rate on the transaction day" is not shown.
  5. `:convert="false"` is deliberately set in 29 places of `Money` (input fields and the like).
- Currently `<Money>`: 252 uses in 101 files.

## Decision legend

| Code | Meaning |
|---|---|
| M | `<Money :amount :currency [:at]>` with a card (the amount is a separate element in the template: a cell, a total, a row) |
| S | an amount inside a phrase `t('key', { amount })`: translate through `<i18n-t keypath>` with a `<Money>` slot; if not yet possible, it stays text, in the baseline |
| T | keep as text: a toast, `aria-label`, `title`, a field hint, a Ctrl K/menu/picker item, an axis label, a chart tooltip (it already has conversions) |
| G | a chart whose tooltip has no conversions: hook up `useMoneyChartTooltip` |
| X | not a money amount (percentages, kilometers, sizes, values for input fields) |

For layers (dialog, sheet) the M/S decision must be checked: the card hosts live in the shell, while a dialog sets a focus trap;
a card over a `Dialog` must open and close with Esc without closing the dialog (checked in `check-layout`, see `docs/ui-overlays.md`).

## Table

The "currency / date" column: where the currency comes from and whether there is a transaction date for `at`.

### Shell / entities (entities, shared)

| # | file:line | what amount | currency / date | decision |
|---|---|---|---|---|
| E1 | `entities/store/lib/wallet-option.ts:23` | wallet balance in a picker item label | base; no date | T |
| E2 | `entities/store/ui/store-positions/StorePositions.vue:59,69,91` | positions table: average price, price, value (TanStack columns, a string from money()) | base; no date | M |
| E3 | `entities/transaction/ui/transactions-list/use-row-menu.ts:96` | a value in a row context menu item | asset symbol; at exists (the transaction label) | T |
| E4 | `entities/transaction/ui/tx-preview/TxPreviewCard.vue:18` | leg quantities in the preview (numberFormat directly, not formatMoney) | asset symbol; no date | M |
| E5 | `entities/transaction/ui/tx-preview/TxPreviewCard.vue:51` | credit card limit in a warning | base | T |
| E6 | `entities/report/lib/insight-text.ts:64` | an amount in the report conclusion text (a string from lib) | report currency | S |
| E7 | `entities/currency/model/chart-tooltip.ts:43; conversions.ts:72; ui/money-card/MoneyCard.vue:31-68` | the card itself and the chart tooltip (conversion rows) | — | T (this is the shared mechanism itself) |

### Home

| # | file:line | what amount | currency / date | decision |
|---|---|---|---|---|
| H1 | `pages/dashboard/ui/home-figures/HomeFigures.vue:44-56,100` | "per day", carry-over, limit, remainder (limitLine is a string in the template) | budget currency | S |
| H2 | `pages/dashboard/ui/home-tasks/HomeTasks.vue:32` | amounts in a task row | prop currency | S |
| H3 | `widgets/month-summary/MonthSummary.vue:29,92` | the budget movement for the month | month currency | S |
| H4 | `widgets/sidebar-pinned/lib/pinned-item.ts:10` | a value next to a pin (inside a link) | pin currency | M (the tooltip works inside a link) |
| H5 | `widgets/sidebar-wallets/lib/totals.ts:7,14 (SidebarWalletItem, SidebarWallets)` | the total of a wallet and a group in the sidebar (inside a link) | base | M |
| H6 | `pages/stores/ui/WalletsHeader.vue:24; WalletTable.vue:76; WalletAssets.vue:96; BillLineRow.vue:70; MeterFields.vue:93; StoreChart.vue:86` | only the currencySymbol sign — not an amount | — | X |

### Glances (quick-glance)

| # | file:line | what amount | currency / date | decision |
|---|---|---|---|---|
| G1 | `widgets/quick-glance/ui/CapitalGlance.vue:28-71` | the capital total, group rows (money/signed) | base; no date | M |
| G2 | `widgets/quick-glance/ui/CategoryGlanceBody.vue:53,59,69` | the month total for a category, a payment | base; the payment has at | M |
| G3 | `widgets/quick-glance/ui/CategoryGlanceBody.vue:39-42` | "before that", average, limit | base | S |
| G4 | `widgets/quick-glance/ui/SpentGlance.vue:72,82,92` | spent, shares, a payment | base; the payment has at | M |
| G5 | `widgets/quick-glance/ui/SpentGlance.vue:44,74` | the limit and "last month" | base | S |
| G6 | `widgets/quick-glance/ui/WalletGlanceBody.vue:47,52,60` | balance, children, a payment | base / leg symbol; at exists | M |
| G7 | `widgets/quick-glance/ui/AutomationGlance.vue:61,85` | the automation amount | leg symbol | M |
| G8 | `widgets/quick-glance/lib/payments.ts:24 (payment.amount in 3 glances)` | the payment amount as a string | asset symbol; at exists | M |

### Transactions

| # | file:line | what amount | currency / date | decision |
|---|---|---|---|---|
| O1 | `pages/transactions/ui/filter-total/FilterTotal.vue:36,42` | the filter total | base | M |
| O2 | `features/transaction-bulk/ui/transaction-bulk-bar/TransactionBulkBar.vue:28,82,98` | totals of the selected (by currency, join) | symbols | M |
| O3 | `widgets/transaction-details/ui/TxFacts.vue:37,52` | rate and value in the transaction card | symbol; at exists (the transaction date) | M (+ at) |
| O4 | `widgets/transaction-details/ui/TxSimilar.vue:37-51` | amounts of similar transactions | symbol; at exists | M (+ at) |
| O5 | `widgets/transaction-details/ui/TxMore.vue:48-49` | gave / received / chain loss | chain currency; at exists | M (+ at) |
| O6 | `widgets/flow-list/FlowList.vue:85-88` | "gave / received / loss" in a phrase | flow currency | S |
| O7 | `widgets/quick-add/lib/saved-text.ts:14` | the "saved" toast text with an amount | leg symbol | T (toast) |
| O8 | `widgets/workflow-editor/model/use-run-summary.ts:26` | an automation run summary | symbol | T |

### Transactions: quick entry forms

| # | file:line | what amount | currency / date | decision |
|---|---|---|---|---|
| OF1 | `features/transaction-create/ui/tx-quick-convert/TxQuickConvert.vue:34,48-51,97` | market price, overpayment, overpayment in base | symbol; no date | M |
| OF2 | `features/transaction-create/ui/tx-quick-spot/TxQuickSpot.vue:80,130` | the trade total in the preview | quote symbol | M |
| OF3 | `features/transaction-create/ui/tx-quick-futures/TxQuickFutures.vue:55` | liquidation price | base | M |
| OF4 | `features/transaction-create/model/use-tx-budget.ts:57` | budget overrun (a form warning) | budget currency | T |
| OF5 | `features/transaction-create/model/use-tx-pricing.ts:113` | the "1 BTC = …" hint at a field | symbol | T |
| OF6 | `features/store-adjust/lib/adjustment-rows.ts:45; model/use-adjustment.ts:162` | "was / will be" rows in the adjustment dialog | asset symbol | M |
| OF7 | `features/price-alert-edit/PriceAlertDialog.vue:52` | "now" — a field hint | currency | T |
| OF8 | `features/automation-edit/ui/automation-amount-dialog/AutomationAmountDialog.vue:59` | a field hint: an estimate | symbol | T |
| OF9 | `features/automation-edit/ui/save-share-dialog/SaveShareDialog.vue:52` | "of X — Y" in the dialog preview | salary currency | S |
| OF10 | `features/automation-simulate/AutomationSimulationDialog.vue:58` | a leg row in the simulation (with the wallet name) | leg symbol | M |
| OF11 | `features/workflow-manage/ui/confirm-dialog/AutomationConfirmDialog.vue:38,82; 47-48` | an amount in the confirmation; income / percent in a phrase | currency | M (82), S (47-48) |
| OF12 | `features/goal-delete/ui/GoalDelete.vue:26` | "saved" in the goal deletion confirmation | goal currency | S |
| OF13 | `features/base-currency-change/model/use-base-currency-change.ts:48-49` | "was → will be" in the base currency change confirmation | both currencies | S |
| OF14 | `features/property-sell/ui/property-sell-dialog/PropertySellDialog.vue:32-66` | book value, profit (colored) | property currency | M |
| OF15 | `features/invest-calculator/lib/advanced-summary.ts:24` | the summary line of a collapsed section | form currency | T |
| OF16 | `features/plan-calculator/lib/result-rows.ts:22-31` | plan calculator result rows | base / symbols | M |
| OF17 | `features/plan-calculator/model/use-calc-preview.ts:112` | a price ratio (rate, numberFormat) | — | X |
| OF18 | `features/utility-bill/model/line-note.ts:31-51; features/utility-edit/ui/meter-fields/MeterFields.vue:28-46,127` | captions under bill lines, a tariff calculation example | service currency | S |
| OF19 | `features/budget-edit/ui/BudgetAmountStep.vue:25-47; BudgetTargetStep.vue:23; BudgetDialog.vue:108` | spending, "usually", limit, confirmation | budget currency | M (47), S (the rest) |
| OF20 | `features/goal-edit/ui/GoalWhereStep.vue:47-77` | "already saved" (77), "after" and "pay in now" (59,72) | goal currency | M (77), S |
| OF21 | `features/emergency-fund-edit/EmergencyFundDialog.vue:31-46` | the cushion: needed, target, gap | currency | S |

### Wallets

| # | file:line | what amount | currency / date | decision |
|---|---|---|---|---|
| W1 | `pages/wallet/ui/wallet-header/WalletHeader.vue:71` | the rate in the header (desktop only) | figure currency | M |
| W2 | `pages/wallet/ui/utility-item/UtilityItemSheet.vue:41-49` | the service sheet: tariff, average, forecast (label-value pairs) | service currency; at — month | M |
| W3 | `pages/wallet/ui/utility-item/UtilityReadingCard.vue:78-84,162` | the calculation for a reading | service currency | M |
| W4 | `pages/wallet/lib/utility-item.ts:28-38 (UtilityItemHistory.vue:54)` | service payment history, the amount column | currency; at — period | M (+ at) |
| W5 | `pages/wallet/lib/utility-line.ts:51-70 (UtilityBillLines.vue:45,81; UtilityBillCard.vue:36)` | bill lines: amount (45,81), estimate, tariff | service currency | M (amount), S (tariff) |
| W6 | `pages/wallet/ui/utilities-tab/UtilityBillCard.vue:67` | the average in a phrase | currency | S |
| W7 | `widgets/utility-list/UtilityCard.vue:39,91; PropertySummaryCard.vue:23,78-79` | per month, utilities, mortgage | currency | S |
| W8 | `widgets/goal-details/GoalDetailsSheet.vue:39-111` | the goal sheet: saved / remaining (80), phrases (46,48), prop :saved (111) | goal currency | M (80), S, T (111) |
| W9 | `widgets/goal-list/GoalList.vue:56-63; GoalCalendarTable.vue:24-64` | warnings and shortfalls by month | currency | S |
| W10 | `widgets/budget-list/BudgetList.vue:142-143` | remaining / overrun | budget currency | S |
| W11 | `widgets/fixed-costs/FixedCosts.vue:48` | the fixed expenses total (join by currencies) | currencies | M |
| W12 | `widgets/bonus-programs/model/use-program-rows.ts:40-41` | "1 point = X ₽" | currency | S |
| W13 | `widgets/capital-health/lib/reason-text.ts:18` | reasons of the capital health assessment (a string from lib) | currency | S |

### Money (the "Money" screen)

| # | file:line | what amount | currency / date | decision |
|---|---|---|---|---|
| M1 | `pages/money/ui/spending/SpendingAnswer.vue:31-75` | the main answer: income (70), remaining (72), total (75), average/spent in phrases | balance currency | M (70,72,75), S |
| M2 | `pages/money/ui/spending/SpendingMonths.vue:25-66` | "remainder" and average; aria-label 66 | currency; at — month | M, T (66) |
| M3 | `pages/money/ui/spending/SpendingCategories.vue:29-55,128,149` | total / average / limit in the table and cards; "of the limit" phrases | currency; at — month | M (128,149), S |
| M4 | `pages/money/ui/month-balance/MonthNow.vue:17-46` | income, expense, carry-over, total; phrases | balance currency | M (41-46), S (23-31) |
| M5 | `pages/money/ui/month-balance/MonthBalanceCard.vue:22-91` | month remainder (91), total (89) | currency | M, S |
| M6 | `pages/money/ui/month-balance/MonthDaysChart.vue:40` | the "per day" chart caption | currency | T |
| M7 | `pages/money/ui/category-sheet/CategorySheet.vue:53-80; CategoryMonths.vue:17-42` | the category sheet: total (80), by month (42), average/usually | currency; at — month | M, S |
| M8 | `pages/money/ui/where/WhereAnswer.vue:11-27; MoneyWhere.vue:32,65` | the "income/spent/remaining" heading | report currency | S |
| M9 | `pages/money/ui/where/WhereTable.vue:40-77; lib/where-view.ts:5; WhereOpened.vue:39,53` | amounts in the "Where it went" table and expanded rows | report currency | M |
| M10 | `pages/money/ui/plan/PlanNext.vue:21` | upcoming payments | asset symbol; at — payment date | M (+ at) |
| M11 | `pages/money/ui/plan/PlanGoals.vue:26-64` | saved / goal (51), remainder, hint | goal currency | M (51), S |
| M12 | `pages/money/ui/plan/PlanLimits.vue:50-118` | spent / limit (102), remaining, overrun | budget currency | M (102), S |
| M13 | `pages/money/ui/wealth/WealthTable.vue:15,31,42` | capital by group | currency | M |
| M14 | `pages/money/ui/wealth/WealthHead.vue:24-28` | the big capital figure (split into number and sign by hand) | currency | M (compact) |
| M15 | `pages/money/ui/wealth/WealthChart.vue:53` | a chart bar label | currency | T |
| M16 | `pages/money/ui/investments/InvestTable.vue:34-45; lib/invest-view.ts:7` | invested, profit | currency | M |
| M17 | `pages/money/lib/health-view.ts:54-72` | health cards: debts, cushion; hints | currency | M (value 72), S |

### Analytics and charts (widgets/analytics-reports)

| # | file:line | what amount | currency / date | decision |
|---|---|---|---|---|
| A1 | `widgets/analytics-reports/views/CategoriesView.vue:24,133` | category table cells | report currency; at — month | M |
| A2 | `widgets/allocation-pie/AllocationPie.vue:39` | the pie tooltip WITHOUT conversions (a comment "hover belongs to the numbers beside") | currency | G |
| A3 | `widgets/analytics-reports/views/PortfolioView.vue:87` | the pie tooltip WITHOUT conversions (its own formatter on top of useMoneyChartTooltip) | currency | G |
| A4 | `charts with useMoneyChartTooltip: Cashflow, Pnl, Networth, Saldo, Inflation, Losses, Portfolio(line), StoreChart, MonthDaysChart, WealthChart, InvestResult, UtilityStats` | a tooltip with conversions (+ "≈" under the value) | chart currency | T (already correct) |
| A5 | `shared/lib/chart-theme.ts:65 (axisMoney: StoreChart, WealthChart, views/chart.ts, result-chart.ts, echarts.ts)` | axis labels | currency | T |
| A6 | `widgets/trading-chart/*, topup-advisor/advisor-chart.ts, capital-health/ui/SavingsCard.vue, inflation-settings/ui/InflationSpark.vue` | candle prices (.toFixed), percentages — not money amounts | — | X |
| A7 | `widgets/invest-result/ui/ResultAnswer.vue:31-80; ResultHint.vue:24-29; lib/answer.ts:39` | the investment calculator answer: 7 phrases with amounts | form currency | S |

### Automations

| # | file:line | what amount | currency / date | decision |
|---|---|---|---|---|
| AU1 | `pages/automations/lib/mine-view.ts:19` | the amount in the "my automations" card | leg currency | M |
| AU2 | `widgets/automation-details/lib/card-view.ts:12` | the amount in the automation card | leg currency | M |
| AU3 | `widgets/automation-list/AutomationList.vue:59` | the leg amount in the list (with "~") | symbol | M |

### Ctrl K

| # | file:line | what amount | currency / date | decision |
|---|---|---|---|---|
| K1 | `pages/app-shell/ui/command-palette/PaletteSearch.vue:64` | "amount for the month" in a result item | base | T (a string inside a list item) |
| K2 | `pages/app-shell/ui/command-palette/PaletteConvert.vue:30-34` | converter rows (rate today / on a date) | row currencies | T |

### Not amounts (included for completeness)

| # | file:line | what amount | currency / date | decision |
|---|---|---|---|---|
| X1 | `toFixed/numberFormat without a currency: transaction-create/lib/form/linked.ts, use-tx-autofill.ts; utility-bill/model/price-row.ts, UtilityBillDialog.vue:86; command-palette/convert-query.ts:74; entities/attachment/lib/attachment-format.ts; widgets/vehicle-card/VehicleCard.vue:18; CurrencySettings.vue:66; shared/ui/sparkline, amount-input, lib/decimal.ts` | input field values, file sizes, kilometers, colors | — | X |

## Summary

The unit of counting is a block, "a file or a group of files with one logic" (table rows); total `formatMoney` calls excluding the mock — 128 in 97 files, plus a handful of `numberFormat`/`toFixed` for money.

| | Blocks |
|---|---|
| Total blocks in the table | 93 |
| not money amounts (X) | 4 |
| Money blocks | 89 |
| convert to `<Money>` (M) | 37 |
| partly `<Money>`, partly in a phrase (M+S) | 13 |
| in a phrase, convert through `<i18n-t>` (S) | 20 |
| keep as text (T: toast, hint, axis, Ctrl K, menu, chart tooltip) | 17 |
| a chart without conversions in the tooltip (G) | 2 |

Total to convert: 70 blocks of 89 (M 37 + M+S 13 + S 20), keep 17, fix a chart 2.
The most debt: the "Money" screen (17 blocks), glances (8), quick entry forms and confirmations (21).

## Open questions for the user

1. An amount outside a button opens by click (a card), by hover only inside buttons and links. Do you want hover everywhere on desktop (a tooltip, as inside a button) in addition to the click?
2. Phrases (S): translate to `<i18n-t>` everywhere, or only in the main text of screens (the "Money" answer, glances) and leave hints and confirmations?
3. `at` for the transaction card, a payment and a month: pass it everywhere the date is known (O3-O5, G2, G4, G6, G8, W4, M10)?

## Proposal: a rule for `tests/checks/ui.check.ts`

The former rule (removed, covered by the new ones) `format-money` catches only `{{ formatMoney(…) }}` and `:value="formatMoney("` right in a template. Almost all bypassing output goes
through a local helper (`const money = (v) => formatMoney(v, …)`), through a `computed`, through `lib/*.ts` that return a string,
so the rule never fires on the current code.

We propose two new rules in the same `{ id, vueOnly, find, say }` format with `check-ui.baseline.json` taken into account (the debt is recorded as a number per `rule file` and can only decrease).

### 1. `money-text` (.vue)

1. In `<script setup>` collect the names of "money helpers": `const X = (…) => … formatMoney(` / `moneyParts(` / `useMoney().money`, as well as `const X = computed(() => … formatMoney(`.
2. In `<template>` look for mustaches whose **entire content** is a helper call or its name: `{{ money(a) }}`, `{{ total }}`, `{{ money(a) }} / {{ money(b) }}`; the same search among the attributes `:label`, `:text`, `:value`.
3. Allowed without a mark (does not fire): a mustache inside a `t(…)`/`te(…)` call (this is S, a separate warning rule), the attributes `:title`, `:aria-label`, `:placeholder`, `:hint`, `:description`, text inside `<option>`, `<SelectItem>`, `<CommandItem>`, `<Tooltip*>`, `<Toast`.
4. The exception mark: a comment `<!-- money: plain — reason -->` before the element or `// money: plain — reason` before the helper declaration. The reason is mandatory, an empty one is a violation.
5. File whitelist: `entities/currency/**`, `shared/lib/chart-theme.ts`, `**/model/*chart*.ts`, `shared/api/mock/**`, `**/command-palette/**` (Ctrl K — text only).

### 2. `money-lib` (.ts in `lib/` and `model/`)

`formatMoney(` in `pages|widgets|features|entities/**/lib|model/*.ts`, **except** the whitelist above and files marked `// money: plain — …` on the first line: the function returns a string that the template then outputs as is. Such files switch to returning a `{ amount, currency, at }` structure, and the template renders `<Money>`.

### False-positive examples

- `:aria-label="`${month}: ${money(row.spent)}`"` (SpendingMonths.vue:66): there is no mustache, and the `aria-label` attribute is among the allowed.
- `notify.success(t('saved', { amount: money(x) }))` in `<script>`: a toast, not a template.
- `const label = computed(() => money(x).replace(/\s*\D+$/, ''))` (`bare` in PlanGoals, GoalDetailsSheet, GoalWhereStep): a number without a sign for a "saved / goal" pair; after the move to `Money` the pair becomes `<Money :currency="…"/>` without a sign, so the rule must allow `bare` if a second `Money` is nearby (the `money: plain` mark).
- `{{ currencySymbol(x) }}`: only the currency sign (WalletTable, MeterFields) — not `formatMoney`, the rule must not catch `currencySymbol`.
- `formatMoney(0, { decimals: 0, currency }).replace(/^[\d\s,.]+/, '')` (WealthHead.vue:28): takes only the currency sign from zero; a workaround, replace with `currencySymbol`.
- A value `:value="money(x)"` for `StatCard` with `Money` in the slot: the rule must not fire if the slot has `<Money`.
- A `money` helper called only inside `t(…)` in a template (S): not an error but a `money-phrase` warning (counted, does not grow in the baseline).
- A local `money` function returning a number (`const money = (v: string) => Number(v)`): cut off by the body calling `formatMoney`.

Progress metric: `mise run check-ui` prints the count of `money-text` and `money-phrase` in the baseline; the debt target is 0 / 0.

## Done (stage 6¾, part 1)

- The rules `money-text` (35 files, 57 violations) and `money-lib` (24 files, 30 violations) in `tests/checks/rules/ui/rules-money.mjs`, the debt in `check-ui.baseline.json`; `format-money` removed (covered by `money-text`). The exception mark: `money: plain — <reason>`.
- Hovering any `<Money>` (desktop) shows a tooltip, a click opens the card; with `at` the tooltip and "Favourites" in the card use the rate on the transaction date. A tap — the card; in a button/link — a 500 ms hold; in a transaction row — the menu item "Amount in other currencies". From the keyboard — focus on the button with the amount.

## Done (stage 6¾, part 3)

- **One mechanism for phrases with amounts.** What remains is `src/shared/lib/money-parts.ts`: `composeSums()` gives `mark(amount | SumPart[])` (a marker for `t(…)` or a string) and `parts(text)` (words and amounts → `SumPart[]`), plus `isSum`, `partsText` (for `title`/`aria-label`). `fillPhrase` and `money-compose.ts` were removed, all consumers were migrated. A marker can be substituted into one text several times.
- **Shared rendering.** `<MoneyPhrase :parts>` in `entities/currency/ui/money-phrase` (exported from `index.ts`) replaced `pages/money/ui/SumParts.vue` and every `v-for` + `isSum` / `'text' in part`. For markup with one or two amounts in a template — `<i18n-t keypath>` with `<Money>` slots.
- **`bare` on `Money`**: a number without the currency sign, the tooltip and card as usual. "saved / goal" pairs ("36 of 50 $", "1 539 of 5 000 $") — the first amount is `bare` (`BudgetAmountStep`, `GoalWhereStep`, `PlanGoals`, `PlanLimits`, `SpendingMonths`, `MonthBalanceCard`, `SpendingCategories`, `GoalDetailsSheet`). The CSS hack `[&>span:nth-child(2)]:hidden` was removed.
- **`at` accepts a `YYYY-MM` month** (the first day of the month, `entities/currency/lib/money-moment.ts`; `Money` puts the already normalized value into `data-money-at`, `useConversionsAt` normalizes by itself). The manual `-01T00:00:00Z` was removed (`utility-item.ts`).
- **`SumPart`** gained `bare` and `convert` (`convert: false` — a number that is not an amount: a pair rate in `TxSimilar`). `FormField` gained a `hint` slot (a hint with an amount).
- **Gaps of the `money-text` rule.** Why it missed: (1) an amount inside `t('key', { … formatMoney(…) })` was cut out as a "phrase argument" (`withoutPhrases`) — `MoneyWhere`, `WhereAnswer`, `MonthDaysChart`, `UtilityBillCard:67`, `TxMore` and others slipped through it; (2) mustaches with two levels of curly braces (`{{ t('k', { a: formatMoney(x, { currency }) }) }}`) were not parsed by the regular expression (`UtilityBillCard`); (3) lists built by a helper (`v-for="row in rows"`, `UtilityItemSheet`, `SpendingCategories`) and (4) text props `:hint|:line|:description|:subtitle|:caption|:message|:summary|:detail` (`PriceAlertDialog`, `AutomationList`, `TxSimilar`) were not inspected; (5) `money-lib` checked only `lib/` and `model/`, not any `.ts` of a slice (`use-row-menu.ts`). Fixed: cutting out `t(…)` was removed, mustaches are parsed with balanced braces, a `v-for` over a helper with an amount and the extended attribute list are caught, `money-lib` looks at all `.ts` files of slices.
- The new findings (23 files) were moved to `<Money>` / `<MoneyPhrase>` / `<i18n-t>`; charts were left on `useMoneyChartTooltip`, axis labels are text.
- The remaining `formatMoney(` calls (excluding the mock): see the task report; all are either in the whitelist (`entities/currency`, `shared/lib`, the palette) or marked `money: plain — <reason>` (toasts, `aria-label`, string props of `SwitchRow`/`ActionSheet`/`FormField`, an exchange rate that is not an amount).
