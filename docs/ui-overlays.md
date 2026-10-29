# Inventory of interface layers

An inventory of everything that pops up over a page: dialogs, sheets, popovers, menus, glances, Ctrl K, entity cards, tooltips. Compiled by reading the code of the `feature/ui-redesign` branch; sections 1–5 were compiled by reading the code; at stage 6¾ the automatic check `tests/e2e/layout/lib/overlays.mjs` was added (sections 6–7), and what it found was fixed. Line numbers are as of the working copy.

Why: the autotest (`tests/e2e/layout`) needs a list of "what to open, how, and what counts as open". There was no such list — see section 5.

## 1. General layer logic

| Question | How it works | Where |
|---|---|---|
| Dialog on a phone | `Dialog` under `useIsPhone()` (the `md` boundary) is rendered as a `Drawer` (a bottom sheet, swipe closes, a 40 px cross); on desktop — a centered `z-50` panel. `FormDialog` is the same `Dialog` with a form, Enter saves | `src/shared/ui/dialog/Dialog.vue:57,89`, `src/shared/ui/form-dialog/FormDialog.vue` |
| `Sheet` | Does not transform: on the right, on a phone `w-full` (the whole screen, no swipe). Entity cards, the bell and the variables list are built this way | `src/shared/ui/sheet/SheetContent.vue:44-47` |
| Drawer | Reka Drawer, from the bottom, `max-h` under the status bar; `overlay:false` — non-modal (`MobileInspector`) | `src/shared/ui/drawer/DrawerContent.vue` |
| Esc | All Reka-based layers (Dialog, Sheet, Drawer, Popover, menus, Tooltip) close with Esc by themselves. Ctrl K has its own `window keydown` listener (`CommandPalette.vue:60`). Glance: Esc + "Enter — open" (`Glance.vue:34`) | — |
| "Back" (Android, `onBackButton`) | `useBackButton` -> `closeTopLayer()`: first the layers from `pushBackLayer` (Ctrl K, the tour), otherwise it sends Esc to the top `[data-dismissable-layer]`; only then `router.back()`; on Home a second "Back" within 2 s minimizes | `src/shared/lib/back-stack.ts`, `src/pages/app-shell/model/use-back-button.ts` |
| A layer in the address | `?settings=<group>&section=` (closing is `router.replace` without the keys), `?open=kind:id~kind:id` (a stack of cards, up to 8; closing pops the top one) | `use-settings-nav.ts`, `entity-link.ts:40-77` |
| Toasts | `Sonner` in `App.vue:88`, outside `AppShell` (works on `/unlock` too); on a phone at the top center under `--safe-top`. There is no explicit z-index (taken from vue-sonner). A dialog, sheet and Drawer do not close on a click on a toast (`keepOpenOnToast`) | `src/shared/lib/toast-outside.ts` |
| z scale | `z-sticky` 10, `z-banner` 40, `z-overlay` 50 (`styles.css:366-376`). Dialog, Drawer, Sheet, Popover, Tooltip, Select — a bare `z-50`; Combobox — `z-[60]` (`Combobox.vue:165`). The order of layers over each other is decided by the order of portals in `body` | — |
| Action menus | `ContextMenu` (right click, Shift+F10; Reka adds long press) + the "⋯" `ActionMenu`: on desktop `DropdownMenu`, on a phone `ActionSheet` (a sheet) from the same `UiAction` list | `src/shared/ui/action-menu`, `src/shared/ui/context-menu` |
| Tooltips | `Tooltip` (300 ms delay, `App.vue:81`) — hover and focus only. Where a hint carries meaning, a popover is used on touch (`ReportTerm.vue:21`) | — |
| Keys | `Ctrl N` — quick add, `Ctrl K` — the palette, `Ctrl B` — the sidebar (desktop), `Alt 1…5` — sections, `Ctrl Enter` — "save and continue" (`src/shared/lib/hotkeys/hotkeys.ts:27-38`) | — |

The host of address layers (query parsing):

| Key | What it opens | Who parses it |
|---|---|---|
| `?open=kind:id~…` | an entity card. Kinds: `store, tx, category, contact, goal, budget, automation, workflow, reminder, alert, asset, scenario`; for `store/workflow/scenario` a page (`entityPage`) is used instead of a card | `EntityHost` in `AppShell`, `use-entity-host.ts`; `tx/goal/automation` have their own sheets, the rest use `EntitySheet` |
| `?settings=<group>[&section=]` | the settings window (`list` — the group list on a phone) | `SettingsShell`, `use-settings-nav.ts`; links — `settings-link.ts` |
| `?new=blank` | the automation wizard | `NewAutomation.vue:24` |
| `?new=reminder\|alert\|salary\|save_share` | a dialog on the automations page, the key is removed immediately | `NewAutomation.vue:39` |
| `?new=salary` | the "salary" preset in the automation list | `use-new-from-route.ts:14` |
| `?new=limit[&category=]` | the limit dialog on "Plan" | `PlanLimits.vue:80` |
| `?edit=<id>` | the automation form | `use-new-from-route.ts:31` |
| `?whatsNew=1` | "What's new" (the whole history) | `WhatsNewDialog.vue:18` |
| `?tour=<route>`, `?lesson=` | a tour (driver.js) | `use-shell-state.ts:40` |
| `?scenario=`, `?category=`, `?contact=`, `?tab=`, `?view=`, `?program=` | not layers: content selection (a category opens `CategorySheet`) | pages |

Address links are checked by `tests/e2e/links/links.e2e.ts` (cards, glances, the "← Back" stack); `?new=`, `?edit=`, `?whatsNew=` are not checked by it.

## 2. What `tests/e2e/layout` already checks

Scenarios are in `tests/e2e/layout/lib/scenario.mjs`, measurements in `probes.mjs`:

- `views(page, size)` (:114) — pages only: static routes, per wallet of each kind and its settings, every settings tab (`?settings=&section=`). Layers on a page are not opened; the settings window is measured as a page (overflow, shift, touch targets).
- `dialogJump` (:208) — once, 1280 mouse: on `/` a click on `[data-tour="quick-add"]`, five measurements of `[role="dialog"][data-state="open"]` over 0…~600 ms, the center does not move by more than 2 px, closing with Esc. Quick add only; not run on a phone.
- `backCloses` (:256) — once, 390: `closeTopLayer()` closes the settings window (`?settings=app`), quick add (`[data-tour="quick-add"]`), the palette (`[data-command-palette-trigger]`). "Open" is `[role="dialog"][data-state="open"], [data-command-palette][data-open="true"]`.
- `toastOverDialog` (:299) — once, 1280: a click on the toast's "Undo" over the settings window works and the window does not close.
- `confirmStable` (:283) — `ConfirmButton`: the button does not move or change size, the question (`data-overlay="confirm"`, a popover on desktop / a sheet on a phone) does not cover the button, Esc closes.
- `listenerLeak` — not layers.
- **Stage 6¾:** `overlays.mjs` (+ `overlay-list.mjs`, `overlay-steps.mjs`, `overlay-cross.mjs`) — every layer from the list is opened, measured and closed at 1280 and 390; it also runs in `--quick`. What is covered — section 7.

