# Опись слоёв интерфейса

Опись всего, что всплывает поверх страницы: диалоги, шторки, поповеры, меню, взгляды, Ctrl K, карточки сущностей, подсказки. Составлена чтением кода ветки `feature/ui-redesign`; разделы 1–5 составлены чтением кода; на этапе 6¾ добавлена автопроверка `scripts/check-layout/overlays.mjs` (разделы 6–7), и найденное ею исправлено. Номера строк даны по состоянию рабочей копии.

Зачем: у автотеста (`scripts/check-layout`) должен быть список «что открыть, как, и что считать открытым». Сейчас такого списка нет — см. раздел 5.

## 1. Общая логика слоёв

| Вопрос | Как устроено | Где |
|---|---|---|
| Диалог на телефоне | `Dialog` при `useIsPhone()` (граница `md`) рисуется как `Drawer` (шторка снизу, свайп закрывает, крестик 40 px); на ПК — центрированная панель `z-50`. `FormDialog` — тот же `Dialog` с формой, Enter сохраняет | `src/shared/ui/dialog/Dialog.vue:57,89`, `src/shared/ui/form-dialog/FormDialog.vue` |
| Шторка `Sheet` | Не превращается: справа, на телефоне `w-full` (экран целиком, без свайпа). Так сделаны карточки сущностей, колокольчик, список переменных | `src/shared/ui/sheet/SheetContent.vue:44-47` |
| Drawer | Reka Drawer, снизу, `max-h` под строку состояния; `overlay:false` — немодальный (`MobileInspector`) | `src/shared/ui/drawer/DrawerContent.vue` |
| Esc | Все слои на Reka (Dialog, Sheet, Drawer, Popover, меню, Tooltip) закрываются Esc сами. Ctrl K — свой слушатель `window keydown` (`CommandPalette.vue:60`). Взгляд: Esc + «Enter — открыть» (`Glance.vue:34`) | — |
| «Назад» (Android, `onBackButton`) | `useBackButton` -> `closeTopLayer()`: сначала слои из `pushBackLayer` (Ctrl K, тур), иначе шлёт Esc в верхний `[data-dismissable-layer]`; только потом `router.back()`; на Главной второй «Назад» за 2 с сворачивает | `src/shared/lib/back-stack.ts`, `src/pages/app-shell/model/use-back-button.ts` |
| Слой в адресе | `?settings=<группа>&section=` (закрытие — `router.replace` без ключей), `?open=kind:id~kind:id` (стопка карточек, до 8; закрытие снимает верхнюю) | `use-settings-nav.ts`, `entity-link.ts:40-77` |
| Тосты | `Sonner` в `App.vue:88`, вне `AppShell` (работает и на `/unlock`); на телефоне сверху по центру под `--safe-top`. Явного z-index нет (берётся из vue-sonner). Диалог, шторка и Drawer не закрываются от клика по тосту (`keepOpenOnToast`) | `src/shared/lib/toast-outside.ts` |
| Шкала z | `z-sticky` 10, `z-banner` 40, `z-overlay` 50 (`styles.css:366-376`). Dialog, Drawer, Sheet, Popover, Tooltip, Select — голое `z-50`; Combobox — `z-[60]` (`Combobox.vue:165`). Порядок слоёв друг над другом решает порядок порталов в `body` | — |
| Меню действий | `ContextMenu` (правый клик, Shift+F10; Reka добавляет долгое нажатие) + «⋯» `ActionMenu`: на ПК `DropdownMenu`, на телефоне `ActionSheet` (шторка) из того же списка `UiAction` | `src/shared/ui/action-menu`, `src/shared/ui/context-menu` |
| Подсказки | `Tooltip` (задержка 300 мс, `App.vue:81`) — только наведение и фокус. Там, где подсказка несёт смысл, на касании ставят поповер (`ReportTerm.vue:21`) | — |
| Клавиши | `Ctrl N` — быстрое добавление, `Ctrl K` — палитра, `Ctrl B` — боковая панель (ПК), `Alt 1…5` — разделы, `Ctrl Enter` — «сохранить и дальше» (`src/shared/lib/hotkeys/hotkeys.ts:27-38`) | — |

Хост адресных слоёв (разбор query):

| Ключ | Что открывает | Кто разбирает |
|---|---|---|
| `?open=kind:id~…` | карточка сущности. Виды: `store, tx, category, contact, goal, budget, automation, workflow, reminder, alert, asset, scenario`; для `store/workflow/scenario` вместо карточки идёт страница (`entityPage`) | `EntityHost` в `AppShell`, `use-entity-host.ts`; `tx/goal/automation` — свои шторки, остальное — `EntitySheet` |
| `?settings=<группа>[&section=]` | окно настроек (`list` — список групп на телефоне) | `SettingsShell`, `use-settings-nav.ts`; ссылки — `settings-link.ts` |
| `?new=blank` | мастер автоматизации | `NewAutomation.vue:24` |
| `?new=reminder\|alert\|salary\|save_share` | диалог на странице автоматизаций, ключ сразу снимается | `NewAutomation.vue:39` |
| `?new=salary` | пресет «зарплата» в списке автоматизаций | `use-new-from-route.ts:14` |
| `?new=limit[&category=]` | диалог лимита на «Плане» | `PlanLimits.vue:80` |
| `?edit=<id>` | форма автоматизации | `use-new-from-route.ts:31` |
| `?whatsNew=1` | «Что нового» (вся история) | `WhatsNewDialog.vue:18` |
| `?tour=<маршрут>`, `?lesson=` | тур (driver.js) | `use-shell-state.ts:40` |
| `?scenario=`, `?category=`, `?contact=`, `?tab=`, `?view=`, `?program=` | не слои: выбор содержимого (категория открывает `CategorySheet`) | страницы |

Проверка адресных ссылок — `scripts/check-links.mjs` (карточки, взгляды, стопка «← Назад»); `?new=`, `?edit=`, `?whatsNew=` ею не проверяются.

## 2. Что уже проверяет `scripts/check-layout`

Сценарии в `scripts/check-layout/scenario.mjs`, измерения в `probes.mjs`:

