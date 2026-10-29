# UI links: where each entity leads and how to get back

Stage 6½ "Link check". The goal: from any place, reach a related entity or page in one or two steps and get back.
The document is an inventory as of the `feature/ui-redesign` branch; the code was not changed. Lines are given as of the working copy.

Terms:
- **Glance** — a quick in-place overview (`Glance`: a popover on desktop, a sheet on a phone), a blue dashed outline on the canvas.
- **Link** — `EntityLink` (a click opens a card or a page, a right click opens an actions menu).
- **Page** — a named route.
- **Card** — a layer over the screen at the address `?open=kind:id~kind:id` (a stack, the top one is visible).
- **Text** — the name is shown as plain text, nothing is clickable.
- **Canvas** / **beyond the canvas** — whether it is on the `Full-*.dc.html` frame (Paths-2 · 8–11, the "Paths 1" rules in the redesign-vs-canvas log, which was removed from the repository and is kept in git history) or invented in code.

## 1. Mechanisms

| Mechanism | Files | What it does | Back |
|---|---|---|---|
| `entityLocation(target, query)` | `src/shared/lib/entity-link.ts` | For `store`, `workflow`, `scenario` it returns a page (`entityPage`): `wallet`, `workflow-edit`, `money-calculator?scenario=`. For the others it puts the target into `?open=` of the current address (a stack of up to 8, a repeat of the top one is not pushed) | Close/"Back" of the card: `router.replace` without the top one; the browser "Back" also pops it (opening is `push`) |
| `EntityLink` | `src/shared/ui/entity-link/EntityLink.vue` | A `RouterLink` to `entityLocation` + a kind icon + a right click (the `entityActions` menu + "Pin"). The only DOM marker: `data-entity="kind:id"` | see above |
| Card host | `src/pages/app-shell/ui/entity-host/*` | Renders the top card: `tx`, `goal`, `automation` have their own sheets, the other kinds use the shared `EntitySheet` (`get_entity_card`, `EntityRelations` relation groups) | The "← Back" button exists only in `EntitySheet` (`canGoBack`); the transaction, goal and automation sheets have only the cross/Esc, which also pops the top card |
| `entityActions` | `src/shared/lib/entity-actions.ts` | One table of kind actions: the card, the link menu (later the palette) | — |
| `openEntity()` | `entity-link.ts:94` | Open a card from code (the transaction row menu, `openTxDetails`) | — |
| `Glance` | `src/shared/ui/glance/Glance.vue`, `widgets/quick-glance` | Wallet (`WalletGlance`), capital (`CapitalGlance`), "Spent" (`SpentGlance`), "auto" (`AutomationGlance`); the category and wallet body in Ctrl K (`CategoryGlanceBody`, `WalletGlanceBody`). "Open →" leads to `openTo` | Esc, a click outside; on a phone "Back" closes the sheet. The page under the glance does not move |
| Transaction row menu | `features/transaction-row-menu/use-tx-row-menu.ts`, `shared/ui/action-menu` | "Quick glance", "Edit", "Repeat", "Similar", "Wallet X", "Contact Y", "Delete". Wallet and contact use `openEntity` | The card closes |
| `BackChip` | `src/shared/ui/back-chip`, `drill.ts` | "← Spending · October": `?back=<path>&backLabel=<label>` is rendered by `Transactions.vue:77`. Only on the transaction list | Esc and Alt ← |
| `drillTo()` | `src/shared/lib/drill.ts` | Transactions from a figure: period, category, wallets, kinds. The "back" chip is placed only by a call with `back:` | — |
| Settings window | `shared/lib/settings-link.ts`, `widgets/settings-shell` | `?settings=<group>&section=<section>`: a layer over any screen | Closing the window removes the keys from the address |
| Notifications | `shared/lib/notice-link.ts` | `link` from the core: a route name, `name:id`, a path, `open:kind:id` | — |
| Tray menu | `crates/app/src/tray.rs`, `use-shell-state.ts:92` | The `tray-navigate` event with a route name | — |

### Known `?open=` kinds

`store, tx, category, contact, goal, budget, automation, workflow, reminder, alert, asset, scenario` (`ENTITY_KINDS`). The host handles `?open=` once, in `AppShell`. The core (`relations.rs`) can produce a card for each; `automation` and `workflow` are looked up with one query (the id is shared). For `store`, `workflow`, `scenario` a card is never produced by `entityLocation` — only if the address is typed by hand.

