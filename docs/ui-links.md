# Связи интерфейса: куда ведёт каждая сущность и как вернуться

Этап 6½ «Проверка связей». Цель: из любого места за один-два шага дойти до связанной сущности или страницы и вернуться назад.
Документ — инвентаризация на момент ветки `feature/ui-redesign`; код не менялся. Строки указаны по состоянию рабочей копии.

Обозначения:
- **Взгляд** — быстрый обзор на месте (`Glance`: поповер на ПК, шторка на телефоне), синий пунктир на холсте.
- **Ссылка** — `EntityLink` (клик открывает карточку или страницу, правый клик — меню действий).
- **Страница** — именованный маршрут.
- **Карточка** — слой поверх экрана по адресу `?open=kind:id~kind:id` (стопка, верхняя видна).
- **Текст** — имя показано просто текстом, ничего не нажимается.
- **Холст** / **сверх холста** — есть ли это на кадре `Full-*.dc.html` (Paths-2 · 8–11, правила «Пути 1» в `docs/ui-redesign-vs-canvas.md`) или придумано в коде.

## 1. Механизмы

| Механизм | Файлы | Что делает | Назад |
|---|---|---|---|
| `entityLocation(target, query)` | `src/shared/lib/entity-link.ts` | Для `store`, `workflow`, `scenario` отдаёт страницу (`entityPage`): `wallet`, `workflow-edit`, `money-calculator?scenario=`. Для остальных кладёт цель в `?open=` текущего адреса (стопка до 8, повтор верхней не пушится) | Закрыть/«Назад» карточки: `router.replace` без верхней; браузерная «Назад» тоже снимает (открытие — `push`) |
| `EntityLink` | `src/shared/ui/entity-link/EntityLink.vue` | `RouterLink` на `entityLocation` + значок вида + правый клик (меню `entityActions` + «Закрепить»). Единственный DOM-признак: `data-entity="kind:id"` | см. выше |
| Хост карточек | `src/pages/app-shell/ui/entity-host/*` | Рисует верхнюю карточку: `tx`, `goal`, `automation` — свои шторки, остальные виды — общий `EntitySheet` (`get_entity_card`, группы связей `EntityRelations`) | Кнопка «← Назад» есть только у `EntitySheet` (`canGoBack`); у шторок операции, цели, автоматизации — только крестик/Esc, он тоже снимает верхнюю карточку |
| `entityActions` | `src/shared/lib/entity-actions.ts` | Одна таблица действий вида: карточка, меню ссылки (потом палитра) | — |
| `openEntity()` | `entity-link.ts:94` | Открыть карточку из кода (меню строки операции, `openTxDetails`) | — |
| `Glance` | `src/shared/ui/glance/Glance.vue`, `widgets/quick-glance` | Кошелёк (`WalletGlance`), капитал (`CapitalGlance`), «Потрачено» (`SpentGlance`), «авто» (`AutomationGlance`); тело категории и кошелька в Ctrl K (`CategoryGlanceBody`, `WalletGlanceBody`). «Открыть →» ведёт в `openTo` | Esc, клик в сторону; на телефоне «Назад» закрывает шторку. Страница под взглядом не двигается |
| Меню строки операции | `features/transaction-row-menu/use-tx-row-menu.ts`, `shared/ui/action-menu` | «Быстрый взгляд», «Изменить», «Повторить», «Похожие», «Кошелёк X», «Контакт Y», «Удалить». Кошелёк и контакт — `openEntity` | Карточка закрывается |
| `BackChip` | `src/shared/ui/back-chip`, `drill.ts` | «← Траты · октябрь»: `?back=<путь>&backLabel=<подпись>` рисует `Transactions.vue:77`. Только на списке операций | Esc и Alt ← |
| `drillTo()` | `src/shared/lib/drill.ts` | Операции по цифре: период, категория, кошельки, виды. Чип «назад» кладёт только вызов с `back:` | — |
| Окно настроек | `shared/lib/settings-link.ts`, `widgets/settings-shell` | `?settings=<группа>&section=<раздел>`: слой над любым экраном | Закрыть окно убирает ключи из адреса |
| Уведомления | `shared/lib/notice-link.ts` | `link` из ядра: имя маршрута, `имя:id`, путь, `open:kind:id` | — |
| Меню трея | `crates/app/src/tray.rs`, `use-shell-state.ts:92` | Событие `tray-navigate` с именем маршрута | — |

### Известные кинды `?open=`

`store, tx, category, contact, goal, budget, automation, workflow, reminder, alert, asset, scenario` (`ENTITY_KINDS`). Хост обрабатывает `?open=` один раз, в `AppShell`. Ядро (`relations.rs`) умеет карточку для каждого; `automation` и `workflow` ищутся одним запросом (id общий). Для `store`, `workflow`, `scenario` карточка никогда не получается из `entityLocation` — только если набрать адрес руками.