`tests/e2e/links` additionally opens: `?open=` cards (`[data-entity-card]`), glances (`[data-glance-trigger]` -> `[data-glance]`, the exit `[data-glance-open]` or `a[href]`), the card stack and `[data-entity-back]`, up to 6 glances per page.

Result: outside `dialogJump/backCloses/toastOverDialog` no dialog, sheet, popover or menu is opened by an autotest; on a phone only pages and three layers in `backCloses` are measured.

The "Check" column below: **yes** — the layer is opened by a script; **links** — only `check-links`; **page** — only the page under the layer is measured; **no**.

## 3. Inventory

Legend. "Selector": **shared** — only Reka's `[role="dialog"]` (`Dialog` on desktop and `Drawer` on a phone have their own `data-state`, no own `data-*`); **exists: …** — an existing one; **none** — a `data-overlay` is needed. "Prop." — the name for `data-overlay`. Entry "menu" = "⋯"/right click on a row. All dialogs on a phone are a bottom sheet (`Dialog` -> `Drawer`) unless noted otherwise. Paths are relative to `src/`.

### 3.1 Shell

| # | Layer (prop. `data-overlay`) | Component | Inside | Entries | Desktop / phone | Selector | Check |
|---|---|---|---|---|---|---|---|
| 1 | `quick-add` | `widgets/quick-add/.../QuickAddDialog.vue` (in `AppShell`, also in `Wallet.vue` for a trade) | transaction form, series, key hint | the header button and the bottom bar "+" (`data-tour="quick-add"`); `Ctrl N`; the tray event `quick-add`; Ctrl K "Add"; `openQuickAdd()` from glances/wallet/Home | desktop: a `size=form` panel; phone: a sheet, a full-width save button (`data-quick-add-submit`) | exists: `data-tour="quick-add"` (button), `data-quick-add-submit` | yes: dialogJump (desktop), backCloses (phone) |
| 2 | `settings` | `widgets/settings-shell/SettingsShell.vue` (Reka `DialogRoot`, not `Dialog`) | groups and tabs, sections, the `settings` tour | `?settings=`; the sidebar; the bottom bar "More"; Ctrl K "Settings"; the tray `settings`; "What's new"/a notification with a link | desktop: a 230x175 window; phone: the whole screen, group list -> group, "‹" back; "Back" goes group -> list -> closed | exists: `data-tour="settings-shell"`, `settings-groups` | yes: backCloses (phone), toastOverDialog, views (tabs as pages) |
| 3 | `palette` | `pages/app-shell/ui/command-palette/CommandPalette.vue` (its own layer, `Teleport`, not Reka) | search, actions, amount converter, a glance on the right (`PalettePeek`), key hints | `Ctrl K`; the header button `data-command-palette-trigger`; on a phone a magnifier icon | desktop: a 620/860 px window, a backdrop; phone: the whole screen with "Cancel" (`data-command-palette-cancel`); Esc — its own listener; "Back" — `pushBackLayer` | exists: `data-command-palette`, `data-open`, `-backdrop`, `-trigger`, `-cancel` | yes: backCloses (phone) |
| 4 | `converter` | `features/currency-convert/ConverterDialog.vue` (in `AppShell`) | currency converter | Ctrl K "Converter" and `100 usd`; the wallet gear menu; `openConverter()` | a sheet on a phone | shared | no |
| 5 | `whats-new` | `widgets/whats-new/WhatsNewDialog.vue` | "What's new" entries | by itself when there are unread ones; `?whatsNew=1`; Ctrl K "What's new"; Settings › Help | sheet | shared | no |
| 6 | `sync-pairing`, `sync-plan` | `features/vault-sync/ui/SyncPairingDialog`, `SyncPlanDialog` | pairing; the merge plan | backend sync events (`useSyncEvents`) | sheet | shared | no |
| 7 | `enrichment-consent` | `features/enrichment-consent/EnrichmentConsentDialog.vue` | consent to enrichment | the flag `settingsStore.enrichmentConsentOpen` | sheet | shared | no |
| 8 | `notifications` | `widgets/notification-bell/NotificationBell.vue` | a `Sheet` on the right: the notification list | the bell in the header (`data-tour="notification-bell"`) | phone: the whole screen | exists: `data-tour="notification-bell"` (button) | no |
| 9 | `bottom-more` | `pages/app-shell/ui/app-bottom-nav/AppBottomNav.vue` | a `Drawer`: other sections, settings, lock | "More" (`data-bottom-more`) | phone only | exists: `data-bottom-more-sheet` | no |
| 10 | `entity-tx` | `widgets/transaction-details/TransactionDetailsSheet.vue` (via `EntityHost`) | the transaction card, relations, actions (`TxMore`) | `?open=tx:id`; click/Enter on a row (`data-tx-open`); the row menu "Quick glance"; Ctrl K; the notification `open:tx:` | phone: the whole screen; a tap on the wallet — text | exists: `data-tx-card`, `-actions`, `data-entity-card` | links |
| 11 | `entity-goal` | `widgets/goal-details/GoalDetailsSheet.vue` | goal, progress, actions, deletion (`GoalDelete` — a dialog inside) | `?open=goal:id`; `EntityLink` | as above | exists: `data-goal-card`, `-actions`, `data-entity-card` | links |
| 12 | `entity-automation` | `widgets/automation-details/AutomationDetailsSheet.vue` | automation, "Edit", "To the editor" | `?open=automation:id`; the "auto" glance; the journal | as above | exists: `data-automation-card`, `-actions`, `data-entity-card` | links |
| 13 | `entity-card` | `widgets/entity-sheet/EntitySheet.vue` | the shared card of category, contact, limit, reminder, alert, asset, relation groups, "← Back" (`data-entity-back`) | `?open=` for the other kinds; `EntityLink` | as above | exists: `data-entity-card="kind:id"`, `data-tour="entity-card"` | links |
| 14 | `entity-dialogs` | `EntityHost.vue`: `GoalDialog`, `BudgetDialog`, `PriceAlertDialog` | forms requested by a card action (`entityDialog`) | an action in the card / the `EntityLink` menu | sheet; live next to the card | shared | no |
| 15 | `money-card` | `entities/currency/ui/money-card-host/MoneyCardHost.vue` | the amount in the base currency at the transaction's rate and today's, favorites; "All rates" (to Settings) | a click (tap) on a `<Money>` amount outside a button/link; `C` copies | desktop: a popover at the amount; phone: a `Drawer` | exists: `data-money-card-host`, `-rates`, on the amount `data-money-open` | no |
| 16 | `money-tooltip` | `entities/currency/ui/money-tooltip-host/MoneyTooltipHost.vue` | conversions | a 300 ms hover on an amount inside a button/link | desktop; not shown on touch (`pointerType==='touch'`) | none | no |
| 17 | `tx-row-menu` | `entities/transaction/ui/transactions-list/TransactionsList.vue` + `features/transaction-row-menu/TxRowMenuHost.vue` | the row menu; "Edit" (`TransactionEditDialog`), "Repeat" (`TransactionCreateDialog`), delete confirmation (`Dialog size=sm`) | right click (on desktop); the "⋯" `ActionMenu` in a row | desktop: a context menu + "⋯"; phone: the context menu is disabled (`:disabled="coarse"`), "⋯" opens an `ActionSheet` | exists: `data-context-menu` (the desktop menu); dialogs — shared | no |
| 18 | `sandbox-reset` | `pages/app-shell/ui/sandbox-bar/SandboxResetDialog.vue` | sandbox reset | a button in `SandboxBar` | sheet | shared | no |
| 19 | `tour` | `shared/lib/tour/use-tour.ts` (driver.js) | the tour popup over a page | "?" in the header; the first visit; `?tour=` | "Back" closes (`pushBackLayer`) | exists: `.driver-popover` | no |
| 20 | `toast` | `App.vue:88` + `shared/ui/sonner` | toasts with "Undo" | `notify.*` | desktop: bottom right; phone: top center | exists: `data-sonner-toast` | yes: toastOverDialog (desktop) |
| 21 | `entity-link-menu` | `shared/ui/entity-link/EntityLink.vue` | the kind actions menu + "Pin" | right click/Shift+F10 on a link | no actions on touch, they are in the card | exists: `data-context-menu` | no |
| 22 | `glance-wallet` | `widgets/quick-glance/ui/WalletGlance.vue` | balances, conversion, transfer | a click on the wallet name in a transaction row (the `#side` slot of lists) and in transaction/goal cards | desktop: a 360 px `Popover`; phone: a `Drawer`; but in transaction rows on a phone the wallet is text | exists: `data-glance-trigger`, `data-glance`, `-open`, `-convert`, `-transfer` | links |
| 23 | `glance-automation` | `.../AutomationGlance.vue` | amount, who, journal; `AutomationAmountDialog` | a click on "auto" in a transaction row | as above | exists: `data-glance-auto-*` | links |