Other address parameters that links lead to and that are handled: `?edit=<id>` (automations; `use-new-from-route.ts:31`), `?new=salary|blank` (automations), `?new=limit&category=` (`PlanLimits.vue:80`), `?category=` (Spending, `SpendingCategories.vue:78`), `?contact=` (`TransactionsContacts.vue:49`), `?scenario=` (`MoneyCalculator.vue:19`), `?tab=`/`?view=` (wallet, `use-wallet-view.ts:65`; wallet settings, `use-settings-groups.ts:57`), `?program=` (bonuses), `?store=,from=,to=,cat=,kind=,q=,...` (the transaction list, `tx-filter.ts`). All found `settings`/`section` values in the code exist in `SETTINGS_GROUPS`.

## 2. Table: the entity, where it is mentioned, how it is shown, where it leads, the way back

The "Canvas" column: **yes** — it is on the frame, **no** — beyond the canvas.

| Entity | Where mentioned | How shown | Where it leads | Way back | Canvas |
|---|---|---|---|---|---|
| **Transaction** | Lists (Transactions, Home "What changed", wallet, calendar, contact, category, exchange) | A row: click/Enter on a transparent `data-tx-open` button | The card `?open=tx:id` | Cross, Esc, browser Back | yes |
| | Goal/automation card, "Similar", palette, pins, notifications | `EntityLink`, `RouterLink` to `entityLocation`, `open:tx:` in a notice | Card | same | no |
| | Transaction rows in a category/wallet card (`relations`) | `EntityLink` | Card | the "← Back" button in `EntitySheet`, only if the stack is >1 | no |
| **Wallet / sub-account** | A transaction row (from → to) on desktop | The `WalletGlance` glance (slot `#side`: `Transactions.vue:128`, `HomeChanges.vue:71`, `EntityHost.vue:20`) | A popover, "Open →" to `/stores/:id` | Esc; from the page — browser back / sidebar | yes |
| | A transaction row on a phone | Text (deliberately: a 16 px touch target) | The transaction card, with a glance there | — | yes (noted in the redesign-vs-canvas log, line 219) |
| | A transaction row: contact / categories / lists without a `#side` slot | Text (see findings) | — | — | no |
| | The row menu "Wallet X" | A menu item, `openEntity` -> the `wallet` page (for `store` it is a page) | `/stores/:id` | Browser back | yes |
| | Sidebar, the "Wallets" table and list, `StoreRowLink`, `StoresView`, `AssetElsewhere`, `FixedCosts`, `DashboardAllocation` | A raw `RouterLink` to `wallet` (no `data-entity`) | `/stores/:id` | Browser back | no (only the sidebar and the table) |
| | `DebtsCard`, `FundCard`, `GoalList`, `GoalFacts`, `HealthLight`, goals in "Money" | `EntityLink` | `/stores/:id` | Browser back | no |
| | Capital on Home/Wallets | The `CapitalGlance` glance | `money-wealth` | Esc | yes |
| | The wallet page | `PageHeader` breadcrumbs **only if the wallet has a parent** (`WalletHeader.vue:74`) | "Wallets" -> parent | a root wallet has no breadcrumbs (finding 20) | no |
| | Wallet settings `/stores/:id/settings` | Breadcrumbs "Wallets › Name › Settings" | the wallet | breadcrumbs | no |
| **Category** | A transaction row (badge) | Text (finding 4) | — | — | no |
| | The transaction card | `EntityLink` | The `category` card -> "Transactions" (`cat=`), "Set a limit", "Limits" | The "← Back" button | no |
| | `SpentGlance` | A `data-glance-category` link to `drillTo(cat, back)` | Transactions with a "← <screen>" chip | `BackChip` | yes |
| | Ctrl K | The `CategoryGlanceBody` glance + Enter "Open in Spending" | `money?category=` (a category row in the current period, otherwise nothing will open) | Esc | yes |
| | The "Categories" report `CategoriesView.vue:61,103`, `LossSummary` (by wallet/contact) | `RouterLink` to `ofCategory` / the Transactions filter | Transactions | without a chip (finding 22) | no |
| | `MonthGrowth` | `EntityLink` | Card | "← Back" | no |
| | The overview "Top category", Home limits, the limit list | Text (findings 6, 7) | — | — | no |
| **Contact and debt** | A transaction row | Text (finding 4); in the menu "Contact Y" (`openEntity`) | The `contact` card -> "Transactions", "Contacts" | "← Back" | yes (menu) |
| | `TxFacts.vue:81` (the transaction card) | `EntityLink` | Card | | no |
| | The "Contacts" page | A list, a contact card on the right with the debt and actions (return, give, take) | `?contact=<id>` | On a phone "‹" (`TransactionsContacts.vue:93`) | yes |
| | Debt | Shown only in the contact card (`ContactDebtPhrase`, `DebtTermsPopover`); in transaction cards the "borrowed/returned" badge is text | — | — | no |
| **Goal** | Home (`DashboardGoals`), "Plan" (`PlanGoals`), `GoalList`, `GoalFacts` | `EntityLink` | The `goal` card (its own `GoalDetailsSheet` sheet) -> "Goals" (`money-plan`) | Cross, Esc | no |
| | The wallet card ("Goals" group) | `EntityLink` | Card | "← Back" | no |
| | The "New goal" action from the wallet card | A dialog, `store` selected | — | Cancel | no |
| **Limit** | `BudgetList`, `PlanLimits`, `DashboardBudgets` | The row and name are text (findings 6, 7); an `EntityLink` to `budget` exists only in the card relation groups | The `budget` card -> "Limits" | "← Back" | no |
| | `SpentGlance` "Set a limit" | A button | `money-plan?new=limit` | Browser back | yes |
| | "Set a limit for 'Groceries'" in Ctrl K | A palette action | `money-plan?new=limit&category=` | same | yes |
| **Automation and scheme** | The "auto" row in the transaction list | The `AutomationGlance` glance (slot `#origin`) | "Automation →" (the `automation` card) or "Scheme →" (the `workflow-edit` page) | Esc | yes |
| | The journal `JournalTable.vue:42` | A `RouterLink` to `entityLocation` if the entry opens, otherwise text | Card | Cross | yes (the "Automations" board) |
| | "Mine" (`MineTable`, `MineList`) | A row-link to `entityLocation` | Card | Cross | yes |
| | `WorkflowRuns.vue:51`, `SourceRow.vue:82` | `EntityLink` to `workflow` | `/automations/workflows/:id` | The "Automations ›" crumb in `EditorToolbar:64` always leads to "Mine", not to where you came from | no |
| | `WorkflowQueue`, `RunPanel` | The scheme name is text (finding 9) | — | — | no |
| | The automation card | Buttons "Edit" (`automations?edit=`) and "To the editor" (`workflow-edit`, the automation id = the scheme id) | | | yes |
| **Reminder** | `ReminderList`, the transaction window, "Tasks" on Home | A list and groups; `EntityLink` only in relation groups | The `reminder` card -> "Reminders" (a list without highlighting) | "← Back" | no |
| **Price alert** | `PriceAlertList` | The ticker is text (finding 10) | The `alert` card -> "Alerts" | | no |
| **Asset / currency** | A transaction row (`RUB → USDT`) | Text | — | — | no |
| | Home "Assets" (`DashboardAssets`), `AssetBaskets`, `SourceRow` | `EntityLink` `asset` | Card: "New alert", "Alerts" (there is no way out to the asset/wallet page with positions — finding 17) | "← Back" | no |
| | Wallet positions | A row opens the `AssetDetailsSheet` sheet (transactions with `q=`, `money-investments?store=`) | Links without a "back" chip | Cross | no |
| | Any `<Money>` amount | A click — `MoneyCard` (in the base currency and favorites), "All rates" -> `?settings=accounting&section=rates` | The settings window | Close the window | yes (Currency-1 · 5) |
| **Report and period** | The "Spent" figure, "Where it went", calendar, chart | `drillTo` -> Transactions; the chip "← Where it went", "← Spending · October" only where the caller passed `back` | Transactions | `BackChip` (finding 22) | yes |
| | Period | A period chip with a popover (`PeriodPicker`), `?from=&to=` | The same screen | Browser back | yes |
| | "Money" reports | `/money/*` tabs (`route-tabs`) | | Tabs | yes |
| | The "Inflation" window from a report | `?settings=accounting&section=inflation` (the "Accounting › Money" tab) | | Close the window | no |
| **Storage / device** | Settings: "Storage", "Device sync" (devices, history, transfer), "App" (`device-app`) | Sections of the settings window; other screens link only to `app/device-app` (`NotificationDetails.vue:124`) and `sync` (core notifications) | The settings window | Close the window | yes (Settings-*) |
| | Recent vaults on the sign-in screen (`RecentVaults`), `SyncDevices` (`peer.name`), `SendVaultPick` | Text/lists inside windows | — | — | yes |
| | The `device` and `vault` entities are not in `ENTITY_KINDS`: they have neither a card nor `data-entity` | | | | |