Прочие обработанные параметры адреса, на которые ведут ссылки: `?edit=<id>` (автоматизации; `use-new-from-route.ts:31`), `?new=salary|blank` (автоматизации), `?new=limit&category=` (`PlanLimits.vue:80`), `?category=` (Траты, `SpendingCategories.vue:78`), `?contact=` (`TransactionsContacts.vue:49`), `?scenario=` (`MoneyCalculator.vue:19`), `?tab=`/`?view=` (кошелёк, `use-wallet-view.ts:65`; настройки кошелька, `use-settings-groups.ts:57`), `?program=` (бонусы), `?store=,from=,to=,cat=,kind=,q=,...` (список операций, `tx-filter.ts`). Все найденные значения `settings`/`section` в коде существуют в `SETTINGS_GROUPS`.

## 2. Таблица: сущность, где упоминается, как показана, куда ведёт, путь назад

Столбец «Холст»: **да** — есть на кадре, **нет** — сверх холста.

| Сущность | Где упоминается | Как показана | Куда ведёт | Путь назад | Холст |
|---|---|---|---|---|---|
| **Операция** | Списки (Операции, Главная «Что изменилось», кошелёк, календарь, контакт, категория, биржа) | Строка: клик/Enter по прозрачной кнопке `data-tx-open` | Карточка `?open=tx:id` | Крестик, Esc, Назад браузера | да |
| | Карточка цели/автоматизации, «Похожие», палитра, закрепления, уведомления | `EntityLink`, `RouterLink` на `entityLocation`, `open:tx:` в notice | Карточка | то же | нет |
| | Строки операций в карточке категории/кошелька (`relations`) | `EntityLink` | Карточка | кнопка «← Назад» в `EntitySheet`, есть только если стопка >1 | нет |
| **Кошелёк / субсчёт** | Строка операции (откуда → куда) на ПК | Взгляд `WalletGlance` (слот `#side`: `Transactions.vue:128`, `HomeChanges.vue:71`, `EntityHost.vue:20`) | Поповер, «Открыть →» на `/stores/:id` | Esc; со страницы — браузер назад / сайдбар | да |
| | Строка операции на телефоне | Текст (намеренно: цель касания 16 px) | Карточка операции, там взгляд | — | да (оговорено в `ui-redesign-vs-canvas.md:219`) |
| | Строка операции: контакт / категории / списки без слота `#side` | Текст (см. находки) | — | — | нет |
| | Меню строки «Кошелёк X» | Пункт меню, `openEntity` -> страница `wallet` (для `store` это страница) | `/stores/:id` | Назад браузера | да |
| | Сайдбар, таблица и список «Кошельки», `StoreRowLink`, `StoresView`, `AssetElsewhere`, `FixedCosts`, `DashboardAllocation` | Сырой `RouterLink` на `wallet` (без `data-entity`) | `/stores/:id` | Назад браузера | нет (только сайдбар и таблица) |
| | `DebtsCard`, `FundCard`, `GoalList`, `GoalFacts`, `HealthLight`, цели в «Деньги» | `EntityLink` | `/stores/:id` | Назад браузера | нет |
| | Капитал на Главной/Кошельках | Взгляд `CapitalGlance` | `money-wealth` | Esc | да |
| | Страница кошелька | Хлебные крошки `PageHeader` **только если у кошелька есть родитель** (`WalletHeader.vue:74`) | «Кошельки» -> родитель | у корневого кошелька крошек нет (находка 20) | нет |
| | Настройки кошелька `/stores/:id/settings` | Крошки «Кошельки › Имя › Настройки» | кошелёк | крошки | нет |
| **Категория** | Строка операции (бейдж) | Текст (находка 4) | — | — | нет |
| | Карточка операции | `EntityLink` | Карточка `category` -> «Операции» (`cat=`), «Задать лимит», «Лимиты» | Кнопка «← Назад» | нет |
| | `SpentGlance` | Ссылка `data-glance-category` на `drillTo(cat, back)` | Операции с чипом «← <экран>» | `BackChip` | да |
| | Ctrl K | Взгляд `CategoryGlanceBody` + Enter «Открыть в Тратах» | `money?category=` (строка категории в текущем периоде, иначе ничего не откроется) | Esc | да |
| | Отчёт «Категории» `CategoriesView.vue:61,103`, `LossSummary` (по кошельку/контакту) | `RouterLink` на `ofCategory` / фильтр Операций | Операции | без чипа (находка 22) | нет |
| | `MonthGrowth` | `EntityLink` | Карточка | «← Назад» | нет |
| | «Топ-категория» обзора, лимиты Главной, список лимитов | Текст (находки 6, 7) | — | — | нет |
| **Контакт и долг** | Строка операции | Текст (находка 4); в меню «Контакт Y» (`openEntity`) | Карточка `contact` -> «Операции», «Контакты» | «← Назад» | да (меню) |
| | `TxFacts.vue:81` (карточка операции) | `EntityLink` | Карточка | | нет |
| | Страница «Контакты» | Список, справа карточка контакта с долгом и действиями (вернуть, дать, взять) | `?contact=<id>` | На телефоне «‹» (`TransactionsContacts.vue:93`) | да |
| | Долг | Показан только в карточке контакта (`ContactDebtPhrase`, `DebtTermsPopover`); в карточках операций бейдж «в долг/вернул» — текст | — | — | нет |
| **Цель** | Главная (`DashboardGoals`), «План» (`PlanGoals`), `GoalList`, `GoalFacts` | `EntityLink` | Карточка `goal` (своя шторка `GoalDetailsSheet`) -> «Цели» (`money-plan`) | Крестик, Esc | нет |
| | Карточка кошелька (группа «Цели») | `EntityLink` | Карточка | «← Назад» | нет |
| | Действие «Новая цель» из карточки кошелька | Диалог, `store` выбран | — | Отмена | нет |
| **Лимит** | `BudgetList`, `PlanLimits`, `DashboardBudgets` | Строка и название — текст (находки 6, 7); сам `EntityLink` на `budget` есть только в группах связей карточек | Карточка `budget` -> «Лимиты» | «← Назад» | нет |
| | `SpentGlance` «Задать лимит» | Кнопка | `money-plan?new=limit` | Назад браузера | да |
| | «Задать лимит на „Продукты“» в Ctrl K | Действие палитры | `money-plan?new=limit&category=` | то же | да |
| **Автоматизация и схема** | Строка «авто» в списке операций | Взгляд `AutomationGlance` (слот `#origin`) | «Автоматизация →» (карточка `automation`) или «Схема →» (страница `workflow-edit`) | Esc | да |
| | Журнал `JournalTable.vue:42` | `RouterLink` на `entityLocation` если запись открывается, иначе текст | Карточка | Крестик | да (доска «Автоматизации») |
| | «Мои» (`MineTable`, `MineList`) | Строка-ссылка на `entityLocation` | Карточка | Крестик | да |
| | `WorkflowRuns.vue:51`, `SourceRow.vue:82` | `EntityLink` на `workflow` | `/automations/workflows/:id` | Крошка «Автоматизации ›» в `EditorToolbar:64` — всегда ведёт на «Мои», а не туда, откуда пришли | нет |
| | `WorkflowQueue`, `RunPanel` | Название схемы — текст (находка 9) | — | — | нет |
| | Карточка автоматизации | Кнопки «Изменить» (`automations?edit=`) и «В редактор» (`workflow-edit`, id автоматизации = id схемы) | | | да |
| **Напоминание** | `ReminderList`, окно операции, «Дела» на Главной | Список и группы; `EntityLink` только в группах связей | Карточка `reminder` -> «Напоминания» (список без подсветки) | «← Назад» | нет |
| **Оповещение о цене** | `PriceAlertList` | Тикер — текст (находка 10) | Карточка `alert` -> «Оповещения» | | нет |
| **Актив / валюта** | Строка операции (`RUB → USDT`) | Текст | — | — | нет |
| | Главная «Активы» (`DashboardAssets`), `AssetBaskets`, `SourceRow` | `EntityLink` `asset` | Карточка: «Новое оповещение», «Оповещения» (на страницу актива/кошелька с позициями выхода нет — находка 17) | «← Назад» | нет |
| | Позиции кошелька | Строка открывает шторку `AssetDetailsSheet` (операции с `q=`, `money-investments?store=`) | Ссылки без чипа «назад» | Крестик | нет |
| | Любая сумма `<Money>` | Клик — `MoneyCard` (в базовой валюте и избранных), «Все курсы» -> `?settings=accounting&section=rates` | Окно настроек | Закрыть окно | да (Currency-1 · 5) |
| **Отчёт и период** | Цифра «Потрачено», «Куда ушли», календарь, график | `drillTo` -> Операции; чип «← Куда ушли», «← Траты · октябрь» только там, где вызывающий передал `back` | Операции | `BackChip` (находка 22) | да |
| | Период | Чип периода с поповером (`PeriodPicker`), `?from=&to=` | Тот же экран | Назад браузера | да |
| | Отчёты «Деньги» | Вкладки `/money/*` (`route-tabs`) | | Вкладки | да |
| | Окно «Инфляция» из отчёта | `?settings=accounting&section=inflation` (вкладка «Учёт › Деньги») | | Закрыть окно | нет |
| **Хранилище / устройство** | Настройки: «Хранилище», «Синхронизация» (устройства, история, передача), «Приложение» (`device-app`) | Разделы окна настроек; из других экранов ссылки только на `app/device-app` (`NotificationDetails.vue:124`) и `sync` (уведомления ядра) | Окно настроек | Закрыть окно | да (Settings-*) |
| | Недавние хранилища на экране входа (`RecentVaults`), `SyncDevices` (`peer.name`), `SendVaultPick` | Текст/списки внутри окон | — | — | да |
| | Сущности `device` и `vault` в `ENTITY_KINDS` нет: у них нет ни карточки, ни `data-entity` | | | | |