- `views(page, size)` (:114) — только страницы: статические маршруты, по кошельку каждого вида и его настройки, каждая вкладка настроек (`?settings=&section=`). Слои на странице не открываются; окно настроек измеряется как страница (переполнение, сдвиг, цели касания).
- `dialogJump` (:208) — один раз, 1280 мышь: на `/` клик `[data-tour="quick-add"]`, пять замеров `[role="dialog"][data-state="open"]` за 0…~600 мс, центр не уходит больше чем на 2 px, закрытие Esc. Только быстрое добавление; на телефоне не гоняется.
- `backCloses` (:256) — один раз, 390: `closeTopLayer()` закрывает окно настроек (`?settings=app`), быстрое добавление (`[data-tour="quick-add"]`), палитру (`[data-command-palette-trigger]`). Открытость — `[role="dialog"][data-state="open"], [data-command-palette][data-open="true"]`.
- `toastOverDialog` (:299) — один раз, 1280: клик по «Undo» тоста поверх окна настроек срабатывает и окно не закрывается.
- `confirmStable` (:283) — `ConfirmButton` не меняет ширину и фокус (это не слой, но рядом).
- `listenerLeak` — не слои.
- **Этап 6¾:** `overlays.mjs` (+ `overlay-list.mjs`, `overlay-steps.mjs`, `overlay-cross.mjs`) — каждый слой из списка открывается, замеряется и закрывается на 1280 и 390; идёт и в `--quick`. Что покрыто — раздел 7.

`scripts/check-links` дополнительно открывает: карточки `?open=` (`[data-entity-card]`), взгляды (`[data-glance-trigger]` -> `[data-glance]`, выход `[data-glance-open]` или `a[href]`), стопку карточек и `[data-entity-back]`, до 6 взглядов на страницу.

Итог: вне `dialogJump/backCloses/toastOverDialog` ни один диалог, шторка, поповер и меню автотестом не открывается; на телефоне замеряются только страницы и три слоя в `backCloses`.

Столбец «Проверка» ниже: **да** — слой открывается скриптом; **links** — только `check-links`; **стр.** — измеряется лишь страница под слоем; **нет**.

## 3. Опись

Обозначения. «Селектор»: **общий** — только `[role="dialog"]` Reka (у `Dialog` на ПК и `Drawer` на телефоне свой `data-state`, своего `data-*` нет); **есть: …** — уже существующий; **нет** — нужен `data-overlay`. «Предл.» — имя для `data-overlay`. Вход «меню» = «⋯»/правый клик строки. Все диалоги на телефоне — шторка снизу (`Dialog` -> `Drawer`), если не сказано иное. Пути даны от `src/`.

### 3.1 Оболочка

| # | Слой (предл. `data-overlay`) | Компонент | Внутри | Входы | ПК / телефон | Селектор | Проверка |
|---|---|---|---|---|---|---|---|
| 1 | `quick-add` | `widgets/quick-add/.../QuickAddDialog.vue` (в `AppShell`, ещё в `Wallet.vue` для сделки) | форма операции, серия, подсказка клавиш | кнопка в шапке и «+» нижней панели (`data-tour="quick-add"`); `Ctrl N`; событие трея `quick-add`; Ctrl K «Добавить»; `openQuickAdd()` из взглядов/кошелька/Главной | ПК: панель `size=form`; телефон: шторка, кнопка сохранения на всю ширину (`data-quick-add-submit`) | есть: `data-tour="quick-add"` (кнопка), `data-quick-add-submit` | да: dialogJump (ПК), backCloses (тел.) |
| 2 | `settings` | `widgets/settings-shell/SettingsShell.vue` (Reka `DialogRoot`, не `Dialog`) | группы и вкладки, разделы, тур `settings` | `?settings=`; сайдбар; «Ещё» нижней панели; Ctrl K «Настройки»; трей `settings`; «Что нового»/уведомления с ссылкой | ПК: окно 230x175; телефон: весь экран, список групп -> группа, «‹» назад; «Назад» идёт группа -> список -> закрыто | есть: `data-tour="settings-shell"`, `settings-groups` | да: backCloses (тел.), toastOverDialog, views (вкладки как страницы) |
| 3 | `palette` | `pages/app-shell/ui/command-palette/CommandPalette.vue` (свой слой, `Teleport`, не Reka) | поиск, действия, конвертер сумм, взгляд справа (`PalettePeek`), подсказки клавиш | `Ctrl K`; кнопка в шапке `data-command-palette-trigger`; на телефоне иконка лупы | ПК: окно 620/860 px, подложка; телефон: весь экран с «Отмена» (`data-command-palette-cancel`); Esc — свой слушатель; «Назад» — `pushBackLayer` | есть: `data-command-palette`, `data-open`, `-backdrop`, `-trigger`, `-cancel` | да: backCloses (тел.) |
| 4 | `converter` | `features/currency-convert/ConverterDialog.vue` (в `AppShell`) | конвертер валют | Ctrl K «Конвертер» и `100 usd`; меню шестерёнки кошелька; `openConverter()` | шторка на телефоне | общий | нет |
| 5 | `whats-new` | `widgets/whats-new/WhatsNewDialog.vue` | записи «Что нового» | сам при непрочитанном; `?whatsNew=1`; Ctrl K «Что нового»; Настройки › Справка | шторка | общий | нет |
| 6 | `sync-pairing`, `sync-plan` | `features/vault-sync/ui/SyncPairingDialog`, `SyncPlanDialog` | сопряжение; план слияния | события бэкенда синхронизации (`useSyncEvents`) | шторка | общий | нет |
| 7 | `enrichment-consent` | `features/enrichment-consent/EnrichmentConsentDialog.vue` | согласие на обогащение | флаг `settingsStore.enrichmentConsentOpen` | шторка | общий | нет |
| 8 | `notifications` | `widgets/notification-bell/NotificationBell.vue` | `Sheet` справа: список уведомлений | колокольчик в шапке (`data-tour="notification-bell"`) | телефон: весь экран | есть: `data-tour="notification-bell"` (кнопка) | нет |
| 9 | `bottom-more` | `pages/app-shell/ui/app-bottom-nav/AppBottomNav.vue` | `Drawer`: остальные разделы, настройки, блокировка | «Ещё» (`data-bottom-more`) | только телефон | есть: `data-bottom-more-sheet` | нет |
| 10 | `entity-tx` | `widgets/transaction-details/TransactionDetailsSheet.vue` (через `EntityHost`) | карточка операции, связи, действия (`TxMore`) | `?open=tx:id`; клик/Enter по строке (`data-tx-open`); меню строки «Быстрый взгляд»; Ctrl K; уведомление `open:tx:` | телефон: весь экран; тап по кошельку — текст | есть: `data-tx-card`, `-actions`, `data-entity-card` | links |
| 11 | `entity-goal` | `widgets/goal-details/GoalDetailsSheet.vue` | цель, прогресс, действия, удаление (`GoalDelete` — диалог внутри) | `?open=goal:id`; `EntityLink` | как выше | есть: `data-goal-card`, `-actions`, `data-entity-card` | links |
| 12 | `entity-automation` | `widgets/automation-details/AutomationDetailsSheet.vue` | автоматизация, «Изменить», «В редактор» | `?open=automation:id`; взгляд «авто»; журнал | как выше | есть: `data-automation-card`, `-actions`, `data-entity-card` | links |
| 13 | `entity-card` | `widgets/entity-sheet/EntitySheet.vue` | общая карточка категории, контакта, лимита, напоминания, оповещения, актива, группы связей, «← Назад» (`data-entity-back`) | `?open=` для остальных видов; `EntityLink` | как выше | есть: `data-entity-card="kind:id"`, `data-tour="entity-card"` | links |
| 14 | `entity-dialogs` | `EntityHost.vue`: `GoalDialog`, `BudgetDialog`, `PriceAlertDialog` | формы, запрошенные действием карточки (`entityDialog`) | действие в карточке/меню `EntityLink` | шторка; живут рядом с карточкой | общий | нет |
| 15 | `money-card` | `entities/currency/ui/money-card-host/MoneyCardHost.vue` | сумма в базовой валюте по курсу операции и сегодняшнему, избранные; «Все курсы» (в Настройки) | клик (тап) по сумме `<Money>` вне кнопки/ссылки; `C` копирует | ПК: поповер у суммы; телефон: `Drawer` | есть: `data-money-card-host`, `-rates`, на сумме `data-money-open` | нет |
| 16 | `money-tooltip` | `entities/currency/ui/money-tooltip-host/MoneyTooltipHost.vue` | конверсии | наведение 300 мс на сумму внутри кнопки/ссылки | ПК; на касании не показывается (`pointerType==='touch'`) | нет | нет |
| 17 | `tx-row-menu` | `entities/transaction/ui/transactions-list/TransactionsList.vue` + `features/transaction-row-menu/TxRowMenuHost.vue` | меню строки; «Изменить» (`TransactionEditDialog`), «Повторить» (`TransactionCreateDialog`), подтверждение удаления (`Dialog size=sm`) | правый клик (на ПК); «⋯» `ActionMenu` в строке | ПК: контекстное меню + «⋯»; телефон: контекстное отключено (`:disabled="coarse"`), «⋯» открывает `ActionSheet` | есть: `data-context-menu` (меню ПК); у диалогов — общий | нет |
| 18 | `sandbox-reset` | `pages/app-shell/ui/sandbox-bar/SandboxResetDialog.vue` | сброс песочницы | кнопка в `SandboxBar` | шторка | общий | нет |
| 19 | `tour` | `shared/lib/tour/use-tour.ts` (driver.js) | всплывашка тура поверх страницы | «?» в шапке; первый заход; `?tour=` | «Назад» закрывает (`pushBackLayer`) | есть: `.driver-popover` | нет |
| 20 | `toast` | `App.vue:88` + `shared/ui/sonner` | тосты с «Undo» | `notify.*` | ПК: справа внизу; телефон: сверху по центру | есть: `data-sonner-toast` | да: toastOverDialog (ПК) |
| 21 | `entity-link-menu` | `shared/ui/entity-link/EntityLink.vue` | меню действий вида + «Закрепить» | правый клик/Shift+F10 по ссылке | на касании действий нет, они в карточке | есть: `data-context-menu` | нет |
| 22 | `glance-wallet` | `widgets/quick-glance/ui/WalletGlance.vue` | остатки, конвертация, перевод | клик по названию кошелька в строке операции (слот `#side` списков) и в карточках операции/цели | ПК: `Popover` 360 px; телефон: `Drawer`; но в строках операций на телефоне кошелёк — текст | есть: `data-glance-trigger`, `data-glance`, `-open`, `-convert`, `-transfer` | links |
| 23 | `glance-automation` | `.../AutomationGlance.vue` | сумма, кто, журнал; `AutomationAmountDialog` | клик по «авто» в строке операции | как выше | есть: `data-glance-auto-*` | links |