## 3. What is clickable and what is not: the list of "non-clickable" places

These are places where an entity name is shown but there is no `EntityLink`/glance (and the whole row is not a link to the same entity). List rows where the name is the heading of a row that leads to its own page are not included.

| # | Entity | File:line | Context |
|---|---|---|---|
| 1 (closed, stage 6½) | Contact | `entities/transaction/ui/transaction-row/TransactionRow.vue:196` | The second line: `· {{ contactName }}` |
| 2 (closed, stage 6½) | Category | `TransactionRow.vue:165` | The category badge |
| 3 (closed, stage 6½) | Asset | `TransactionRow.vue:152` | `RUB → USDT` |
| 4 | Wallet | `ContactDetails.vue:198`, `CategorySheet.vue:92`, `SpendingCalendar.vue:179`, `StoreTransactions.vue:71`, `ExchangeImported.vue:46`, `TransactionDetailsSheet` without a host | `TransactionsList`/`TransactionRow` without the `#side` and `#origin` slots: "from → to" and "auto" are text |
| 5 | Wallet | `entities/transaction/ui/tx-preview/TxPreviewCard.vue:69` | A preview in a form (acceptable: a form) |
| 6 | Category | `widgets/analytics-overview/AnalyticsOverview.vue:75` | "Top category" |
| 7 | Category, wallet | `widgets/dashboard-budgets/DashboardBudgets.vue:43`, `widgets/budget-list/BudgetList.vue:95` | The limit target: "Groceries · month" |
| 8 | Wallet (real estate) | `widgets/utilities-due/UtilitiesDue.vue:78` | The apartment name in "Utilities due soon" |
| 9 | Wallet | `widgets/cashback-bonuses/CashbackBonuses.vue:51,63` | The source and the program |
| 10 | Wallet | `widgets/bonus-programs/ui/ProgramList.vue:55`, `ProgramTable.vue:48` | The program name (a row selects the program in place, which is acceptable) |
| 11 (closed, stage 6½) | Wallet | `features/automation-simulate/AutomationSimulationDialog.vue:120` | Balances in the simulation (a dialog) |
| 12 | Scheme | `widgets/workflow-queue/WorkflowQueue.vue:48,76,97`, `widgets/workflow-editor/ui/RunPanel.vue:75`, `widgets/workflow-runs/WorkflowRuns.vue:41` | The scheme name as text (in `WorkflowRuns:51` there is an `EntityLink` to the same scheme next to it, a duplicate) |
| 13 (closed, stage 6½) | Alert, asset | `widgets/price-alert-list/PriceAlertList.vue:79` | The ticker in an alert row |
| 14 (closed, stage 6½) | Automation | `pages/automations/ui/journal/JournalTable.vue:47` (`opens()` is false), `JournalList.vue:26` | A journal entry name when the card is unavailable |
| 15 (closed, stage 6½) | Contact | `entities/contact/ui/contact-turnover/ContactTurnoverTable.vue:18` | An asset in the turnover (there are only assets there) |
| 16 (closed, stage 6½) | Asset | `pages/wallet/ui/wallet-assets/WalletAssets.vue:68,106`, `pages/stores/ui/WalletAssets.vue:34` | The ticker; the row meanwhile opens `AssetDetailsSheet` (acceptable, but without `data-entity`) |
| 17 (closed, stage 6½) | Wallet, contact | `widgets/loss-summary/LossSummary.vue:96` | A link to the journal with a `store=`/`contact=` filter, not to the entity |
| 18 (closed, stage 6½) | Wallet | `entities/store/ui/store-card/StoreCard.vue:84`, `StoreRowLink.vue:21`, `SidebarWalletItem.vue:43,72`, `WalletTable.vue:105`, `WalletList.vue:25`, `StoresView.vue:32`, `AssetElsewhere.vue:49`, `FixedCosts.vue:56`, `DashboardAllocation.vue:125` | Working links, but a raw `RouterLink`: no `data-entity`, no right click, no "Pin" |