## 3. Что нажимается и что нет: перечень «некликабельных» мест

Это места, где показано имя сущности, а `EntityLink`/взгляда нет (и вся строка не является ссылкой на ту же сущность). Строки списка, где имя — заголовок самой строки, ведущей на свою страницу, сюда не входят.

| № | Сущность | Файл:строка | Контекст |
|---|---|---|---|
| 1 (закрыто, этап 6½) | Контакт | `entities/transaction/ui/transaction-row/TransactionRow.vue:196` | Вторая строка: `· {{ contactName }}` |
| 2 (закрыто, этап 6½) | Категория | `TransactionRow.vue:165` | Бейдж категории |
| 3 (закрыто, этап 6½) | Актив | `TransactionRow.vue:152` | `RUB → USDT` |
| 4 | Кошелёк | `ContactDetails.vue:198`, `CategorySheet.vue:92`, `SpendingCalendar.vue:179`, `StoreTransactions.vue:71`, `ExchangeImported.vue:46`, `TransactionDetailsSheet` без хоста | `TransactionsList`/`TransactionRow` без слота `#side` и `#origin`: «откуда → куда» и «авто» — текст |
| 5 | Кошелёк | `entities/transaction/ui/tx-preview/TxPreviewCard.vue:69` | Превью в форме (допустимо: форма) |
| 6 | Категория | `widgets/analytics-overview/AnalyticsOverview.vue:75` | «Главная категория» |
| 7 | Категория, кошелёк | `widgets/dashboard-budgets/DashboardBudgets.vue:43`, `widgets/budget-list/BudgetList.vue:95` | Цель лимита: «Продукты · месяц» |
| 8 | Кошелёк (недвижимость) | `widgets/utilities-due/UtilitiesDue.vue:78` | Имя квартиры в «ЖКХ скоро» |
| 9 | Кошелёк | `widgets/cashback-bonuses/CashbackBonuses.vue:51,63` | Источник и программа |
| 10 | Кошелёк | `widgets/bonus-programs/ui/ProgramList.vue:55`, `ProgramTable.vue:48` | Имя программы (строка выбирает программу на месте, это допустимо) |
| 11 (закрыто, этап 6½) | Кошелёк | `features/automation-simulate/AutomationSimulationDialog.vue:120` | Остатки в симуляции (диалог) |
| 12 | Схема | `widgets/workflow-queue/WorkflowQueue.vue:48,76,97`, `widgets/workflow-editor/ui/RunPanel.vue:75`, `widgets/workflow-runs/WorkflowRuns.vue:41` | Название схемы текстом (в `WorkflowRuns:51` рядом есть `EntityLink` на ту же схему, дубль) |
| 13 (закрыто, этап 6½) | Оповещение, актив | `widgets/price-alert-list/PriceAlertList.vue:79` | Тикер в строке оповещения |
| 14 (закрыто, этап 6½) | Автоматизация | `pages/automations/ui/journal/JournalTable.vue:47` (`opens()` ложно), `JournalList.vue:26` | Имя записи журнала, когда карточка недоступна |
| 15 (закрыто, этап 6½) | Контакт | `entities/contact/ui/contact-turnover/ContactTurnoverTable.vue:18` | Актив в обороте (там одни активы) |
| 16 (закрыто, этап 6½) | Актив | `pages/wallet/ui/wallet-assets/WalletAssets.vue:68,106`, `pages/stores/ui/WalletAssets.vue:34` | Тикер; строка при этом открывает `AssetDetailsSheet` (допустимо, но без `data-entity`) |
| 17 (закрыто, этап 6½) | Кошелёк, контакт | `widgets/loss-summary/LossSummary.vue:96` | Ссылка на журнал с фильтром `store=`/`contact=`, не на сущность |
| 18 (закрыто, этап 6½) | Кошелёк | `entities/store/ui/store-card/StoreCard.vue:84`, `StoreRowLink.vue:21`, `SidebarWalletItem.vue:43,72`, `WalletTable.vue:105`, `WalletList.vue:25`, `StoresView.vue:32`, `AssetElsewhere.vue:49`, `FixedCosts.vue:56`, `DashboardAllocation.vue:125` | Рабочие ссылки, но сырой `RouterLink`: нет `data-entity`, нет правого клика, нет «Закрепить» |