### 3.2 Главная

| # | Слой | Компонент | Внутри | Входы | ПК / телефон | Селектор | Проверка |
|---|---|---|---|---|---|---|---|
| 24 | `glance-capital` | `widgets/quick-glance/ui/CapitalGlance.vue` (`HomeFigures.vue:70`, `Stores.vue:87`) | кошельки и «почему» | клик по цифре капитала | поповер / Drawer | есть: `data-glance-wallets`, `-why` | links |
| 25 | `glance-spent` | `.../SpentGlance.vue` (`HomeFigures.vue:94`) | категории, «Задать лимит» (`?new=limit`) | клик по «Потрачено» | поповер / Drawer | есть: `data-glance-category`, `-limit` | links |
| 26 | `dashboard-catalog` | `pages/dashboard/ui/dashboard-catalog/DashboardCatalog.vue` | каталог виджетов | кнопка настройки Главной | шторка | общий | нет |
| 27 | `widget-menu` | `shared/ui/widget-frame/WidgetFrame.vue` | меню виджета (скрыть, закрепить) | правый клик + «⋯» (`ActionMenu`) | ПК / `ActionSheet` | есть: `data-context-menu` (ПК) | нет |
| 28 | `home-store-create` | `StoreCreateDialog` из `HomeFirstSteps`/`OnboardingChecklist` | новый кошелёк | кнопка первого шага | шторка | общий | нет |
| 29 | `tx-row-menu` (Главная) | `TxRowMenuHost` в `HomeChanges.vue` | см. #17 | см. #17 | см. #17 | см. #17 | нет |

### 3.3 Операции

| # | Слой | Компонент | Внутри | Входы | ПК / телефон | Селектор | Проверка |
|---|---|---|---|---|---|---|---|
| 30 | `tx-create` | `features/transaction-create/TransactionCreateDialog.vue` | полная форма операции | кнопка на кошельке, в карточке цели/коммуналки; «Повторить»; план из калькулятора | шторка | общий | нет |
| 31 | `tx-edit` | `features/transaction-edit/TransactionEditDialog.vue` | правка операции | «Изменить» в меню строки и карточке | шторка | общий | нет |
| 32 | `tx-link` | `features/transaction-link/TransactionLinkDialog.vue` | связать операции | `TxMore` в карточке операции | шторка | общий | нет |
| 33 | `tx-save-filter` | `features/transaction-filter/SaveFilterButton.vue` | имя фильтра | кнопка в Операциях (`data-tour="tx-save-filter"`) | шторка | есть: `data-tour` (кнопка) | нет |
| 34 | `tx-bulk-pick` | `features/transaction-bulk/.../BulkPickDialog.vue` | выбор категории/кошелька для выбранных | панель массовых действий | шторка | общий | нет |
| 35 | `tx-export` | `features/report-export/TransactionsExportButton.vue` | `DropdownMenu`: форматы | кнопка в Операциях | меню по клику | нет | нет |
| 36 | `contact-menu` | `pages/transactions/ui/contact-list/ContactList.vue` | действия контакта | правый клик/долгое нажатие по строке; на экране контакта есть «⋯» (`ContactDetails.vue`) | ПК: список слева; телефон: список -> экран контакта | есть: `data-context-menu` | нет |
| 37 | `contact-edit` | `features/contact-edit/ui/ContactEditDialog.vue`, `ContactAccountRow.vue` (меню+`ActionSheet`) | форма контакта; счета | кнопки в Контактах | шторка | общий | нет |
| 38 | `debt-terms` | `features/debt-manage/ui/DebtTermsPopover.vue` | условия долга | клик по фразе долга | `Popover` на обоих | нет | нет |
| 39 | `bonus-program` | `features/loyalty-edit/PointsDialog.vue`, `ui/ProgramCreateDialog.vue` (`data-tour="bonuses-wizard"`), `ProgramTable.vue` (`ActionMenu`) | баллы, новая программа | кнопки на «Бонусах» | шторка | есть: `data-tour="bonuses-wizard"` (форма) | нет |
| 40 | `calendar-row-menu` | `widgets/spending-calendar` + `TxRowMenuHost` | см. #17 | день календаря | — | см. #17 | нет |
| 41 | `store-multiselect` | `entities/store/ui/store-node-select/StoreNodeMultiSelect.vue` | дерево кошельков (флажки) | поле фильтра | `Popover` по клику | нет | нет |