Total: **13 entity kinds** in the table of section 2 (transaction, wallet/sub-account, category, contact and debt, goal, limit, automation, scheme, reminder, alert, asset/currency, report and period, storage/device); in code: `EntityLink` — 23 calls in 17 files, 4 glance kinds (wallet, capital, "Spent", "auto") on 8 pages, about 18 raw `RouterLink`s to entity pages (item 18). A name is shown as plain text in **11 places/groups** (items 1–4, 6–9, 12, 13, 14 of the table; items 5, 10, 11, 15, 16 are acceptable).

## 4. Findings

Type: **B** — broken (a button does not work), **T** — non-clickable text, **E** — an exit from a glance/card, **N** — no way back, **D** — a DOM proposal. The "Canvas" column: yes/no.

### Broken targets

| # | Type | Where | What is wrong | Proposal | Canvas |
|---|---|---|---|---|---|
| 1 | B | `crates/app/src/tray.rs:27,33`; handler `use-shell-state.ts:94` (`router.push({ name: target })`) | The tray items "Analytics" and "Planning" send `analytics-cashflow` and `planning-automations`. There are no such routes, `router.push` throws "No match for" | Replace with `money` and `automations` | no |
| 2 | B | `crates/core/src/service/notify_snapshot.rs:121,180`, `crates/app/src/scheduler.rs:623,645,673,781`, `crates/core/src/service/datasources.rs:535,563`, `crates/core/src/service/health.rs:425` (+ the test `health.rs:741`) | Core notifications lead to `planning-alerts`, `planning-reminders`, `planning-automations`, `planning-sources`, `analytics-health`. In `notice-link.ts:21` this becomes `{ name }` and the "Open" button in the bell (`NotificationBell.vue:53`) and in the toast (`use-backend-notices.ts:89`) fails. The demo data (`mock/demo.ts:256,266`) use paths, so the error is not visible in a browser with the mock | In the core: `automations-alerts`, `automations-reminders`, `automations`, `automations-sources`, `money-health`; fix the test. Or translate the name in `noticeRoute` (a table of old names) so as not to change saved notifications | no |
| 3 | B | `crates/core/src/service/inflation.rs:146` | The link `/?settings=inflation`: `inflation` is a section, not a group; `resolveSettingsTarget` will open the first group, "App". The correct one is `/?settings=accounting&section=inflation` (as `MoneyToolbar.vue:105` already calls it) | Fix the string (and, optionally, make `noticeRoute` accept `settings=<section>` as a section) | no |
| 3a (closed, stage 6½) | B (low) | `crates/core/src/model/fixtures.rs:470` | The test `/planning` is an old path; an unknown path leads to home (`router/index.ts`: `/:pathMatch(.*)*`) | Replace with `/automations` | no |