Итого: **13 видов сущностей** в таблице раздела 2 (операция, кошелёк/субсчёт, категория, контакт и долг, цель, лимит, автоматизация, схема, напоминание, оповещение, актив/валюта, отчёт и период, хранилище/устройство); в коде: `EntityLink` — 23 вызова в 17 файлах, 4 вида взгляда (кошелёк, капитал, «Потрачено», «авто») на 8 страницах, около 18 сырых `RouterLink` на страницы сущностей (п. 18). Простым текстом имя показано в **11 местах/группах** (пп. 1–4, 6–9, 12, 13, 14 таблицы; пп. 5, 10, 11, 15, 16 — допустимые).

## 4. Находки

Тип: **Б** — сломано (кнопка не работает), **Т** — некликабельный текст, **В** — выход со взгляда/карточки, **Н** — нет пути назад, **Д** — предложение по DOM. Столбец «Холст»: да/нет.

### Сломанные цели

| № | Тип | Где | Что не так | Предложение | Холст |
|---|---|---|---|---|---|
| 1 | Б | `crates/app/src/tray.rs:27,33`; обработчик `use-shell-state.ts:94` (`router.push({ name: target })`) | Пункты трея «Аналитика» и «Планирование» шлют `analytics-cashflow` и `planning-automations`. Таких маршрутов нет, `router.push` бросает «No match for» | Заменить на `money` и `automations` | нет |
| 2 | Б | `crates/core/src/service/notify_snapshot.rs:121,180`, `crates/app/src/scheduler.rs:623,645,673,781`, `crates/core/src/service/datasources.rs:535,563`, `crates/core/src/service/health.rs:425` (+ тест `health.rs:741`) | Уведомления ядра ведут на `planning-alerts`, `planning-reminders`, `planning-automations`, `planning-sources`, `analytics-health`. В `notice-link.ts:21` это превращается в `{ name }` и кнопка «Открыть» в колокольчике (`NotificationBell.vue:53`) и в тосте (`use-backend-notices.ts:89`) падает. Демо-данные (`mock/demo.ts:256,266`) используют пути, поэтому в браузере с моком ошибка не видна | В ядре: `automations-alerts`, `automations-reminders`, `automations`, `automations-sources`, `money-health`; тест поправить. Либо перевести имя в `noticeRoute` (таблица старых имён), чтобы не менять сохранённые уведомления | нет |
| 3 | Б | `crates/core/src/service/inflation.rs:146` | Ссылка `/?settings=inflation`: `inflation` — раздел, не группа; `resolveSettingsTarget` откроет первую группу «Приложение». Верно: `/?settings=accounting&section=inflation` (так уже зовёт `MoneyToolbar.vue:105`) | Исправить строку (и, при желании, в `noticeRoute` принять `settings=<раздел>` как раздел) | нет |
| 3a (закрыто, этап 6½) | Б (низкий) | `crates/core/src/model/fixtures.rs:470` | Тестовый `/planning` — старый путь; неизвестный путь ведёт на главную (`router/index.ts`: `/:pathMatch(.*)*`) | Заменить на `/automations` | нет |