### 3.2 Home

| # | Layer | Component | Inside | Entries | Desktop / phone | Selector | Check |
|---|---|---|---|---|---|---|---|
| 24 | `glance-capital` | `widgets/quick-glance/ui/CapitalGlance.vue` (`HomeFigures.vue:70`, `Stores.vue:87`) | wallets and "why" | a click on the capital figure | popover / Drawer | exists: `data-glance-wallets`, `-why` | links |
| 25 | `glance-spent` | `.../SpentGlance.vue` (`HomeFigures.vue:94`) | categories, "Set a limit" (`?new=limit`) | a click on "Spent" | popover / Drawer | exists: `data-glance-category`, `-limit` | links |
| 26 | `dashboard-catalog` | `pages/dashboard/ui/dashboard-catalog/DashboardCatalog.vue` | the widget catalog | the Home settings button | sheet | shared | no |
| 27 | `widget-menu` | `shared/ui/widget-frame/WidgetFrame.vue` | the widget menu (hide, pin) | right click + "⋯" (`ActionMenu`) | desktop / `ActionSheet` | exists: `data-context-menu` (desktop) | no |
| 28 | `home-store-create` | `StoreCreateDialog` from `HomeFirstSteps`/`OnboardingChecklist` | a new wallet | the first-step button | sheet | shared | no |
| 29 | `tx-row-menu` (Home) | `TxRowMenuHost` in `HomeChanges.vue` | see #17 | see #17 | see #17 | see #17 | no |

### 3.3 Transactions

| # | Layer | Component | Inside | Entries | Desktop / phone | Selector | Check |
|---|---|---|---|---|---|---|---|
| 30 | `tx-create` | `features/transaction-create/TransactionCreateDialog.vue` | the full transaction form | a button on a wallet, in a goal/utilities card; "Repeat"; a plan from the calculator | sheet | shared | no |
| 31 | `tx-edit` | `features/transaction-edit/TransactionEditDialog.vue` | editing a transaction | "Edit" in the row menu and the card | sheet | shared | no |
| 32 | `tx-link` | `features/transaction-link/TransactionLinkDialog.vue` | link transactions | `TxMore` in the transaction card | sheet | shared | no |
| 33 | `tx-save-filter` | `features/transaction-filter/SaveFilterButton.vue` | the filter name | a button in Transactions (`data-tour="tx-save-filter"`) | sheet | exists: `data-tour` (button) | no |
| 34 | `tx-bulk-pick` | `features/transaction-bulk/.../BulkPickDialog.vue` | picking a category/wallet for the selected | the bulk actions bar | sheet | shared | no |
| 35 | `tx-export` | `features/report-export/TransactionsExportButton.vue` | a `DropdownMenu`: formats | a button in Transactions | a menu on click | none | no |
| 36 | `contact-menu` | `pages/transactions/ui/contact-list/ContactList.vue` | contact actions | right click/long press on a row; the contact screen has "⋯" (`ContactDetails.vue`) | desktop: a list on the left; phone: list -> contact screen | exists: `data-context-menu` | no |
| 37 | `contact-edit` | `features/contact-edit/ui/ContactEditDialog.vue`, `ContactAccountRow.vue` (menu+`ActionSheet`) | the contact form; accounts | buttons in Contacts | sheet | shared | no |
| 38 | `debt-terms` | `features/debt-manage/ui/DebtTermsPopover.vue` | debt terms | a click on the debt phrase | `Popover` on both | none | no |
| 39 | `bonus-program` | `features/loyalty-edit/PointsDialog.vue`, `ui/ProgramCreateDialog.vue` (`data-tour="bonuses-wizard"`), `ProgramTable.vue` (`ActionMenu`) | points, a new program | buttons on "Bonuses" | sheet | exists: `data-tour="bonuses-wizard"` (form) | no |
| 40 | `calendar-row-menu` | `widgets/spending-calendar` + `TxRowMenuHost` | see #17 | a calendar day | — | see #17 | no |
| 41 | `store-multiselect` | `entities/store/ui/store-node-select/StoreNodeMultiSelect.vue` | the wallet tree (checkboxes) | a filter field | a `Popover` on click | none | no |