### Non-clickable mentions (make an EntityLink)

| # | Type | Where | What is wrong | Proposal | Canvas |
|---|---|---|---|---|---|
| 4 (closed, stage 6½) | T | `TransactionRow.vue:165,196,152` | The category, contact and asset in a transaction row are text; the menu has the wallet and contact, but no category or asset | Contact and category: move into a slot or `EntityLink plain` with `pointer-events-auto relative` (as for `#side`); menu: items "Category X", "Asset" via `openEntity`. The row already has a transparent overlay button, the touch targets on a phone are in the card | no (the canvas menu has "Similar", wallet, contact) |
| 5 (closed, stage 6½) | T | `ContactDetails.vue:198`, `CategorySheet.vue:92`, `SpendingCalendar.vue:179`, `StoreTransactions.vue:71`, `ExchangeImported.vue:46` | `TransactionsList` without the `#side`/`#origin` slots: "from → to" and "auto" are not clickable, although in the same rows on "Transactions" they are | One shared wrapper slot inside `TransactionsList` (`WalletGlance`/`AutomationGlance` by default), so that pages do not copy the same templates (it is in 3 places) | yes (the Paths-2 · 10 frame says "From: What changed, Transactions, wallet") |
| 6 (closed, stage 6½) | T | `AnalyticsOverview.vue:75`, `DashboardBudgets.vue:43`, `BudgetList.vue:95` | The category/target of a limit is text | `EntityLink` `category` (or `budget` for the whole limit row) | no |
| 7 (closed, stage 6½) | T | `UtilitiesDue.vue:78` | The apartment in "Utilities due soon" is text | `EntityLink` `store` | no |
| 8 (closed, stage 6½) | T | `CashbackBonuses.vue:51,63` | The source and the bonus program are text | `EntityLink` `store` (`row.store_id`) | no |
| 9 (closed, stage 6½) | T | `WorkflowQueue.vue:48,76,97`, `RunPanel.vue:75`, `WorkflowRuns.vue:41` | The scheme name is text, while `WorkflowRuns.vue:51` has a duplicate link | Replace the first `<span>` with `EntityLink workflow`, remove the duplicate | no |
| 10 (closed, stage 6½) | T | `PriceAlertList.vue:79` | The alert ticker is text | `EntityLink` `alert` or `asset` | no |
| 11 | T | `JournalTable.vue:47`, `JournalList.vue:26` | The entry name has no link when `opens()` is false | Keep it text; explain in a hint ("scheme deleted") | yes |
| 12 (closed, stage 6½) | T | `LossSummary.vue:96` | The wallet/contact name leads to the journal, not to the entity | Mark it as "journal for ..." or add an `EntityLink` next to it | no |