### Некликабельные упоминания (сделать EntityLink)

| № | Тип | Где | Что не так | Предложение | Холст |
|---|---|---|---|---|---|
| 4 (закрыто, этап 6½) | Т | `TransactionRow.vue:165,196,152` | Категория, контакт, актив в строке операции — текст; в меню есть кошелёк и контакт, а категорий и актива нет | Контакт и категория: вынести в слот или `EntityLink plain` с `pointer-events-auto relative` (как у `#side`); меню: пункты «Категория X», «Актив» по `openEntity`. Строка уже имеет прозрачную кнопку-оверлей, цели касания на телефоне — в карточке | нет (в меню холста — «Похожие», кошелёк, контакт) |
| 5 (закрыто, этап 6½) | Т | `ContactDetails.vue:198`, `CategorySheet.vue:92`, `SpendingCalendar.vue:179`, `StoreTransactions.vue:71`, `ExchangeImported.vue:46` | `TransactionsList` без слотов `#side`/`#origin`: «откуда → куда» и «авто» не нажимаются, хотя в тех же строках на «Операциях» нажимаются | Один общий слот-обёртка внутри `TransactionsList` (`WalletGlance`/`AutomationGlance` по умолчанию), чтобы страницы не копировали одни и те же шаблоны (есть в 3 местах) | да (кадр Paths-2 · 10 указывает «Откуда: Что изменилось, Операции, кошелёк») |
| 6 (закрыто, этап 6½) | Т | `AnalyticsOverview.vue:75`, `DashboardBudgets.vue:43`, `BudgetList.vue:95` | Категория/цель лимита — текст | `EntityLink` `category` (или `budget` для всей строки лимита) | нет |
| 7 (закрыто, этап 6½) | Т | `UtilitiesDue.vue:78` | Квартира в «ЖКХ скоро» — текст | `EntityLink` `store` | нет |
| 8 (закрыто, этап 6½) | Т | `CashbackBonuses.vue:51,63` | Источник и программа бонусов — текст | `EntityLink` `store` (`row.store_id`) | нет |
| 9 (закрыто, этап 6½) | Т | `WorkflowQueue.vue:48,76,97`, `RunPanel.vue:75`, `WorkflowRuns.vue:41` | Название схемы — текст, при этом в `WorkflowRuns.vue:51` дубль-ссылка | Заменить первый `<span>` на `EntityLink workflow`, убрать дубль | нет |
| 10 (закрыто, этап 6½) | Т | `PriceAlertList.vue:79` | Тикер оповещения — текст | `EntityLink` `alert` или `asset` | нет |
| 11 | Т | `JournalTable.vue:47`, `JournalList.vue:26` | Имя записи без ссылки, когда `opens()` ложно | Оставить текст; в подсказке объяснить («схема удалена»)  | да |
| 12 (закрыто, этап 6½) | Т | `LossSummary.vue:96` | Название кошелька/контакта ведёт на журнал, а не на сущность | Пометить как «журнал по ...» или добавить `EntityLink` рядом | нет |