### 3.4 Кошельки

| # | Слой | Компонент | Внутри | Входы | ПК / телефон | Селектор | Проверка |
|---|---|---|---|---|---|---|---|
| 42 | `store-create` | `features/store-create/StoreCreateDialog.vue` | новый кошелёк / подкошелёк | «Добавить» в сайдбаре и на «Кошельках»; Главная; подкошелёк в кошельке | шторка | общий | нет |
| 43 | `store-delete` | `Dialog` в `pages/stores/Stores.vue:140` | подтверждение удаления | меню карточки/строки | шторка | общий | нет |
| 44 | `store-menu` | `entities/store/ui/store-card/StoreCard.vue`, `pages/stores/ui/WalletTable.vue` | действия кошелька | правый клик + «⋯» | ПК / `ActionSheet` | есть: `data-context-menu` (ПК), `data-tour="stores-list"` | нет |
| 45 | `sidebar-wallet-menu` | `widgets/sidebar-wallets/SidebarWallets.vue`, `ui/sidebar-wallet-item/SidebarWalletItem.vue` | действия группы и кошелька | правый клик | только ПК (боковая панель на телефоне скрыта) | есть: `data-context-menu` | нет |
| 46 | `sidebar-pinned-menu` | `widgets/sidebar-pinned/SidebarPinned.vue` | снять закрепление и т. п. | правый клик | только ПК | есть: `data-context-menu` | нет |
| 47 | `wallet-gear` | `pages/wallet/ui/wallet-gear-menu/WalletGearMenu.vue` | `DropdownMenu`: настройки, конвертер… | шестерёнка (`data-tour="wallet-gear"`) | меню по клику, одно на оба | есть: `data-tour` | нет |
| 48 | `wallet-dialogs` | `pages/wallet/Wallet.vue:226-242`: `AdjustmentDialog`, `StoreSettingsDialog`, `StoreChangeDialog`, `PropertySellDialog`, `PlanCalculatorDialog`, `GoalDialog`, `AutomationDialog` (`StoreCreate` и `TransactionCreate` см. #42, #30) | действия кошелька | кнопки/меню шестерёнки на странице кошелька | шторка; «один диалог за раз, не внутри другого» (комментарий `Wallet.vue:96`) | общий | нет |
| 49 | `asset-details` | `widgets/asset-details/AssetDetailsSheet.vue` (+`AssetRevalueButton` `FormDialog`, `PriceAlertDialog`) | актив: позиция, переоценка, оповещение | строка позиции в кошельке; `?open=asset:` идёт в `EntitySheet` | `Sheet` справа / экран | нет | нет |
| 50 | `utility` | `pages/wallet/ui/utility-item/UtilityItemSheet.vue`, `UtilityDialog`, `UtilityBillDialog`, `UtilityRemindersButton` (`data-tour="utility-reminders"`) | коммуналка: счёт, показания, напоминания | вкладка «Коммуналка» | `Sheet` + шторки | есть: `data-tour` (кнопка) | нет |
| 51 | `vehicle` | `widgets/vehicle-card`: `MileageDialog`, `ServiceDoneDialog` | пробег, обслуживание | карточка авто | шторка | общий | нет |
| 52 | `property-value` | `features/property-revalue/RevalueRuleDialog.vue` | правило переоценки | карточка недвижимости | шторка | общий | нет |
| 53 | `asset-class` | `features/asset-class-edit/ui/AssetClassDialog.vue` | класс актива | `TopupAdvisor` | шторка | общий | нет |
| 54 | `attachments` | `entities/attachment/ui/attachment-add/AttachmentAdd.vue` (`Popover`/`Drawer` `data-attachment-sheet`, `DropdownMenuRoot`), `AttachmentItem.vue` (`ActionMenu`) | добавить вложение | поле вложений | ПК: меню; телефон: шторка | есть: `data-attachment-sheet` | нет |
| 55 | `exchange-menu` | `widgets/exchange-terminal/ExchangeTerminal.vue` | действия пары, `PriceAlertDialog` | правый клик + «⋯» | ПК / `ActionSheet` | есть: `data-context-menu` | нет |
| 56 | `chart-menu` | `widgets/trading-chart/TradingChart.vue:166` | «Алерт на цене», инструменты рисования | правый клик по графику | только правый клик: на касании не открывается (алерт по последней цене и инструменты есть на панели `ChartToolbar`) | есть: `data-context-menu` | нет |
| 57 | `chart-tooltips` | `ChartToolbar.vue` (5 `Tooltip`), echarts-подсказки в виджетах | подписи кнопок; значения | наведение/фокус; у кнопок есть `aria-label` | echarts — тап | нет | нет |
| 58 | `wallet-settings-dialogs` | `StoreChangeDialog`, `StoreDangerZone` на `/stores/:id/settings` | смена типа, удаление | кнопки страницы | шторка | общий | стр. |

### 3.5 Деньги

| # | Слой | Компонент | Внутри | Входы | ПК / телефон | Селектор | Проверка |
|---|---|---|---|---|---|---|---|
| 59 | `category-sheet` | `pages/money/ui/category-sheet/CategorySheet.vue` | категория: операции, лимит | `?category=`; клик по категории в «Тратах» | `Sheet`, на телефоне экран | есть: `data-tour="category-sheet"`, `data-category-sheet-limit` | нет |
| 60 | `budget` | `features/budget-edit/BudgetDialog.vue` | лимит | «Лимит» на «Плане» (`data-plan-limit`), `?new=limit`, Ctrl K, взгляд «Задать лимит», `SpendingCategories` | шторка | общий | нет |
| 61 | `goal` | `features/goal-edit/GoalDialog.vue` | цель | «План», «Где деньги», калькулятор, кошелёк, карточка кошелька | шторка | общий | нет |
| 62 | `goal-delete` | `features/goal-delete/ui/GoalDelete.vue` | подтверждение, внутри `GoalDetailsSheet` | карточка цели | диалог над шторкой | общий | нет |
| 63 | `plan-calculator` | `features/plan-calculator/PlanCalculatorDialog.vue` | калькулятор плана | «Инвестиции», кошелёк | шторка | общий | нет |
| 64 | `trade-record` | `features/trade-record/TradeRecordDialog.vue` | запись сделки | «Инвестиции» | шторка | общий | нет |
| 65 | `emergency-fund` | `features/emergency-fund-edit/EmergencyFundDialog.vue` | подушка | `FundCard`, «Здоровье» | шторка | общий | нет |
| 66 | `money-wallets-filter` | `pages/money/ui/money-toolbar/MoneyToolbar.vue:79` | `Popover`: выбор кошельков; рядом период, экспорт (`data-tour="period-picker"`, `report-export`, `report-filter`) | кнопка панели | `Popover` на обоих (w ≤ 18 rem) | есть: `data-tour` (кнопки) | нет |
| 67 | `report-export` | `features/report-export/ReportExportButton.vue` | `DropdownMenu` | кнопка | меню | нет | нет |
| 68 | `calc-compare` | `widgets/invest-result/ui/CompareMenu.vue` | `Popover`: что сравнить | кнопка калькулятора | поповер | есть: `data-calc-compare-menu` | нет |
| 69 | `calc-scenarios` | `features/calc-scenario-manage/ScenarioBar.vue` (`ActionMenu`) + `ui/ScenarioNameDialog.vue` | сценарии | «⋯» (`data-tour="calc-scenarios"`) | ПК: меню; телефон: `ActionSheet` | есть: `data-tour` | нет |
| 70 | `report-term` | `entities/report/ui/report-term/ReportTerm.vue` | пояснение термина | наведение/фокус; на касании тап | ПК: `Tooltip`; телефон: `Popover` (`data-touch-inline`) | нет | нет |

### 3.6 Автоматизации

| # | Слой | Компонент | Внутри | Входы | ПК / телефон | Селектор | Проверка |
|---|---|---|---|---|---|---|---|
| 71 | `automation-wizard` | `features/automation-edit/ui/automation-wizard/AutomationWizard.vue` | мастер (шаг операции — форма `TransactionCreate`) | `?new=blank`; «Создать» (`data-tour="automations-create"`) | шторка | есть: `data-tour` (кнопка) | нет |
| 72 | `automation-forms` | `SalaryDialog` (`data-tour="salary-form"`), `SaveShareDialog` (`save-share-form`), `ReminderDialog`, `PriceAlertDialog` из `NewAutomation.vue` | готовые формы | `?new=salary\|reminder\|alert\|save_share` | шторка | есть: `data-tour` (формы) | нет |
| 73 | `automation-edit` | `features/automation-edit/AutomationDialog.vue` | форма автоматизации | `?edit=<id>`; список; кошелёк | шторка | общий | нет |
| 74 | `automation-simulate` | `features/automation-simulate/AutomationSimulationDialog.vue` | пробный прогон | список автоматизаций | шторка | общий | нет |
| 75 | `automation-confirm` | `features/workflow-manage/.../AutomationConfirmDialog.vue` | подтверждение из журнала | `AutomationsJournal` | шторка | общий | нет |
| 76 | `workflow-import` | `WorkflowImportDialog.vue` | импорт схемы | кнопка (`data-tour="automations-import"`) | шторка | есть: `data-tour` (кнопка) | нет |
| 77 | `workflow-vars` | `Sheet` в `pages/automations/AutomationsSection.vue:75` + `WorkflowVarDialog` | переменные | кнопка (`data-tour="workflow-vars-open"`) | `Sheet`, на телефоне экран | есть: `data-tour` (кнопка) | нет |
| 78 | `template-dialog` | `features/workflow-template-gallery/ui/TemplateDialog.vue` | шаблон | галерея | шторка | общий | нет |
| 79 | `source` | `features/data-source-edit/ui/SourceDialog.vue` (`data-tour="source-url"`, `source-test`), `SourceRow.vue` (`ActionMenu`) | источник данных | «Источники» | шторка / `ActionSheet` | есть: `data-tour` (поля) | нет |
| 80 | `reminder` | `features/reminder-edit/ReminderDialog.vue` (`data-tour="reminder-repeat"`), `widgets/reminder-list/ui/ReminderRow.vue` (`DropdownMenu`) | напоминание | `?new=reminder`, список | шторка / меню | есть: `data-tour` | confirmStable (только `ConfirmButton`) |
| 81 | `price-alert` | `features/price-alert-edit/PriceAlertDialog.vue` | оповещение о цене | `?new=alert`, список, терминал, карточка актива/`alert` | шторка | общий | нет |
| 82 | `workflow-leave` | `Dialog` в `pages/automations/WorkflowEdit.vue:96` | «Выйти без сохранения» | уход со страницы редактора (route guard) | шторка | общий | нет |
| 83 | `workflow-toolbar-menus` | `widgets/workflow-editor/ui/EditorToolbar.vue`, `RunFromMenu.vue` (`DropdownMenu`) | команды, «Запустить с…» | кнопки панели (`data-tour="workflow-edit-commands"`, `workflow-run`) | ПК | есть: `data-tour` | нет |
| 84 | `workflow-mobile-bar` | `widgets/workflow-editor/ui/MobileEditorBar.vue` (`DropdownMenu` + `Dialog` переименования) | команды, имя | панель редактора на телефоне | телефон | есть: `data-tour` (кнопки) | нет |
| 85 | `workflow-inspector` | `widgets/workflow-editor/ui/MobileInspector.vue` | немодальный `Drawer` со снап-точками: свойства узла | тап по узлу | телефон | нет | нет |
| 86 | `workflow-add-node` | `widgets/workflow-editor/ui/QuickAddHost.vue` | `Drawer` (телефон) / панель (ПК): добавить узел | кнопка «+»/двойной тап по холсту | оба | нет | нет |

### 3.7 Настройки (внутри окна `?settings=`)

| # | Слой | Компонент | Внутри | Входы | ПК / телефон | Селектор | Проверка |
|---|---|---|---|---|---|---|---|
| 87 | `password-set` / `-change` / `-remove` | `features/vault-password/ui/SetPasswordDialog`, `ChangePasswordDialog`, `RemovePasswordDialog` | пароль хранилища | Настройки › Хранилище | диалог над полноэкранным окном | общий | нет |
| 88 | `backup-restore` | `features/vault-backups/ui/RestoreBackupDialog.vue` | восстановление копии | Настройки › Хранилище | шторка | общий | нет |
| 89 | `vault-send` / `vault-receive` | `features/vault-transfer/ui/SendVaultDialog`, `ReceiveVaultDialog` | перенос хранилища | Настройки › Синхронизация; `Unlock` | шторка | общий | нет |
| 90 | `base-currency` | `features/base-currency-change/.../BaseCurrencyChangeDialog.vue` | смена базовой валюты | Настройки › Деньги | шторка | общий | нет |
| 91 | `category-manager` | `widgets/category-manager/ui/CategoryAddDialog.vue`, `CategoryEditDialog.vue` | категория | Настройки › Категории | шторка | общий | нет |
| 92 | `enrichment-settings` | `widgets/enrichment-settings/EnrichmentSettings.vue` | настройки обогащения | Настройки | шторка | общий | нет |
| 93 | `dev-tools` | `widgets/vault-dev-tools/VaultDevTools.vue` | разработческий (только dev) | Настройки › Разработка | шторка | общий | нет |
| 94 | `hotkey-settings` | `HotkeySettings.vue` (не слой, но описывает клавиши) | — | — | — | — | — |

Строка 94 — справка, не считается слоем.

### 3.8 Вход

| # | Слой | Компонент | Внутри | Входы | ПК / телефон | Селектор | Проверка |
|---|---|---|---|---|---|---|---|
| 95 | `vault-unlock` | `features/vault-unlock/VaultUnlockDialog.vue` | пароль | карточка хранилища на `/unlock` (`data-unlock-card`) | шторка; пароль с `autofocus` (клавиатура сразу) | есть: `data-unlock-card` (карточка) | нет (экран `/unlock` в `views` не входит) |
| 96 | `vault-create` | `features/vault-create/VaultCreateDialog.vue` | новое хранилище | `Unlock` | шторка | общий | нет |
| 97 | `vault-delete` | `features/vault-delete/VaultDeleteDialog.vue` (`size=confirm`) | забыть/удалить | `RecentVaults` | шторка | общий | нет |
| 98 | `recent-vaults` | `widgets/recent-vaults/RecentVaults.vue` | список недавних; «Другое» (`data-recents-other`) | кнопка списка (`data-tour="unlock-recents"`) | ПК: `Popover`; телефон: `Drawer` (разделено по `isPhone`) | есть: `data-recents-other`, `data-tour` | нет |
| 99 | `app-settings-unlock` | `widgets/app-settings/ui/AppSettingsDialog.vue` | настройки до входа | `Unlock` | шторка | общий | нет |

## 4. Подозрительное

Отмечено чтением кода; пункты с пометкой «проба» требуют проверки в браузере.

1. **Суммы в кнопках и ссылках без телефона и клавиатуры.** `MoneyTooltipHost` реагирует только на `pointerover` мышью и пропускает касание (`MoneyTooltipHost.vue:39`); карточка суммы (`MoneyCardHost`) открывается только для сумм вне кнопки/ссылки (`money-target.ts:isPressableSum`). Итог: у суммы внутри кнопки (строка таблицы-ссылки, кошелёк в списке) на телефоне и с клавиатуры конверсии нет вообще (правило 16: «ни одного действия только по наведению»).
2. **Ctrl K и Ctrl N поверх открытого модального диалога. ИСПРАВЛЕНО на 6¾** (глобальные клавиши `useHotkey` ждут, пока открыт модальный слой `[role="dialog"][data-state="open"]`; проверка `palette-over-dialog`; было: палитра открывалась под диалогом, клики и фокус до неё не доходили). Хоткеи глобальные (`useHotkey`), палитра телепортируется в `body` вне слоя Reka; открытый `Dialog` ставит `pointer-events:none` на `body` и держит фокус — клики по палитре могут не доходить. Обратный случай: `Ctrl N` при открытом диалоге открывает второй. В `backCloses` порядка «диалог + палитра» нет.
3. **Меню без касания.** (а) `TradingChart` (`TradingChart.vue:166`): «Алерт на цене курсора» и «Рисовать» — только правым кликом; на панели есть алерт по последней цене и выбор инструмента, но алерт на произвольной цене на телефоне недоступен. (б) Закреплённые (`SidebarPinned`) видны только в боковой панели, которой на телефоне нет: закрепить можно (меню строки, виджет), увидеть и снять — нет. (в) Меню кошелька в сайдбаре (`SidebarWalletItem`, `SidebarWallets`) — только правый клик, на телефоне те же действия есть в `WalletTable` («⋯»), но не группы. (г) `ContactList` — правый клик и долгое нажатие Reka, видимой кнопки в строке нет (есть «⋯» на экране контакта). (д) `EntityLink`: правый клик, на касании действия в карточке (по задумке).
4. **Тосты и слои, которые их не знают. ЧАСТИЧНО ИСПРАВЛЕНО на 6¾** (`PopoverContent` теперь знает тост — клик по «Undo» не закрывает взгляд, `toast-over-glance`; на телефоне тост опущен под шапку, `PHONE_TOAST_TOP` в `App.vue`, «×» шторки и «‹» настроек открыты, `toast-over-sheet`; остались меню `DropdownMenu` и палитра, `onClickOutside` которой закроет её по клику в «Undo»). `keepOpenOnToast` подключён только к `Dialog`, `Sheet`, `Drawer`, `SettingsShell`. Не знают: `Popover` (взгляды, карточка суммы), меню, палитра (`onClickOutside` закроет её по клику в «Undo»). На телефоне тост сверху по центру ложится на шапку полноэкранных шторок и окна настроек (`pt-safe`) на время показа и закрывает «‹»/«×». `toastOverDialog` проверяет только окно настроек на 1280 px, не шторку и не телефон.
5. **Шкала слоёв по порядку порталов.** Dialog, Drawer, Sheet, Popover, Tooltip, Select — одинаковое `z-50`; Combobox — `z-[60]` (в `check-ui` шкала `z-*` правило 17). Поповеры внутри диалога (`AssetChip` в `TxQuick*` форме операции, `DateTimePicker`, `Select`, `Combobox`) работают только пока портал создан позже диалога; на телефоне они внутри `Drawer` со свайпом закрытия — конфликт жестов (проба).
6. **Вложенные слои. ПРОВЕРЕНО на 6¾ для диалога над шторкой** (`dialog-in-sheet`: вопрос об удалении над карточкой цели, первый Esc/«Назад» закрывает вопрос, второй — карточку; ждать конец анимации). Взгляд-`Popover` -> диалог по-прежнему не проверен. Диалог из шторки (`GoalDelete` в `GoalDetailsSheet`, `TransactionCreate` из `TransactionDetailsSheet`, `AutomationAmountDialog` из взгляда). Взгляд-`Popover` открывает диалог: закрытие взгляда перед диалогом не проверено (проба). `EntityHost` оставляет `GoalDialog/BudgetDialog/PriceAlertDialog` смонтированными вместе с карточкой.
7. **Тур и диалоги.** Автозапуск тура пропускается, если открыт `[role="dialog"]` (`use-shell-state.ts:45`); палитра (`role="dialog"` без `data-state`) и тур поверх нее вместе не проверены (проба).
8. **Слои без селектора. ЗАКРЫТО на 6¾** (проп `overlay` -> `data-overlay` у `Dialog`, `FormDialog`, `SheetContent`, `PopoverContent`, `Glance`; у `DrawerContent` проп называется `layer`, потому что `overlay` там уже флаг подложки; расставлен у ~105 слоёв; `$attrs` `Dialog` по-прежнему не пробрасывается). Ниже — исходное описание. Все ~60 диалогов различимы только текстом заголовка; у `Popover` нет ничего, кроме `data-slot="popover-content"`; у `Dialog.vue`/`Drawer`/`FormDialog` атрибуты `$attrs` не пробрасываются (`DialogContent`/`DrawerContent` без `v-bind="$attrs"`), поэтому `data-*` на `<Dialog>` в шаблоне пропадает.
9. **Адресные входы без проверки. ЧАСТИЧНО ЗАКРЫТО на 6¾** (проверяются `?settings=`, `?whatsNew=1`, `?new=blank|salary|save_share|reminder|alert|limit`, `?edit=`, `?open=`, `?category=`, `?tour=`; `?edit=` на `/automations` оказался мёртвой ссылкой — его разбирал только список в настройках кошелька — исправлено, разбор в `NewAutomation`). Исходно: `?new=reminder|alert|salary|save_share|blank|limit`, `?edit=`, `?whatsNew=1`, `?tour=` не открываются ни одним скриптом; `?new=` снимается `router.replace` — повторный заход в тот же адрес не откроет слой (по задумке, но не проверено).
10. **Телефон и клавиатура.** `Dialog` на телефоне не фокусирует поле (экранная клавиатура), кроме `autofocus` — безопасно; но `FormDialog` в режиме `phoneSubmitOnly` и `Ctrl Enter` «сохранить и дальше» (`data-quick-add-keys`) без кнопки — смотреть конкретные формы (проба).
11. **Диалог без Esc/Назад. Палитра: не воспроизводится после п. 2** (палитра не открывается поверх модального слоя). Исходно:  Не найдено: все слои на Reka или регистрируют `pushBackLayer`. Исключение по устройству: палитра (свой `window keydown`, закрывается и при Esc, адресованном Reka-слою под ней — оба закроются одним нажатием; проба).
12. **Окно настроек на `/unlock`. ПРОВЕРЕНО на 6¾** (`app-settings-unlock`: открывается, Esc/«Назад» закрывают). Исходно:  `AppSettingsDialog` там — отдельный `Dialog`, не `SettingsShell`: `backCloses` его не охватывает.

## 5. Цифры

Подсчёт по таблицам раздела 3 (строка = слой или связка слоёв одного компонента; строка 94 справка и не считается):

- слоёв всего: 98 строк. Фактических компонентов-слоёв больше: в ~40 строках по 2–10 (например, #48 — 7 диалогов), всего около 120.
- без собственного селектора слоя: 66 строк из 98 — 52 («общий» `[role="dialog"]`: 42, «нет»: 10) и ещё 14, где есть лишь `data-tour` на кнопке-входе, а не на слое.
- с селектором слоя или его части: 32 строки (карточки сущностей, взгляды, палитра, окно настроек, шторка «Ещё», карточка суммы, тосты, меню `data-context-menu`).
- уже проверяются автотестом (да): быстрое добавление, окно настроек, палитра, тост — 4 строки; ещё 8 строк покрыты `check-links` (карточки и взгляды); остальные 86 — нет.
- подозрительных мест: 12 (раздел 4), из них 3 — прямое нарушение правил 16/17 (пункты 1, 3, 4).

### Топ-10 подозрительного

1. Сумма внутри кнопки/ссылки: конверсии нет на касании и с клавиатуры (`MoneyTooltipHost.vue:39`).
2. Ctrl K/Ctrl N над открытым диалогом: палитра в `body` под `pointer-events:none` (проба).
3. Закреплённые видны только в боковой панели, на телефоне не показываются и не снимаются.
4. Меню графика (`TradingChart.vue:166`) — только правый клик; «алерт на цене курсора» недоступен на касании.
5. Тосты не знают `Popover`/меню/палитру: клик по «Undo» закрывает слой.
6. Тост сверху на телефоне закрывает шапку полноэкранных шторок и «‹» настроек; `toastOverDialog` его не ловит.
7. Поповеры/селекты внутри `Drawer` на телефоне: жесты свайпа против прокрутки списка, порядок слоёв держится на `z-50`/`z-[60]`.
8. 66 из 98 слоёв без `data-*`; `Dialog`/`Drawer` не пробрасывают `$attrs`.
9. `?new=…`, `?edit=`, `?whatsNew=1`, `?tour=` не проверяются; `AppSettingsDialog` на `/unlock` вне `backCloses`.
10. Меню кошелька в сайдбаре и контактов — правый клик без видимой кнопки (группы кошельков на телефоне без меню).

## 6. Предложение: `scripts/check-layout/overlays.mjs`

**Сделано на этапе 6¾.** Отличия от наброска ниже: проп `overlay` стоит на содержимом (`SheetContent`, `PopoverContent`), у `DrawerContent` он называется `layer`; список записей вынесен в `overlay-list.mjs`, шаги — в `overlay-steps.mjs`, «слой над слоем» — в `overlay-cross.mjs`. К шагам добавлены `context` (правый клик) и `hover`, а к записи — `phoneOpen`, `fresh`, `locked`, `cleanup`, `keys`, `centred` (см. шапку `overlay-list.mjs`). Для каждой записи и размера цикл: открыть, дождаться слоя, замерить (за экраном, прыжок центра у центрированных диалогов, сдвиг после остановки, фокус внутри слоя, переполнение и цели касания ≥ 40 px только внутри слоя — `window.__scope` в `probes.mjs`), закрыть Esc (ПК) или `closeTopLayer()` (телефон), проверить, что слой закрылся, адресный ключ снят и страница под слоем не изменила размеров. В `--quick` идёт 1280 и 390 в одной (тёмной) теме; `--overlays[=id,id]` гоняет только слои. Исходный набросок:

1. В примитивах `Dialog`, `FormDialog`, `Sheet`, `Drawer`, `Popover`, `Glance` добавить необязательный проп `overlay` и класть его в `data-overlay` на корень контента (`DialogContent`, `DrawerContent`, `SheetContent`, `PopoverContent`). Одно изменение даёт селектор для всех слоёв без правки их шаблонов, а в вызывающем месте остаётся одно слово: `<Dialog overlay="goal" …>`. Для уже имеющихся готовых селекторов (`data-tx-card`, `data-command-palette`…) ничего менять не нужно, `layer` указывает на них.
2. Список слоёв — данные, обход — общий цикл: для каждой записи и размера окна открыть, измерить, закрыть (Esc), «Назад» (`closeTopLayer`), проверить, что страница под слоем не сдвинулась.

Формат записи:

```js
// scripts/check-layout/overlays.mjs
// id        — совпадает с data-overlay (и с номером в docs/ui-overlays.md)
// route     — страница, на которой слой открывается (подстановки: {store:<kind>}, {goal}, {tx})
// open      — шаги: { click: селектор } | { key: 'Ctrl+K' } | { query: {…} } | { eval: код }
// layer     — селектор открытого слоя
// phone     — true: открывать и на 390/360 (иначе только ПК); 'only' — только телефон
// back      — по умолчанию true: closeTopLayer() закрывает слой
// closes    — необязательно: 'esc' | 'back' | 'button' для явной проверки способа
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

Что делает цикл на каждую запись и размер: открыть шаги; дождаться `layer` видимым; замерить прыжок (как `dialogJump`), переполнение и цели касания внутри слоя; Esc (ПК) или `closeTopLayer()` (телефон) закрывает; адресный слой убирает свой ключ; страница под слоем не изменила размеры. Отдельные сквозные записи: «слой над слоем» (диалог в шторке, палитра над диалогом, тост над шторкой на телефоне) — для пунктов 2, 4, 6 раздела 4.

Порядок внедрения: сначала `data-overlay` в примитивах, затем 10–15 записей (разделы 3.1 и по одному слою на раздел), потом остальные по таблице; запись без `layer` не создаётся — это и есть критерий «слой без селектора».

## 7. Покрытие `overlays.mjs` (этап 6¾)

Записей 55 (1280: все, кроме `phone: 'only'`; 390: те, где `phone`) и 4 сквозных: `dialog-in-sheet` (п. 6), `palette-over-dialog` (п. 2), `toast-over-sheet`, `toast-over-glance` (п. 4). Покрыто строк описи: 57 из 98 (3.1: 22 из 23, 3.2: 4 из 6, 3.3: 8 из 12, 3.4: 4 из 17, 3.5: 6 из 12, 3.6: 7 из 16, 3.7: 3 из 7, 3.8: 3 из 5); 41 строка осталась без проверки. `data-overlay` стоит у всех ~105 диалогов, шторок и поповеров с примитивами выше; запись в списке можно добавить одной строкой там, где слой удаётся открыть без особого состояния.

`sync-plan` идёт только на 390 (`phone: 'only'`): закрытие окна — отказ, мок пишет строку в журнал и вид «История» на телефоне после этого сдвигается при загрузке; на 1280 тот же компонент закрыт проверкой `sync-pairing` и размерами окна. Порядок прогона: ПК-слои после ПК-обхода, телефонные — в конце, после обхода телефона.

Запись `id` совпадает с `data-overlay` (у слоёв, где селектор был раньше, — с ним: `[data-goal-card]`, `[data-command-palette]`, `[data-context-menu]`, `.driver-popover`, `[role="menu"]`).

Что осталось без проверки и почему:

| Строки | Слой | Причина |
|---|---|---|
| 16, 57, 70 (ПК) | `money-tooltip`, `chart-tooltips`, `report-term` | `Tooltip` Reka: нет пропа `overlay`, открывается только наведением и закрывается уходом мыши, Esc/«Назад» его не касаются |
| 28, 95 | `home-store-create`, `vault-unlock` | нужно особое состояние: пустой кошелёк-мастер на Главной; хранилище с паролем (в мок-хранилище пароля нет, «Открыть» входит сразу) |
| 32, 34, 62-65, 74-75, 78-79, 82 | `tx-link`, `tx-bulk-pick`, `goal-delete` (есть в сквозной), `plan-calculator`, `trade-record`, `emergency-fund`, `automation-simulate`, `automation-confirm`, `template-dialog`, `source`, `workflow-leave` | вход — несколько шагов через меню карточки/выбор строк/уход из редактора; `data-overlay` есть, записи нет |
| 35, 67 | `tx-export`, `report-export` | `DropdownMenu` без пропа `overlay` и без общего селектора |
| 38, 45-46, 49-56, 58 | `debt-terms`, `sidebar-wallet-menu`, `sidebar-pinned-menu`, `asset-details`, `utility`, `vehicle`, `property-value`, `asset-class`, `attachments`, `exchange-menu`, `chart-menu`, `wallet-settings-dialogs` | нужен кошелёк особого вида (долг, актив, авто), закреплённое или развёрнутая группа боковой панели; правый клик по графику |
| 43 | `store-delete` | пункт «Удалить» неактивен у кошельков с операциями, а в моке все такие |
| 69, 83-86 | `calc-scenarios`, `workflow-toolbar-menus`, `workflow-mobile-bar`, `workflow-inspector`, `workflow-add-node` | холст схем (vue-flow), вход — тап по узлу/двойной тап |
| 87 (set/remove), 89 (send), 90, 92, 93 | пароль: задать/снять, `vault-send`, `base-currency`, `enrichment-settings`, `dev-tools` | вход зависит от состояния (есть ли пароль), от выбора в списке валют или от режима разработки |
| 97 | `vault-delete` | нужен список недавних с записью, которую можно забыть |

Найдено и исправлено проверкой (все — реальные дефекты приложения):

1. `MoneyTooltipHost`: подсказка конверсии открывалась на фокусе любой панели, в которой есть сумма (телефон: конвертер, `focusFirstField` фокусирует панель); подсказка становилась верхним слоем и «Назад»/Esc закрывали сначала её. Теперь на фокусе реагирует только сама кнопка/ссылка с суммой (`sumOfFocus`).
2. `MoneyCardHost`: фокус не попадал в карточку суммы на ПК (Reka шлёт событие обёртке поповера), клавиша `C` не работала. Фокус теперь на самой карточке.
3. `Dialog`: `focusFirstField` брал скрытое `<input type="file" class="hidden">` и фокус терялся (импорт схемы). Берётся первое нарисованное поле.
4. `SettingsShell`: `@open-auto-focus.prevent` оставлял фокус вне окна настроек; фокус теперь на самом окне (поле не берёт, клавиатура на телефоне не выезжает).
5. `ActionMenu`: `class` вызывающего («relative z-10» в `WalletTable`) терялся, «⋯» оказывалась под растянутой ссылкой строки — клик открывал кошелёк, а не меню. Класс теперь у кнопки.
6. `NewAutomation`: `?edit=<id>` на `/automations` (ссылка «Изменить» из карточки автоматизации) ничего не открывала. Форма открывается и ключ снимается при закрытии.
7. `StoreNodeMultiSelect`: цели касания флажков 38x32. Строки с `touch-hit`.
8. Глобальные клавиши над модальным слоем (п. 2 раздела 4), `Popover` и тост, тост над шапкой на телефоне (п. 4).