### Glances and cards without an exit

| # | Type | Where | What is wrong | Proposal | Canvas |
|---|---|---|---|---|---|
| 13 | E | `Glance.vue`, `quick-glance/ui/*` | The category glance (`CategoryGlanceBody`) and the wallet in Ctrl K have "Open →". `SpentGlance` and `AutomationGlance` have an exit. The **currency** glance (`MoneyCard`) has an exit only to "All rates". There is no glance for **contact, goal, limit** (noted in the redesign-vs-canvas log, line 251) | Leave it; an item for information | yes (noted) |
| 14 | E | `entity-actions.ts:71-73` (`contact`) | "Contacts" leads to `transactions-contacts` without `?contact=<id>`: the list opens with nothing selected | `to: (t) => ({ name: 'transactions-contacts', query: { contact: t.id } })` | no |
| 15 | E | `entity-actions.ts:75,85,102-105` (`goal`, `budget`, `reminder`, `alert`) | "Goals", "Limits", "Reminders", "Alerts" lead to a list without highlighting the needed record: from a limit or alert card the needed row has to be searched for | Add `?focus=<id>` to the lists, or open editing (as `automation` -> `?edit=`) | no |
| 16 | E | `entity-actions.ts:66-70` (`category`) | A category has no page; "Transactions" + "New limit" + "Limits". There is no "Open in Spending" (`money?category=`), although Ctrl K offers it | Add the action "Open in Spending" -> `{ name: 'money', query: { category: id } }` (works if the category is in the period) | yes (Paths-2 · 8, "Open in Spending") |
| 17 | E | `entity-actions.ts:106-109` (`asset`) | An asset from `EntityLink` has only "New alert" and "Alerts". There is no "Transactions with the asset" and "Where it is held" | An `operations` action for the asset (`q=<symbol>`), or "Positions" | no |
| 18 | E | `entity-actions.ts:65` (`tx`) | `tx: []` — the right-click menu on a transaction link is empty, only "Pin" remains | Add "Open wallet" / "Similar" (as in the row menu) | no |
| 19 | E | `AutomationList.vue:218`, `AutomationDetailsSheet.vue:48` | Not an error, but not obvious: `workflow-edit` receives the automation id. It works because the automation and scheme ids are shared (`relations.rs:227`) | A comment or `entityPage({kind:'workflow'})` | yes |

### No way back

| # | Type | Where | What is wrong | Proposal | Canvas |
|---|---|---|---|---|---|
| 20 (closed, stage 6½) | N | `pages/wallet/ui/wallet-header/WalletHeader.vue:74` | Breadcrumbs only for a sub-account (`ancestors.length`). A root wallet opens without "← Wallets" (the page has a button only in the "not found" state, `Wallet.vue:246`) | Always show `PageHeader` ("Wallets › Name") | yes (Wallets-2) |
| 21 (closed, stage 6½) | N | `widgets/workflow-editor/ui/EditorToolbar.vue:64` | "Automations ›" always leads to "Mine", even if you came from a wallet, from a scheme card or from Home. On a phone `MobileEditorBar.vue:72` already does `router.back()` | Also `router.back()` when `history.state.back` exists, otherwise "Mine" | no |
| 22 (closed, stage 6½) | N | `drillTo` calls without `back`: `CategorySheet.vue:67`, `MoneyWhere.vue:33`, `analytics-reports/lib/drill-down.ts:24`, `views/saldo.ts:19`, `BudgetList.vue:48`, `SpendingWeek.vue:21`, `SpendingMonth.vue:59` | You land in Transactions without a "← <where from>" chip. (With a chip: `SpentGlance`, `WhereTable.vue:37`, `SpendingIncomplete.vue:48`, `TxSimilar`, `GoalFacts`) | Pass `back: { to: route.fullPath, label }` in these 7 places. Better a single point: `drillTo` takes `back` by default from the current address | yes (Paths-1 · 5 "The way back" promises a chip for every "from a figure to a list") |
| 23 (closed, stage 6½) | N | `entity-actions.ts:38` (`transactions()`), `AssetDetailsSheet.vue:49` (`txLink`, `analyticsLink`), `HomeChanges.vue:39` ("All transactions"), `LossSummary.vue:96` | From a card/sheet to Transactions without a chip | The same `back` as in item 22 | no |
| 24 (closed, stage 6½) | N | The sheets `TransactionDetailsSheet`, `GoalDetailsSheet`, `AutomationDetailsSheet` inside a `?open=a~b` stack | No explicit "← Back" button, only the cross/Esc/browser "Back" (which pops the top card). `EntitySheet` has the button (`EntitySheet.vue:55`) | Move "← Back" into the shared sheet header, show it at `canGoBack` | no |
| 25 | N | The pages `automations-*`, `transactions-contacts` (desktop), `money-*` | Section roots, the way back is the tabs/sidebar; acceptable. A contact on a phone has "‹" | — | yes |