### Взгляды и карточки без выхода

| № | Тип | Где | Что не так | Предложение | Холст |
|---|---|---|---|---|---|
| 13 | В | `Glance.vue`, `quick-glance/ui/*` | Взгляд на категорию (`CategoryGlanceBody`) и кошелёк в Ctrl K умеют «Открыть →». У `SpentGlance` и `AutomationGlance` выход есть. У взгляда на **валюту** (`MoneyCard`) выход только на «Все курсы». На **контакт, цель, лимит** взгляда нет (в `ui-redesign-vs-canvas.md:251` отмечено) | Оставить; пункт для сведения | да (отмечено) |
| 14 | В | `entity-actions.ts:71-73` (`contact`) | «Контакты» ведёт на `transactions-contacts` без `?contact=<id>`: открывается список без выбора | `to: (t) => ({ name: 'transactions-contacts', query: { contact: t.id } })` | нет |
| 15 | В | `entity-actions.ts:75,85,102-105` (`goal`, `budget`, `reminder`, `alert`) | «Цели», «Лимиты», «Напоминания», «Оповещения» ведут на список без подсветки нужной записи: из карточки лимита или оповещения нужную строку приходится искать | Добавить `?focus=<id>` в списки, либо открывать правку (как `automation` -> `?edit=`) | нет |
| 16 | В | `entity-actions.ts:66-70` (`category`) | У категории нет страницы; «Операции» + «Новый лимит» + «Лимиты». Нет «Открыть в Тратах» (`money?category=`), хотя Ctrl K её предлагает | Добавить действие «Открыть в Тратах» -> `{ name: 'money', query: { category: id } }` (работает, если категория есть в периоде) | да (Paths-2 · 8, «Открыть в Тратах») |
| 17 | В | `entity-actions.ts:106-109` (`asset`) | У актива из `EntityLink` только «Новое оповещение» и «Оповещения». Нет «Операции с активом» и «Где лежит» | Действие `operations` по активу (`q=<symbol>`), либо «Позиции» | нет |
| 18 | В | `entity-actions.ts:65` (`tx`) | `tx: []` — меню правого клика по ссылке на операцию пустое, остаётся только «Закрепить» | Добавить «Открыть кошелёк» / «Похожие» (как в меню строки) | нет |
| 19 | В | `AutomationList.vue:218`, `AutomationDetailsSheet.vue:48` | Не ошибка, но неочевидно: `workflow-edit` получает id автоматизации. Работает потому, что id автоматизации и схемы общий (`relations.rs:227`) | Комментарий или `entityPage({kind:'workflow'})` | да |

### Нет пути назад

| № | Тип | Где | Что не так | Предложение | Холст |
|---|---|---|---|---|---|
| 20 (закрыто, этап 6½) | Н | `pages/wallet/ui/wallet-header/WalletHeader.vue:74` | Крошки только у субсчёта (`ancestors.length`). Корневой кошелёк открывается без «← Кошельки» (на странице есть кнопка только в состоянии «не найден», `Wallet.vue:246`) | Показывать `PageHeader` всегда («Кошельки › Имя») | да (Wallets-2) |
| 21 (закрыто, этап 6½) | Н | `widgets/workflow-editor/ui/EditorToolbar.vue:64` | «Автоматизации ›» всегда ведёт на «Мои», даже если пришли из кошелька, из карточки схемы или с Главной. На телефоне `MobileEditorBar.vue:72` уже делает `router.back()` | Тоже `router.back()` при наличии `history.state.back`, иначе «Мои» | нет |
| 22 (закрыто, этап 6½) | Н | вызовы `drillTo` без `back`: `CategorySheet.vue:67`, `MoneyWhere.vue:33`, `analytics-reports/lib/drill-down.ts:24`, `views/saldo.ts:19`, `BudgetList.vue:48`, `SpendingWeek.vue:21`, `SpendingMonth.vue:59` | Попадаешь в Операции без чипа «← <откуда>». (С чипом: `SpentGlance`, `WhereTable.vue:37`, `SpendingIncomplete.vue:48`, `TxSimilar`, `GoalFacts`) | Подставить `back: { to: route.fullPath, label }` в этих 7 местах. Лучше одна точка: `drillTo` принимает `back` по умолчанию из текущего адреса | да (Paths-1 · 5 «Дорога назад» обещает чип для всех «из цифры в список») |
| 23 (закрыто, этап 6½) | Н | `entity-actions.ts:38` (`transactions()`), `AssetDetailsSheet.vue:49` (`txLink`, `analyticsLink`), `HomeChanges.vue:39` («Все операции»), `LossSummary.vue:96` | Из карточки/шторки в Операции без чипа | Тот же `back` из п. 22 | нет |
| 24 (закрыто, этап 6½) | Н | Шторки `TransactionDetailsSheet`, `GoalDetailsSheet`, `AutomationDetailsSheet` внутри стопки `?open=a~b` | Нет явной кнопки «← Назад», только крестик/Esc/браузерная «Назад» (она снимает верхнюю карточку). У `EntitySheet` кнопка есть (`EntitySheet.vue:55`) | Вынести «← Назад» в общий заголовок шторок, показывать при `canGoBack` | нет |
| 25 | Н | Страницы `automations-*`, `transactions-contacts` (ПК), `money-*` | Корни разделов, назад — вкладки/сайдбар; допустимо. Контакт на телефоне имеет «‹» | — | да |