### 3.4 Wallets

| # | Layer | Component | Inside | Entries | Desktop / phone | Selector | Check |
|---|---|---|---|---|---|---|---|
| 42 | `store-create` | `features/store-create/StoreCreateDialog.vue` | a new wallet / sub-wallet | "Add" in the sidebar and on "Wallets"; Home; a sub-wallet in a wallet | sheet | shared | no |
| 43 | `store-delete` | a `Dialog` in `pages/stores/Stores.vue:140` | delete confirmation | the card/row menu | sheet | shared | no |
| 44 | `store-menu` | `entities/store/ui/store-card/StoreCard.vue`, `pages/stores/ui/WalletTable.vue` | wallet actions | right click + "⋯" | desktop / `ActionSheet` | exists: `data-context-menu` (desktop), `data-tour="stores-list"` | no |
| 45 | `sidebar-wallet-menu` | `widgets/sidebar-wallets/SidebarWallets.vue`, `ui/sidebar-wallet-item/SidebarWalletItem.vue` | group and wallet actions | right click | desktop only (the sidebar is hidden on a phone) | exists: `data-context-menu` | no |
| 46 | `sidebar-pinned-menu` | `widgets/sidebar-pinned/SidebarPinned.vue` | unpin etc. | right click | desktop only | exists: `data-context-menu` | no |
| 47 | `wallet-gear` | `pages/wallet/ui/wallet-gear-menu/WalletGearMenu.vue` | a `DropdownMenu`: settings, converter… | the gear (`data-tour="wallet-gear"`) | a click menu, one for both | exists: `data-tour` | no |
| 48 | `wallet-dialogs` | `pages/wallet/Wallet.vue:226-242`: `AdjustmentDialog`, `StoreSettingsDialog`, `StoreChangeDialog`, `PropertySellDialog`, `PlanCalculatorDialog`, `GoalDialog`, `AutomationDialog` (`StoreCreate` and `TransactionCreate` see #42, #30) | wallet actions | buttons/the gear menu on the wallet page | sheet; "one dialog at a time, not inside another" (a comment in `Wallet.vue:96`) | shared | no |
| 49 | `asset-details` | `widgets/asset-details/AssetDetailsSheet.vue` (+`AssetRevalueButton` `FormDialog`, `PriceAlertDialog`) | asset: position, revaluation, alert | a position row in a wallet; `?open=asset:` goes to `EntitySheet` | a `Sheet` on the right / a screen | none | no |
| 50 | `utility` | `pages/wallet/ui/utility-item/UtilityItemSheet.vue`, `UtilityDialog`, `UtilityBillDialog`, `UtilityRemindersButton` (`data-tour="utility-reminders"`) | utilities: bill, readings, reminders | the "Utilities" tab | a `Sheet` + sheets | exists: `data-tour` (button) | no |
| 51 | `vehicle` | `widgets/vehicle-card`: `MileageDialog`, `ServiceDoneDialog` | mileage, service | the vehicle card | sheet | shared | no |
| 52 | `property-value` | `features/property-revalue/RevalueRuleDialog.vue` | a revaluation rule | the real estate card | sheet | shared | no |
| 53 | `asset-class` | `features/asset-class-edit/ui/AssetClassDialog.vue` | an asset class | `TopupAdvisor` | sheet | shared | no |
| 54 | `attachments` | `entities/attachment/ui/attachment-add/AttachmentAdd.vue` (`Popover`/`Drawer` `data-attachment-sheet`, `DropdownMenuRoot`), `AttachmentItem.vue` (`ActionMenu`) | add an attachment | the attachments field | desktop: a menu; phone: a sheet | exists: `data-attachment-sheet` | no |
| 55 | `exchange-menu` | `widgets/exchange-terminal/ExchangeTerminal.vue` | pair actions, `PriceAlertDialog` | right click + "⋯" | desktop / `ActionSheet` | exists: `data-context-menu` | no |
| 56 | `chart-menu` | `widgets/trading-chart/TradingChart.vue:166` | "Alert at price", drawing tools | right click on the chart | right click only: does not open on touch (an alert at the last price and the tools are on the `ChartToolbar` panel) | exists: `data-context-menu` | no |
| 57 | `chart-tooltips` | `ChartToolbar.vue` (5 `Tooltip`s), echarts tooltips in widgets | button labels; values | hover/focus; buttons have `aria-label` | echarts — tap | none | no |
| 58 | `wallet-settings-dialogs` | `StoreChangeDialog`, `StoreDangerZone` on `/stores/:id/settings` | changing the type, deletion | page buttons | sheet | shared | page |

### 3.5 Money

| # | Layer | Component | Inside | Entries | Desktop / phone | Selector | Check |
|---|---|---|---|---|---|---|---|
| 59 | `category-sheet` | `pages/money/ui/category-sheet/CategorySheet.vue` | a category: transactions, limit | `?category=`; a click on a category in "Spending" | a `Sheet`, on a phone a screen | exists: `data-tour="category-sheet"`, `data-category-sheet-limit` | no |
| 60 | `budget` | `features/budget-edit/BudgetDialog.vue` | a limit | "Limit" on "Plan" (`data-plan-limit`), `?new=limit`, Ctrl K, the glance "Set a limit", `SpendingCategories` | sheet | shared | no |
| 61 | `goal` | `features/goal-edit/GoalDialog.vue` | a goal | "Plan", "Where the money is", the calculator, a wallet, a wallet card | sheet | shared | no |
| 62 | `goal-delete` | `features/goal-delete/ui/GoalDelete.vue` | confirmation, inside `GoalDetailsSheet` | the goal card | a dialog over a sheet | shared | no |
| 63 | `plan-calculator` | `features/plan-calculator/PlanCalculatorDialog.vue` | the plan calculator | "Investments", a wallet | sheet | shared | no |
| 64 | `trade-record` | `features/trade-record/TradeRecordDialog.vue` | recording a trade | "Investments" | sheet | shared | no |
| 65 | `emergency-fund` | `features/emergency-fund-edit/EmergencyFundDialog.vue` | the cushion | `FundCard`, "Health" | sheet | shared | no |
| 66 | `money-wallets-filter` | `pages/money/ui/money-toolbar/MoneyToolbar.vue:79` | a `Popover`: wallet selection; next to it the period and export (`data-tour="period-picker"`, `report-export`, `report-filter`) | a toolbar button | a `Popover` on both (w ≤ 18 rem) | exists: `data-tour` (buttons) | no |
| 67 | `report-export` | `features/report-export/ReportExportButton.vue` | a `DropdownMenu` | a button | menu | none | no |
| 68 | `calc-compare` | `widgets/invest-result/ui/CompareMenu.vue` | a `Popover`: what to compare | a calculator button | popover | exists: `data-calc-compare-menu` | no |
| 69 | `calc-scenarios` | `features/calc-scenario-manage/ScenarioBar.vue` (`ActionMenu`) + `ui/ScenarioNameDialog.vue` | scenarios | "⋯" (`data-tour="calc-scenarios"`) | desktop: a menu; phone: an `ActionSheet` | exists: `data-tour` | no |
| 70 | `report-term` | `entities/report/ui/report-term/ReportTerm.vue` | a term explanation | hover/focus; on touch a tap | desktop: a `Tooltip`; phone: a `Popover` (`data-touch-inline`) | none | no |

### 3.6 Automations

| # | Layer | Component | Inside | Entries | Desktop / phone | Selector | Check |
|---|---|---|---|---|---|---|---|
| 71 | `automation-wizard` | `features/automation-edit/ui/automation-wizard/AutomationWizard.vue` | the wizard (the transaction step is a `TransactionCreate` form) | `?new=blank`; "Create" (`data-tour="automations-create"`) | sheet | exists: `data-tour` (button) | no |
| 72 | `automation-forms` | `SalaryDialog` (`data-tour="salary-form"`), `SaveShareDialog` (`save-share-form`), `ReminderDialog`, `PriceAlertDialog` from `NewAutomation.vue` | ready-made forms | `?new=salary\|reminder\|alert\|save_share` | sheet | exists: `data-tour` (forms) | no |
| 73 | `automation-edit` | `features/automation-edit/AutomationDialog.vue` | the automation form | `?edit=<id>`; the list; a wallet | sheet | shared | no |
| 74 | `automation-simulate` | `features/automation-simulate/AutomationSimulationDialog.vue` | a dry run | the automation list | sheet | shared | no |
| 75 | `automation-confirm` | `features/workflow-manage/.../AutomationConfirmDialog.vue` | confirmation from the journal | `AutomationsJournal` | sheet | shared | no |
| 76 | `workflow-import` | `WorkflowImportDialog.vue` | scheme import | a button (`data-tour="automations-import"`) | sheet | exists: `data-tour` (button) | no |
| 77 | `workflow-vars` | a `Sheet` in `pages/automations/AutomationsSection.vue:75` + `WorkflowVarDialog` | variables | a button (`data-tour="workflow-vars-open"`) | a `Sheet`, on a phone a screen | exists: `data-tour` (button) | no |
| 78 | `template-dialog` | `features/workflow-template-gallery/ui/TemplateDialog.vue` | a template | the gallery | sheet | shared | no |
| 79 | `source` | `features/data-source-edit/ui/SourceDialog.vue` (`data-tour="source-url"`, `source-test`), `SourceRow.vue` (`ActionMenu`) | a data source | "Sources" | sheet / `ActionSheet` | exists: `data-tour` (fields) | no |
| 80 | `reminder` | `features/reminder-edit/ReminderDialog.vue` (`data-tour="reminder-repeat"`), `widgets/reminder-list/ui/ReminderRow.vue` (`DropdownMenu`) | a reminder | `?new=reminder`, the list | sheet / menu | exists: `data-tour` | `confirm` (list) and confirmStable (`ConfirmButton` only) |
| 81 | `price-alert` | `features/price-alert-edit/PriceAlertDialog.vue` | a price alert | `?new=alert`, the list, the terminal, an asset/`alert` card | sheet | shared | no |
| 82 | `workflow-leave` | a `Dialog` in `pages/automations/WorkflowEdit.vue:96` | "Leave without saving" | leaving the editor page (a route guard) | sheet | shared | no |
| 83 | `workflow-toolbar-menus` | `widgets/workflow-editor/ui/EditorToolbar.vue`, `RunFromMenu.vue` (`DropdownMenu`) | commands, "Run from…" | toolbar buttons (`data-tour="workflow-edit-commands"`, `workflow-run`) | desktop | exists: `data-tour` | no |
| 84 | `workflow-mobile-bar` | `widgets/workflow-editor/ui/MobileEditorBar.vue` (`DropdownMenu` + a rename `Dialog`) | commands, name | the editor bar on a phone | phone | exists: `data-tour` (buttons) | no |
| 85 | `workflow-inspector` | `widgets/workflow-editor/ui/MobileInspector.vue` | a non-modal `Drawer` with snap points: node properties | a tap on a node | phone | none | no |
| 86 | `workflow-add-node` | `widgets/workflow-editor/ui/QuickAddHost.vue` | a `Drawer` (phone) / a panel (desktop): add a node | the "+" button / a double tap on the canvas | both | none | no |

### 3.7 Settings (inside the `?settings=` window)

| # | Layer | Component | Inside | Entries | Desktop / phone | Selector | Check |
|---|---|---|---|---|---|---|---|
| 87 | `password-set` / `-change` / `-remove` | `features/vault-password/ui/SetPasswordDialog`, `ChangePasswordDialog`, `RemovePasswordDialog` | the vault password | Settings › Vault | a dialog over a full-screen window | shared | no |
| 88 | `backup-restore` | `features/vault-backups/ui/RestoreBackupDialog.vue` | restoring a copy | Settings › Vault | sheet | shared | no |
| 89 | `vault-send` / `vault-receive` | `features/vault-transfer/ui/SendVaultDialog`, `ReceiveVaultDialog` | vault transfer | Settings › Sync; `Unlock` | sheet | shared | no |
| 90 | `base-currency` | `features/base-currency-change/.../BaseCurrencyChangeDialog.vue` | changing the base currency | Settings › Accounting | sheet | shared | no |
| 91 | `category-manager` | `widgets/category-manager/ui/CategoryAddDialog.vue`, `CategoryEditDialog.vue` | a category | Settings › Categories | sheet | shared | no |
| 92 | `enrichment-settings` | `widgets/enrichment-settings/EnrichmentSettings.vue` | enrichment settings | Settings | sheet | shared | no |
| 93 | `dev-tools` | `widgets/vault-dev-tools/VaultDevTools.vue` | developer (dev only) | Settings › Developer | sheet | shared | no |
| 94 | `hotkey-settings` | `HotkeySettings.vue` (not a layer, but describes the keys) | — | — | — | — | — |

Row 94 is a reference, not counted as a layer.

### 3.8 Sign-in

| # | Layer | Component | Inside | Entries | Desktop / phone | Selector | Check |
|---|---|---|---|---|---|---|---|
| 95 | `vault-unlock` | `features/vault-unlock/VaultUnlockDialog.vue` | the password | a vault card on `/unlock` (`data-unlock-card`) | sheet; the password has `autofocus` (the keyboard at once) | exists: `data-unlock-card` (card) | no (the `/unlock` screen is not part of `views`) |
| 96 | `vault-create` | `features/vault-create/VaultCreateDialog.vue` | a new vault | `Unlock` | sheet | shared | no |
| 97 | `vault-delete` | `features/vault-delete/VaultDeleteDialog.vue` (`size=confirm`) | forget/delete | `RecentVaults` | sheet | shared | no |
| 98 | `recent-vaults` | `widgets/recent-vaults/RecentVaults.vue` | the recents list; "Other" (`data-recents-other`) | the list button (`data-tour="unlock-recents"`) | desktop: a `Popover`; phone: a `Drawer` (split by `isPhone`) | exists: `data-recents-other`, `data-tour` | no |
| 99 | `app-settings-unlock` | `widgets/app-settings/ui/AppSettingsDialog.vue` | settings before sign-in | `Unlock` | sheet | shared | no |

## 4. Suspicious

Noted by reading the code; items marked "probe" need checking in a browser.

1. **Amounts in buttons and links without phone and keyboard support.** `MoneyTooltipHost` reacts only to a mouse `pointerover` and skips touch (`MoneyTooltipHost.vue:39`); the amount card (`MoneyCardHost`) opens only for amounts outside a button/link (`money-target.ts:isPressableSum`). Result: an amount inside a button (a link-table row, a wallet in a list) has no conversions at all on a phone and from the keyboard (rule 16: "no action only on hover").
2. **Ctrl K and Ctrl N over an open modal dialog. FIXED at 6¾** (global `useHotkey` keys wait while a modal layer `[role="dialog"][data-state="open"]` is open; the check `palette-over-dialog`; before: the palette opened under the dialog, clicks and focus did not reach it). The hotkeys are global (`useHotkey`), the palette is teleported into `body` outside the Reka layer; an open `Dialog` sets `pointer-events:none` on `body` and holds focus — clicks on the palette may not get through. The reverse case: `Ctrl N` with a dialog open opens a second one. `backCloses` has no "dialog + palette" order.
3. **Menus without touch.** (a) `TradingChart` (`TradingChart.vue:166`): "Alert at cursor price" and "Draw" — by right click only; the panel has an alert at the last price and a tool choice, but an alert at an arbitrary price is unavailable on a phone. (b) Pinned items (`SidebarPinned`) are visible only in the sidebar, which a phone does not have: you can pin (row menu, widget), but not see or unpin. (c) The wallet menu in the sidebar (`SidebarWalletItem`, `SidebarWallets`) is right click only, on a phone the same actions are in `WalletTable` ("⋯"), but not for groups. (d) `ContactList` — right click and Reka's long press, no visible button in a row (there is "⋯" on the contact screen). (e) `EntityLink`: right click, on touch the actions are in the card (by design).
4. **Toasts and layers that do not know about them. PARTLY FIXED at 6¾** (`PopoverContent` now knows about toasts — a click on "Undo" does not close a glance, `toast-over-glance`; on a phone the toast is lowered under the header, `PHONE_TOAST_TOP` in `App.vue`, the sheet "×" and the settings "‹" are open, `toast-over-sheet`; what remains are `DropdownMenu` menus and the palette, whose `onClickOutside` closes it on a click on "Undo"). `keepOpenOnToast` is wired only to `Dialog`, `Sheet`, `Drawer`, `SettingsShell`. They do not know: `Popover` (glances, the amount card), menus, the palette (`onClickOutside` closes it on a click on "Undo"). On a phone the toast at the top center lies over the header of full-screen sheets and the settings window (`pt-safe`) while shown and covers "‹"/"×". `toastOverDialog` checks only the settings window at 1280 px, not a sheet and not a phone.
5. **A layer scale by portal order.** Dialog, Drawer, Sheet, Popover, Tooltip, Select have the same `z-50`; Combobox — `z-[60]` (in `check-ui` the `z-*` scale, rule 17). Popovers inside a dialog (`AssetChip` in the `TxQuick*` transaction form, `DateTimePicker`, `Select`, `Combobox`) work only while the portal is created later than the dialog; on a phone they are inside a `Drawer` with swipe-to-close — a gesture conflict (probe).
6. **Nested layers. CHECKED at 6¾ for a dialog over a sheet** (`dialog-in-sheet`: a deletion question over a goal card, the first Esc/"Back" closes the question, the second the card; wait for the animation to end). A glance `Popover` -> dialog is still not checked. A dialog from a sheet (`GoalDelete` in `GoalDetailsSheet`, `TransactionCreate` from `TransactionDetailsSheet`, `AutomationAmountDialog` from a glance). A glance `Popover` opens a dialog: closing the glance before the dialog is not checked (probe). `EntityHost` keeps `GoalDialog/BudgetDialog/PriceAlertDialog` mounted together with the card.
7. **The tour and dialogs.** The tour autostart is skipped if a `[role="dialog"]` is open (`use-shell-state.ts:45`); the palette (`role="dialog"` without `data-state`) and the tour over it together are not checked (probe).
8. **Layers without a selector. CLOSED at 6¾** (the `overlay` prop -> `data-overlay` on `Dialog`, `FormDialog`, `SheetContent`, `PopoverContent`, `Glance`; on `DrawerContent` the prop is called `layer`, because `overlay` there is already the backdrop flag; placed on ~105 layers; `$attrs` of `Dialog` is still not forwarded). Below is the original description. All ~60 dialogs are distinguishable only by the heading text; `Popover` has nothing except `data-slot="popover-content"`; `Dialog.vue`/`Drawer`/`FormDialog` do not forward `$attrs` (`DialogContent`/`DrawerContent` without `v-bind="$attrs"`), so a `data-*` on `<Dialog>` in a template is lost.
9. **Address entries without a check. PARTLY CLOSED at 6¾** (`?settings=`, `?whatsNew=1`, `?new=blank|salary|save_share|reminder|alert|limit`, `?edit=`, `?open=`, `?category=`, `?tour=` are checked; `?edit=` on `/automations` turned out to be a dead link — it was parsed only by the list in wallet settings — fixed, parsing moved into `NewAutomation`). Originally: `?new=reminder|alert|salary|save_share|blank|limit`, `?edit=`, `?whatsNew=1`, `?tour=` are not opened by any script; `?new=` is removed by `router.replace` — re-entering the same address will not open the layer (by design, but not checked).
10. **Phone and keyboard.** `Dialog` on a phone does not focus a field (the on-screen keyboard), except `autofocus` — safe; but `FormDialog` in the `phoneSubmitOnly` mode and `Ctrl Enter` "save and continue" (`data-quick-add-keys`) without a button — look at specific forms (probe).
11. **A dialog without Esc/Back. Palette: not reproducible after item 2** (the palette does not open over a modal layer). Originally: Not found: all layers are Reka-based or register `pushBackLayer`. An exception by design: the palette (its own `window keydown`, also closes on an Esc addressed to the Reka layer under it — both close with one press; probe).
12. **The settings window on `/unlock`. CHECKED at 6¾** (`app-settings-unlock`: it opens, Esc/"Back" close it). Originally: `AppSettingsDialog` there is a separate `Dialog`, not `SettingsShell`: `backCloses` does not cover it.

## 5. Numbers

Counted from the tables of section 3 (a row = a layer or a bundle of layers of one component; row 94 is a reference and not counted):

- layers in total: 98 rows. There are more actual layer components: ~40 rows hold 2–10 (for example, #48 — 7 dialogs), about 120 in total.
- without their own layer selector: 66 rows out of 98 — 52 ("shared" `[role="dialog"]`: 42, "none": 10) and 14 more where there is only a `data-tour` on the entry button, not on the layer.
- with a layer selector or part of one: 32 rows (entity cards, glances, the palette, the settings window, the "More" sheet, the amount card, toasts, `data-context-menu` menus).
- already checked by an autotest (yes): quick add, the settings window, the palette, a toast — 4 rows; another 8 rows are covered by `check-links` (cards and glances); the remaining 86 — no.
- suspicious places: 12 (section 4), 3 of which are a direct violation of rules 16/17 (items 1, 3, 4).

### Top 10 suspicious

1. An amount inside a button/link: no conversions on touch and from the keyboard (`MoneyTooltipHost.vue:39`).
2. Ctrl K/Ctrl N over an open dialog: the palette in `body` under `pointer-events:none` (probe).
3. Pinned items are visible only in the sidebar, on a phone they are neither shown nor unpinned.
4. The chart menu (`TradingChart.vue:166`) is right click only; "alert at cursor price" is unavailable on touch.
5. Toasts do not know `Popover`/menus/the palette: a click on "Undo" closes the layer.
6. A toast at the top on a phone covers the header of full-screen sheets and the settings "‹"; `toastOverDialog` does not catch it.
7. Popovers/selects inside a `Drawer` on a phone: swipe gestures versus list scrolling, the layer order rests on `z-50`/`z-[60]`.
8. 66 of 98 layers without `data-*`; `Dialog`/`Drawer` do not forward `$attrs`.
9. `?new=…`, `?edit=`, `?whatsNew=1`, `?tour=` are not checked; `AppSettingsDialog` on `/unlock` is outside `backCloses`.
10. The wallet menu in the sidebar and for contacts is right click without a visible button (wallet groups have no menu on a phone).

## 6. Proposal: `tests/e2e/layout/lib/overlays.mjs`

**Done at stage 6¾.** Differences from the sketch below: the `overlay` prop sits on the content (`SheetContent`, `PopoverContent`), on `DrawerContent` it is called `layer`; the list of entries is moved to `overlay-list.mjs`, the steps to `overlay-steps.mjs`, "a layer over a layer" to `overlay-cross.mjs`. Steps gained `context` (right click) and `hover`, and an entry gained `phoneOpen`, `fresh`, `locked`, `cleanup`, `keys`, `centred` (see the header of `overlay-list.mjs`). For every entry and size the loop is: open, wait for the layer, measure (off-screen, a jump of the center for centered dialogs, a shift after it settles, focus inside the layer, overflow and touch targets ≥ 40 px only inside the layer — `window.__scope` in `probes.mjs`), close with Esc (desktop) or `closeTopLayer()` (phone), check that the layer closed, the address key was removed and the page under the layer did not change size. `--quick` runs 1280 and 390 in one (dark) theme; `--overlays[=id,id]` runs only layers. The original sketch:

1. In the primitives `Dialog`, `FormDialog`, `Sheet`, `Drawer`, `Popover`, `Glance` add an optional `overlay` prop and put it into `data-overlay` on the content root (`DialogContent`, `DrawerContent`, `SheetContent`, `PopoverContent`). One change gives a selector for all layers without editing their templates, and the call site gets one word: `<Dialog overlay="goal" …>`. For the ready-made selectors that already exist (`data-tx-card`, `data-command-palette`…) nothing needs to change, `layer` points to them.
2. The list of layers is data, the traversal is a shared loop: for every entry and window size open, measure, close (Esc), "Back" (`closeTopLayer`), check that the page under the layer did not shift.

The entry format:

```js
// tests/e2e/layout/lib/overlays.mjs
// id        — matches data-overlay (and the number in docs/ui-overlays.md)
// route     — the page on which the layer opens (substitutions: {store:<kind>}, {goal}, {tx})
// open      — steps: { click: selector } | { key: 'Ctrl+K' } | { query: {…} } | { eval: code }
// layer     — the selector of the open layer
// phone     — true: also open at 390/360 (otherwise desktop only); 'only' — phone only
// back      — true by default: closeTopLayer() closes the layer
// closes    — optional: 'esc' | 'back' | 'button' to check a specific way
export const OVERLAYS = [
  { id: 'quick-add', route: '/', phone: true,
    open: [{ click: '[data-tour="quick-add"]' }],
    layer: '[role="dialog"][data-state="open"]' },
  { id: 'settings-phone-group', route: '/', phone: 'only',
    open: [{ query: { settings: 'app' } }],
    layer: '[data-tour="settings-shell"]' },
  { id: 'palette', route: '/', phone: true,
    open: [{ key: 'Ctrl+K' }],
    layer: '[data-command-palette][data-open="true"]' },
  { id: 'entity-goal', route: '/money/plan', phone: true,
    open: [{ query: { open: 'goal:{goal}' } }],
    layer: '[data-goal-card]' },
  { id: 'glance-capital', route: '/', phone: true,
    open: [{ click: '[data-glance-trigger="capital"]' }],
    layer: '[data-glance]' },
  { id: 'budget-from-link', route: '/money/plan', phone: true,
    open: [{ query: { new: 'limit', category: 'food' } }],
    layer: '[data-overlay="budget"]' },
  { id: 'password-set', route: '/?settings=vault', phone: true,
    open: [{ click: '[data-overlay-open="password-set"]' }],
    layer: '[data-overlay="password-set"]' },
]
```

What the loop does for each entry and size: run the steps; wait for `layer` to be visible; measure the jump (like `dialogJump`), overflow and touch targets inside the layer; Esc (desktop) or `closeTopLayer()` (phone) closes; an address layer removes its key; the page under the layer did not change size. Separate end-to-end entries: "a layer over a layer" (a dialog in a sheet, the palette over a dialog, a toast over a sheet on a phone) — for items 2, 4, 6 of section 4.

Rollout order: first `data-overlay` in the primitives, then 10–15 entries (section 3.1 and one layer per section), then the rest by the table; an entry without a `layer` is not created — that is the criterion "a layer without a selector".

## 7. `overlays.mjs` coverage (stage 6¾)

55 entries (1280: all except `phone: 'only'`; 390: those with `phone`) and 4 cross-cutting: `dialog-in-sheet` (item 6), `palette-over-dialog` (item 2), `toast-over-sheet`, `toast-over-glance` (item 4). Inventory rows covered: 57 of 98 (3.1: 22 of 23, 3.2: 4 of 6, 3.3: 8 of 12, 3.4: 4 of 17, 3.5: 6 of 12, 3.6: 7 of 16, 3.7: 3 of 7, 3.8: 3 of 5); 41 rows remain unchecked. `data-overlay` sits on all ~105 dialogs, sheets and popovers with the primitives above; an entry can be added to the list in one line wherever the layer can be opened without a special state.

`sync-plan` runs only at 390 (`phone: 'only'`): closing the window is a refusal, the mock writes a row to the journal and the "History" view on a phone shifts on load after that; at 1280 the same component is covered by the `sync-pairing` check and the window sizes. The run order: desktop layers after the desktop traversal, phone ones at the end, after the phone traversal.

An entry `id` matches `data-overlay` (for layers where a selector existed earlier — with it: `[data-goal-card]`, `[data-command-palette]`, `[data-context-menu]`, `.driver-popover`, `[role="menu"]`).

What remained unchecked and why:

| Rows | Layer | Reason |
|---|---|---|
| 16, 57, 70 (desktop) | `money-tooltip`, `chart-tooltips`, `report-term` | Reka `Tooltip`: no `overlay` prop, opens only on hover and closes when the mouse leaves, Esc/"Back" do not touch it |
| 28, 95 | `home-store-create`, `vault-unlock` | a special state is needed: an empty wallet wizard on Home; a vault with a password (the mock vault has no password, "Open" signs in at once) |
| 32, 34, 62-65, 74-75, 78-79, 82 | `tx-link`, `tx-bulk-pick`, `goal-delete` (is in the cross-cutting one), `plan-calculator`, `trade-record`, `emergency-fund`, `automation-simulate`, `automation-confirm`, `template-dialog`, `source`, `workflow-leave` | the entry takes several steps through a card menu / selecting rows / leaving the editor; `data-overlay` exists, the entry does not |
| 35, 67 | `tx-export`, `report-export` | a `DropdownMenu` without an `overlay` prop and without a shared selector |
| 38, 45-46, 49-56, 58 | `debt-terms`, `sidebar-wallet-menu`, `sidebar-pinned-menu`, `asset-details`, `utility`, `vehicle`, `property-value`, `asset-class`, `attachments`, `exchange-menu`, `chart-menu`, `wallet-settings-dialogs` | a wallet of a special kind is needed (debt, asset, vehicle), a pinned item or an expanded sidebar group; right click on a chart |
| 43 | `store-delete` | the "Delete" item is inactive for wallets with transactions, and in the mock all of them have some |
| 69, 83-86 | `calc-scenarios`, `workflow-toolbar-menus`, `workflow-mobile-bar`, `workflow-inspector`, `workflow-add-node` | the scheme canvas (vue-flow), the entry is a tap on a node / a double tap |
| 87 (set/remove), 89 (send), 90, 92, 93 | password: set/remove, `vault-send`, `base-currency`, `enrichment-settings`, `dev-tools` | the entry depends on state (whether there is a password), on a choice in the currency list, or on development mode |
| 97 | `vault-delete` | a recents list with an entry that can be forgotten is needed |

Found and fixed by the check (all of them real app defects):

1. `MoneyTooltipHost`: the conversion hint opened on focus of any panel containing an amount (phone: the converter, `focusFirstField` focuses the panel); the hint became the top layer and "Back"/Esc closed it first. Now only the button/link with the amount itself reacts to focus (`sumOfFocus`).
2. `MoneyCardHost`: focus did not get into the amount card on desktop (Reka sends the event to the popover wrapper), the `C` key did not work. Focus is now on the card itself.
3. `Dialog`: `focusFirstField` picked a hidden `<input type="file" class="hidden">` and focus was lost (scheme import). The first rendered field is taken.
4. `SettingsShell`: `@open-auto-focus.prevent` left focus outside the settings window; focus is now on the window itself (it takes no field, the keyboard does not slide up on a phone).
5. `ActionMenu`: the caller's `class` ("relative z-10" in `WalletTable`) was lost, "⋯" ended up under the stretched row link — a click opened the wallet, not the menu. The class is now on the button.
6. `NewAutomation`: `?edit=<id>` on `/automations` (the "Edit" link from an automation card) opened nothing. The form opens and the key is removed on close.
7. `StoreNodeMultiSelect`: touch targets of the checkboxes were 38x32. Rows with `touch-hit`.
8. Global keys over a modal layer (item 2 of section 4), `Popover` and the toast, the toast over the header on a phone (item 4).

## 8. A layer in a layer (task 227)

"Parent - nested" pairs are checked by `tests/e2e/layout/lib/overlay-nested.mjs` (desktop 1280 with a mouse, phone 390 with touch; on the mock only): a Select in a dialog, date selection in a dialog, ConfirmButton and a menu in a sheet, the `Money` amount panel in a quick glance and in a sheet, a dialog over settings, Select and ConfirmButton in settings (desktop). For each pair: the nested one is on top and entirely within the window; a press inside it does not close the parent; a press on the parent closes only the nested one; Esc / "Back" pops one layer at a time; focus returns to the parent; after both layers the page is clickable again and not blocked.

What was broken and fixed:

| What | Cause | Where fixed |
|---|---|---|
| After closing the amount panel, a menu and a ConfirmButton question on a phone, and a dialog over settings, focus fell to `<body>`: the parent got no keys, and "Back" targets the focused node | the layer has no "button that opened it" (a click on an amount, opening by route, the row went away) and Reka returns focus to `body` | `src/shared/lib/layer-focus.ts` (one observer for all layers, installed in `App.vue`): remembers who had focus when the layer took it and after closing returns it there, otherwise to the top remaining layer |
| The amount card (`MoneyCardHost`) opened under the quick glance (phone), a click on it closed the glance, while a click on the glance did not close the card | the portal (`Teleport`) lives since the shell started and keeps its place in `body`; the order of layers and Reka's "outside/inside" are decided by the order in `body` | `src/shared/lib/late-portal.ts`: `PopoverContent` and `DrawerContent` create the portal when the layer opens (and hold it while closing): it goes to the end of `body` |
| A popover with a calendar (date selection in a dialog) on a short window went past the bottom edge | `PopoverContent` had no height limit | `src/shared/ui/popover/PopoverContent.vue`: `max-h` by the available height and scrolling |

The rest (portal order, outside-click, Esc one layer at a time) already worked on the shared Reka stack and `back-stack.ts`; the probe holds this.