### Old paths and non-working parameters

| # | Type | Where | What is wrong | Proposal |
|---|---|---|---|---|
| 26 | — | Old paths `/analytics`, `/planning`, `/workflows` | Not used in the interface code (checked with `grep`), there are no redirects by the stage 1 decision: an unknown path -> `/` | Items 1–3, 3a are enough |
| 27 | — | `?settings=` | All values in the code (`accounting/rates|currencies (General), budget|inflation (Money)`, `app/device-app|notifications`, `sync`, palette groups) exist. The exception is item 3. The old `/settings` redirects to `/?settings=list` | — |
| 28 | — | `?open=` | Handled by a single host (`EntityHost`); all 12 kinds are covered. `entityLocation` for `store`/`workflow`/`scenario` bypasses `?open=` | — |

### DOM attributes

| # | Type | Where | What is wrong | Proposal |
|---|---|---|---|---|
| 29 (closed, stage 6½) | D | `EntityLink.vue:40` | There is only `data-entity="kind:id"` on `<a>`. For `store`/`workflow`/`scenario` the link leads to a page, for the others to `?open=`; a script cannot tell what to expect | Add in the same place `:data-entity-open="entityPage(target) ? 'page' : 'card'"` (or `data-entity-kind` + `data-entity-id` separately). One point |
| 30 (closed, stage 6½) | D | Raw `RouterLink`s to entity pages (item 18 of the table above) | No `data-entity`: a script does not see them | Switch to `EntityLink` (with `plain` and its own class) where it is a wallet/scheme name; keep the row-cards (sidebar, table), but add `data-entity` |
| 31 (closed, stage 6½) | D | `Glance.vue:58,86` | There are `data-glance-trigger="wallet|capital|spent|auto"`, `data-glance` (the body), `data-glance-open` ("Open →"). No entity id | Add an `entity?: EntityRef` prop and `:data-entity="..."` on the trigger (passed in `WalletGlance`, `AutomationGlance`) |
| 32 (closed, stage 6½) | D | The `EntitySheet.vue:44` card (`data-tour="entity-card"`) | No `data-entity-open="kind:id"` on the card itself: a script has nothing to check that the right one opened | Add it to `SheetContent`, and the same attribute for the tx/goal/automation sheets |

Closed in stage 6½ (part B1): 4 — the category and contact in a transaction row became `EntityLink` (without their own menu: the row menu now contains "Category 'X'"), 5 — `TransactionsList` takes the wallet glance and "auto" by default from `provideTxGlances` (`shared/lib/tx-glances.ts`, provided by `AppShell`), the wallet of the current page stays text; 6–10 — `EntityLink` in limits (`budgetEntity`), utilities, bonuses, schemes and alerts; 12 — the hint "Journal for …"; 30 — `data-entity` on all raw links to `/stores/` and `/automations/workflows/` (`pageEntityKey` for crumbs, reminders, tasks, upcoming). `check-links`: 230 -> 303 checked, 16 -> 0 warnings.