### Старые пути и неработающие параметры

| № | Тип | Где | Что не так | Предложение |
|---|---|---|---|---|
| 26 | — | Старые пути `/analytics`, `/planning`, `/workflows` | В коде интерфейса не используются (проверено `grep`), редиректов нет по решению этапа 1: неизвестный путь -> `/` | Достаточно п. 1–3, 3a |
| 27 | — | `?settings=` | Все значения в коде (`accounting/rates|currencies (Общее), budget|inflation (Деньги)`, `app/device-app|notifications`, `sync`, группы палитры) существуют. Исключение — п. 3. Старый `/settings` перенаправляется на `/?settings=list` | — |
| 28 | — | `?open=` | Обрабатывает один хост (`EntityHost`); все 12 видов разобраны. `entityLocation` для `store`/`workflow`/`scenario` минует `?open=` | — |

### DOM-атрибуты

| № | Тип | Где | Что не так | Предложение |
|---|---|---|---|---|
| 29 (закрыто, этап 6½) | Д | `EntityLink.vue:40` | Есть только `data-entity="kind:id"` на `<a>`. Для `store`/`workflow`/`scenario` ссылка ведёт на страницу, для остальных — на `?open=`; скрипт не может отличить, чего ждать | Добавить в том же месте `:data-entity-open="entityPage(target) ? 'page' : 'card'"` (или `data-entity-kind` + `data-entity-id` отдельно). Одна точка |
| 30 (закрыто, этап 6½) | Д | Сырые `RouterLink` на страницы сущностей (п. 18 таблицы выше) | Нет `data-entity`: скрипт их не видит | Перевести на `EntityLink` (с `plain` и своим классом) там, где это имя кошелька/схемы; строки-карточки (сайдбар, таблица) оставить, но добавить `data-entity` |
| 31 (закрыто, этап 6½) | Д | `Glance.vue:58,86` | Есть `data-glance-trigger="wallet|capital|spent|auto"`, `data-glance` (тело), `data-glance-open` («Открыть →»). Нет id сущности | Добавить проп `entity?: EntityRef` и `:data-entity="..."` на триггер (в `WalletGlance`, `AutomationGlance` передаётся) |
| 32 (закрыто, этап 6½) | Д | Карточка `EntitySheet.vue:44` (`data-tour="entity-card"`) | Нет `data-entity-open="kind:id"` на самой карточке: скрипту нечем проверить, что открылась нужная | Добавить на `SheetContent`, а для шторок tx/goal/automation — тот же атрибут |

Закрыто в этапе 6½ (часть B1): 4 — категория и контакт в строке операции стали `EntityLink` (без своего меню: меню строки теперь содержит «Категория «X»»), 5 — `TransactionsList` берёт взгляд на кошелёк и «авто» по умолчанию из `provideTxGlances` (`shared/lib/tx-glances.ts`, отдаёт `AppShell`), кошелёк текущей страницы остаётся текстом; 6–10 — `EntityLink` в лимитах (`budgetEntity`), «ЖКХ», бонусах, схемах и оповещениях; 12 — подсказка «Журнал по …»; 30 — `data-entity` у всех сырых ссылок на `/stores/` и `/automations/workflows/` (`pageEntityKey` для крошек, напоминаний, задач, ближайших). `check-links`: 230 -> 303 проверенных, 16 -> 0 предупреждений.

Закрыто в этапе 6½ (часть B2): 11 — подсказка «Схема удалена» у имени без ссылки (`JournalTable`; на телефоне строка и так не нажимается); 13 — у карточки суммы (`MoneyCardHost`) выход «Все курсы →» получил `data-glance-open`, у остальных взглядов выход уже был; 14 — «Открыть в контактах» ведёт на `transactions-contacts?contact=<id>` (страница это уже понимала); 15 — `?focus=<id>` для целей, лимитов, напоминаний, оповещений: `shared/lib/use-focus-row.ts` (прокрутка + подсветка токенами `bg-accent ring-ring` на 2,2 с, затем `focus` убирается из адреса), строки несут `data-focus-row`, «План» сам разворачивает списки; 16 — «Открыть в Траты» (`spendingOfCategory`, общий с Ctrl K); 17 — «Операции с активом» (`?q=<тикер>`); «Где лежит» не добавлен: нет дешёвого способа без нового запроса; 18 — меню ссылки на операцию: «Открыть кошелёк» и «Похожие» (`similarLocation` в `shared/lib/transactions`, один построитель с меню строки; пункты есть, когда операция известна карточке/списку: `txDetailsTx`); 20 — крошка «Кошельки › Имя» у корневого кошелька, на телефоне «‹» (`data-wallet-back`); 21 — `useCameFrom` (`history.state.back`): «Автоматизации ›» и «‹» редактора ведут назад, иначе в «Мои», подпись по разделу, откуда пришли; 22–23 — `drillTo` без `back` берёт «← <этот экран>» у оболочки (`registerDefaultBack` в `AppShell`, подпись — название страницы, карточки `?open=` в путь не входят), `back: false` отключает; то же в действиях карточек, `AssetDetailsSheet`, «Все операции» на Главной, `LossSummary` (`hereBackQuery`); 24 — `EntityBack` («← Назад», `data-entity-back`) в шторках операции, цели, автоматизации и общей карточке, виден при стопке `?open=a~b`.