Closed in stage 6½ (part B2): 11 — the hint "Scheme deleted" on a name without a link (`JournalTable`; on a phone the row is not clickable anyway); 13 — the money card (`MoneyCardHost`) exit "All rates →" got `data-glance-open`, the other glances already had an exit; 14 — "Open in contacts" leads to `transactions-contacts?contact=<id>` (the page already understood it); 15 — `?focus=<id>` for goals, limits, reminders, alerts: `shared/lib/use-focus-row.ts` (scroll + highlight with the tokens `bg-accent ring-ring` for 2.2 s, then `focus` is removed from the address), rows carry `data-focus-row`, "Plan" expands the lists itself; 16 — "Open in Spending" (`spendingOfCategory`, shared with Ctrl K); 17 — "Transactions with the asset" (`?q=<ticker>`); "Where it is held" was not added: there is no cheap way without a new query; 18 — the transaction link menu: "Open wallet" and "Similar" (`similarLocation` in `shared/lib/transactions`, one builder with the row menu; the items exist when the transaction is known to the card/list: `txDetailsTx`); 20 — the crumb "Wallets › Name" on a root wallet, on a phone "‹" (`data-wallet-back`); 21 — `useCameFrom` (`history.state.back`): "Automations ›" and "‹" of the editor lead back, otherwise to "Mine", the label depends on the section you came from; 22–23 — `drillTo` without `back` takes "← <this screen>" from the shell (`registerDefaultBack` in `AppShell`, the label is the page title, `?open=` cards are not part of the path), `back: false` disables it; the same in card actions, `AssetDetailsSheet`, "All transactions" on Home, `LossSummary` (`hereBackQuery`); 24 — `EntityBack` ("← Back", `data-entity-back`) in the transaction, goal, automation sheets and the shared card, visible with a `?open=a~b` stack.

Closed in stage 6½ (part A): the core route names are collected in `crates/core/src/routes.rs` (in a debug build `notice()` checks the link), the comparison with the router is `tests/checks/routes.check.ts` (in `mise run check`); `reachable()` in `notice-link.ts` leads home if the route does not exist (bell, toast, tray). Along the way `workflows-journal` -> `automations-journal` was found and fixed. New attributes: `data-entity-open="page|card"` on `EntityLink`, `data-entity-card="kind:id"` on `?open=` sheets, `data-entity` on the `Glance` trigger (the `entity` prop).

## 5. DOM markers for `tests/e2e/links/links.e2e.ts`

What exists now:

| Attribute | Where | What it means |
|---|---|---|
| `data-entity-open="page\|card"` | `<a>` in `EntityLink.vue` | Where the link leads: a page or a card on top |
| `data-entity-card="kind:id"` | `SheetContent` of `?open=` cards | Which card is open |
| `data-entity="kind:id"` | `<a>` in `EntityLink.vue:40` | A link to an entity. Parsed at the first `:` (an asset id may contain colons) |
| `data-glance-trigger="wallet\|capital\|spent\|auto"` | the `Glance` trigger button | A glance |
| `data-glance`, `data-glance-open`, `data-glance-category` | the glance body, the "Open →" button, the category link in `SpentGlance` | The popover/sheet and exits from it |
| `data-back-chip` | `BackChip.vue` | "← <where from>" |
| `data-tx-open`, `data-tx-auto`, `data-tx-attachments` | a transaction row | The transparent open button, the "auto" slot, the attachment counter |
| `data-money-convertible`, `data-money-amount`, `data-money-currency`, `data-money-at`, `data-money-open` | `<Money>` | Amount/currency and the open card |
| `data-tour="entity-card"` / `"entity-actions"` | `EntitySheet` | The card and its action buttons |
| `data-widget-title`, `data-mine-row`, `data-journal-row` | widget titles, "Mine" and journal rows | — |

What is missing and worth adding **in one point — `EntityLink.vue`** (all entity links pass through it):

1. `data-entity-open="page|card"` — whether to wait for a route change or `?open=`.
2. Together with it (`data-entity` already exists) a script for every link: click, check the address (`/stores/:id`, `/automations/workflows/:id`, `/money/calculator?scenario=` or `?open=` ends with `kind:id`), make sure the card opened (`[data-entity-card="kind:id"]`, add it to `SheetContent` of all four sheets) without `entity.missing`, and that Esc/"← Back" returns the previous address.
3. For glances — `data-entity` on `Glance` through the `entity` prop.
4. For the "back" chip `data-back-chip` is enough: the check is that every `drillTo` from a "figure" to a list has the chip.
5. A rule for the script: any `RouterLink` whose `href` matches an entity route (`/stores/`, `/automations/workflows/`) must have `data-entity` (this catches item 18 of the table).

## 6. What is on the canvas, what is beyond the canvas

On the canvas (checked against `Full-Paths-2` and the redesign-vs-canvas log): a wallet glance in a transaction row (dashed), "auto" -> glance, the capital and "Spent" glances, the chip "← Spending · October"/"← Where it went", the row menu (glance, wallet, contact), Ctrl K with a category and wallet glance, amounts -> the currency card, "Set a limit". The canvas does not show: wallet names in the expanded "Where it went" without a dashed outline in the app (documented), links to a category, contact, goal, limit, scheme, asset, reminder, alert (the whole `EntityLink` and the cards — decision J1, **beyond the canvas**), nor the "back" chips from sheets, the root wallet crumbs, the return from the scheme editor.