Закрыто в этапе 6½ (часть A): имена маршрутов ядра собраны в `crates/core/src/routes.rs` (в сборке отладки `notice()` проверяет ссылку), сверка с роутером — `scripts/check-routes.mjs` (в `mise run check`); `reachable()` в `notice-link.ts` ведёт на главную, если маршрута нет (колокольчик, тост, трей). Заодно найдено и исправлено `workflows-journal` -> `automations-journal`. Новые атрибуты: `data-entity-open="page|card"` у `EntityLink`, `data-entity-card="kind:id"` у шторок `?open=`, `data-entity` у триггера `Glance` (проп `entity`).

## 5. DOM-признаки для `scripts/check-links.mjs`

Что есть сейчас:

| Атрибут | Где | Что значит |
|---|---|---|
| `data-entity-open="page\|card"` | `<a>` в `EntityLink.vue` | Куда ведёт ссылка: страница или карточка поверх |
| `data-entity-card="kind:id"` | `SheetContent` карточек `?open=` | Какая карточка открыта |
| `data-entity="kind:id"` | `<a>` в `EntityLink.vue:40` | Ссылка на сущность. Разбор по первому `:` (id актива может содержать двоеточия) |
| `data-glance-trigger="wallet\|capital\|spent\|auto"` | кнопка-триггер `Glance` | Взгляд |
| `data-glance`, `data-glance-open`, `data-glance-category` | тело взгляда, кнопка «Открыть →», ссылка категории в `SpentGlance` | Поповер/шторка и выходы из неё |
| `data-back-chip` | `BackChip.vue` | «← <откуда>» |
| `data-tx-open`, `data-tx-auto`, `data-tx-attachments` | строка операции | Прозрачная кнопка открытия, слот «авто», счётчик вложений |
| `data-money-convertible`, `data-money-amount`, `data-money-currency`, `data-money-at`, `data-money-open` | `<Money>` | Сумма/валюта и открытая карточка |
| `data-tour="entity-card"` / `"entity-actions"` | `EntitySheet` | Карточка и её кнопки действий |
| `data-widget-title`, `data-mine-row`, `data-journal-row` | заголовки виджетов, строки «Мои» и журнала | — |

Чего нет и что стоит добавить **в одной точке — `EntityLink.vue`** (все ссылки сущностей проходят через него):

1. `data-entity-open="page|card"` — ждать ли смены маршрута или `?open=`.
2. Вместе с ним (уже есть `data-entity`) скрипт для каждой ссылки: кликнуть, проверить адрес (`/stores/:id`, `/automations/workflows/:id`, `/money/calculator?scenario=` или `?open=` оканчивается на `kind:id`), убедиться, что открылась карточка (`[data-entity-card="kind:id"]`, добавить на `SheetContent` всех четырёх шторок) без `entity.missing`, и что Esc/«← Назад» возвращает прежний адрес.
3. Для взглядов — `data-entity` на `Glance` через проп `entity`.
4. Для чипа «назад» достаточно `data-back-chip`: проверка — на каждом `drillTo` из «цифры» в списке чип есть.
5. Правило для скрипта: любой `RouterLink`, чей `href` совпадает с маршрутом сущности (`/stores/`, `/automations/workflows/`), обязан иметь `data-entity` (поймает п. 18 таблицы).

## 6. Что на холсте, что сверх холста

На холсте (проверено `Full-Paths-2`, `ui-redesign-vs-canvas.md`): взгляд на кошелёк в строке операции (пунктир), «авто» -> взгляд, взгляд на капитал и «Потрачено», чип «← Траты · октябрь»/«← Куда ушли», меню строки (взгляд, кошелёк, контакт), Ctrl K со взглядом на категорию и кошелёк, суммы -> карточка валюты, «Задать лимит». Холст не показывает: названия кошельков в раскрытии «Куда ушли» без пунктира в приложении (документировано), ссылки на категорию, контакт, цель, лимит, схему, актив, напоминание, оповещение (весь `EntityLink` и карточки — решение J1, **сверх холста**), а также чипы «назад» из шторок, крошки корневого кошелька, возврат из редактора схем.
