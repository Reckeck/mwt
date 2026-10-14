---
name: ui-design-system
description: >
  Как выглядит и ведёт себя интерфейс Multi Wallet Tracker: какой общий компонент
  взять (EmptyState, SectionState, DataTable, FormDialog, PageHeader, StatCard,
  Money/PnlText, SettingsSection, AdvancedFields, ConfirmButton), токены цвета и
  шкала заголовков, формы и проверка полей, уведомление о каждом действии
  (`meta` у useMutation, `notify`), загрузка/ошибка/пусто, таблицы и деньги,
  клавиатура и фокус, тексты. Загружать перед любой вёрсткой в src/**/*.vue:
  экран, диалог, форма, таблица, кнопка, карточка, toast, пустое состояние,
  загрузка, сумма, P&L, тёмная тема, UI-ревью.
paths: src/**/*.vue
---

# UI Multi Wallet Tracker

Скилл отвечает на два вопроса: **из чего собрать** экран и **как он должен
себя вести**. Как добавить или починить сам компонент shadcn-vue — скилл
`shadcn-vue`; где лежит код и куда его импортировать — `feature-sliced-design`
и `frontend-feature`; туры и «Что нового» — `onboarding`.

**Не применять:** Rust, CI, чистая логика в `lib/`/`model/` без разметки.

## Отличия проекта от скилла shadcn-vue

`shadcn-vue` ставится из реестра и не правится, поэтому расхождения
записаны здесь. В этих пунктах проект главнее:

| shadcn-vue говорит | В проекте |
|---|---|
| иконки `@lucide/vue` | `lucide-vue-next`; после `shadcn-vue add` замени импорт |
| удаление подтверждает `AlertDialog` | `ConfirmButton` на месте (клик → «Удалить?» → клик), для обратимого — «Отменить» в уведомлении |
| формы на `FieldGroup` + `Field` | `FormField` (обёртка над shadcn `Field`: `label`, `required`, `error`, `hint`); `FieldSet`/`FieldGroup` вокруг — можно |
| `CardHeader`/`CardContent`/… | `Card` со слотами `#header` / default / `#footer` (части размечены `data-slot`) |
| `Button` без состояния загрузки, рядом `Spinner` | так же; в диалоге формы это уже делает `FormDialog` |
| слой widgets не нужен (скилл FSD) | widgets есть — правило 10 AGENTS.md |

Компоненты shadcn живут в `src/shared/ui/<name>/` (алиасы `components.json`).
CLI не умеет пробный прогон: ставь во временную копию проекта и переноси
нужные папки, чтобы не затереть доработанные `button`, `input`, `label`,
`dialog`.

## Критичные правила

У каждого правила — чем оно проверяется. «Ревью» значит: скрипта нет, следи сам.

1. **Готовый компонент вместо разметки.** Сначала каталог ниже, потом реестр
   shadcn-vue, только потом своя композиция из компонентов shadcn в
   `shared/ui`. Похожая разметка в трёх местах — повод вынести общий
   компонент. → `check-ui` (raw-button, raw-check, raw-table, hand-empty)
2. **Каждое действие сообщает исход.** У `useMutation` есть
   `meta: { success, error }` (и `undo` для обратимого) либо
   `meta: { silent: '<почему>' }` — тогда ошибку показывает свой `onError`
   через `notify.error`. Уведомление показывает `MutationCache`, а не
   компонент. → `check-ui` (mutation-meta),
   [feedback.md](references/feedback.md)
3. **Загрузка, ошибка и «пусто» различимы.** Ложное «Ничего нет», пока данные
   ещё едут, — баг. `SectionState` по `isPending`/`error`/пусто. → ревью
4. **Деньги — через `Money`/`PnlText`/`format.ts`.** Цифры `tabular-nums`, в
   таблице по правому краю; у P&L знак, ноль нейтральный. → `check-ui`
   (to-fixed), [data-display.md](references/data-display.md)
5. **Цвет не единственный носитель смысла.** Рост/падение — знак и цвет,
   статус — текст или иконка и цвет. → ревью
6. **Только токены.** `text-success`, `text-warning`, `text-destructive`,
   `bg-muted`…; без `emerald-*`/`red-*`, hex и ручных `dark:`. → `check-ui`
   (raw-color), [tokens.md](references/tokens.md)
7. **Кнопка сохранения активна**, поля проверяются при нажатии и показывают
   ошибку под собой. → `check-forms`, [forms.md](references/forms.md)
8. **Счёт, актив, валюта — только готовыми пикерами** (правило 14). →
   `check-forms`
9. **Enter отправляет форму** (диалог формы — `FormDialog` или `<form>`), Esc
   закрывает окно, фокус виден (`focus-visible:ring`). → ревью,
   [a11y-motion.md](references/a11y-motion.md)
10. **Опасное действие — подтверждение на месте или «Отменить».** →
    ревью
11. **Анимация только там, где помогает понять**, без `transition-all`;
    `prefers-reduced-motion` гасит всё глобально (`styles.css`). →
    `check-ui` (transition-all)
12. **Состояние экрана переживает переход:** вкладка и фильтры — в
    query-параметрах URL. → ревью
13. **Длинный список не рисуется целиком:** больше ~50 строк — «Показать
    ещё N» постранично (образец — `TransactionsList`). → ревью
14. **Тексты — в обоих локалях**, по правилам [copy.md](references/copy.md). →
    `check-onboarding` (совпадение ключей)
15. **Файл — одна смысловая часть:** `.vue` до 250 строк, `.ts` до 300;
    длиннее — дели на `model/`, `lib/`, `ui/`. → `check-ui` (file-size)

## Каталог: задача → компонент

| Задача | Компонент | Где |
|---|---|---|
| шапка страницы: крошки, заголовок, действия | `PageHeader` | `shared/ui/page-header` |
| загрузка / ошибка с «Повторить» / пусто / данные | `SectionState` | `shared/ui/section-state` |
| пустое состояние с кнопкой «Создать» | `EmptyState` | `shared/ui/empty-state` |
| таблица с сортировкой, числа справа | `DataTable` (колонки `ColumnDef`, `meta.align`) | `shared/ui/data-table` |
| подпись + крупное число | `StatCard` | `shared/ui/stat-card` |
| сумма с валютой и подсказкой курса | `Money` (`colored` — знак и цвет) | `entities/currency` |
| прибыль/убыток, с процентом | `PnlText` | `entities/currency` |
| диалог с одной формой | `FormDialog` (`form-id`, `pending`, `@submit`) | `shared/ui/form-dialog` |
| поле формы с `*`, ошибкой, подсказкой | `FormField` | `shared/ui/form-field` |
| блок настроек с одним заголовком | `SettingsSection` | `shared/ui/settings-section` |
| редкие поля формы под «Дополнительно» | `AdvancedFields` | `shared/ui/advanced-fields` |
| дата / дата и время / месяц | `DateTimePicker` (`mode`) | `shared/ui/date-time-picker` |
| панель фильтров списка или отчёта | `FilterBar` (частые — в слоте, редкие — `#more`, `active` → «Сбросить (N)»; поле — `FILTER_FIELD_CLASS`) | `shared/ui/filter-bar` |
| строка списка или карточка — одна большая кнопка | `Button size="block"` (сама раскладка — классами) | `shared/ui/button` |
| маленький «×» в чипе или строке | `Button variant="ghost" size="icon-xs"` (`icon-sm` — 32 px) | `shared/ui/button` |
| таблица без сортировки (месяцы по порядку, ячейка на несколько колонок) | части shadcn `Table` | `shared/ui/table` |
| удалить / убрать с подтверждением | `ConfirmButton` | `shared/ui/confirm-button` |
| меню «⋯» строки или карточки | `ActionMenu` | `shared/ui/action-menu` |
| виджет Главной | `WidgetFrame` | `shared/ui/widget-frame` |
| выноска-предупреждение, ошибка на месте | `Alert` | `shared/ui/alert` |
| поповер, команда, список с поиском | `Popover`, `Command`, `Combobox` | `shared/ui/…` |
| выбор из 2–7 вариантов | `ToggleGroup` | `shared/ui/toggle-group` |
| радио, чекбокс, переключатель | `RadioGroup`, `Checkbox`, `Switch` | `shared/ui/…` |
| хлебные крошки отдельно | `Breadcrumb…` | `shared/ui/breadcrumb` |
| ожидание на кнопке | `Spinner` | `shared/ui/spinner` |
| заглушка загрузки | `Skeleton` (обычно через `SectionState`) | `shared/ui/skeleton` |
| горячая клавиша в подсказке | `Kbd` | `shared/ui/kbd` |
| счёт / кошелёк | `StoreNodeSelect`, в операции — `AccountRefPicker` | `entities/store`, `features/transaction-create` |
| актив | `AssetInput` (поиск), `AssetSelect` (из известных) | `entities/asset` |
| валюта | `CurrencyPicker` | `entities/currency` |

Полный список — папки `src/shared/ui/*` (каждая с `index.ts`). Нет нужного —
`pnpm dlx shadcn-vue@latest search`, затем `add`.

## Порядок работы

1. **Проверь, нет ли этого уже.** Поищи IPC-команду и ключ i18n
   (`grep -rn "delete_contact" src`): действие может уже жить в другой фиче.
2. **Проверь контракт бэкенда** до вёрстки формы: DTO в `ipc.gen.d.ts` и
   проверки в `crates/core/src/service/*` — обязательные поля и отказы,
   которые форма должна поймать сама (правило 13).
3. **Собери из каталога.** Каталог главнее существующего кода: старые экраны
   ещё переводятся на общие компоненты (`check-ui.baseline.json`), поэтому
   «сделай как в соседнем файле» может привести к старому шаблону. Своё —
   только композиция в `shared/ui` (папка + `index.ts` + `*.types.ts`,
   правило 11).
4. Подключи исход каждого действия (`meta`), три состояния данных
   (`SectionState`), тексты в обоих локалях.
5. Обучение (скилл `onboarding`): новый экран — тур в `tours.ts`; новый
   заметный элемент — шаг в тур его экрана и `data-tour`; смена поведения —
   пункт в текущей записи `whats-new.ts`.
6. Проверь глазами в `pnpm dev` (мок, пароль `secret`): светлая и тёмная
   тема, окно 1280 и узкое ~800, загрузка (ложного «пусто» нет).
7. `mise run check` (Biome, vue-tsc, `check-onboarding`, `check-forms`,
   `check-ui`, clippy, тесты).

## Проверки

| Скрипт | Ловит |
|---|---|
| `mise run check-ui` | сырые `button`/`checkbox`/`radio`/`table`, ручное пустое состояние, `emerald`/`red`/hex, `transition-all`, `toFixed` в `.vue`, `div @click` без роли, `useMutation` без `meta`, файл длиннее лимита |
| `mise run check-forms` | кнопка сохранения заблокирована проверкой полей, ручная проверка в `@submit`, самодельные списки счетов/активов/валют |
| `mise run check-onboarding` | экран без тура, шаг без элемента, нет текста, ключи ru ≠ en |

Долги, которые были до правила, записаны в `scripts/check-ui.baseline.json`.
Файл не может стать хуже своей записи. Исправил — `node scripts/check-ui.mjs
--update`, запись уменьшится. Новый файл должен быть чистым.

## UI-ревью: формат отчёта

Находки по файлам, `файл:строка`, одна строка на находку, сначала то, что
мешает пользователю (ложное «пусто», молчащее действие, недоступная
клавиатурой кнопка), потом единообразие. Без вступлений.

## Справочник

- [tokens.md](references/tokens.md) — цвета, шкала заголовков, отступы, радиусы, тёмная тема
- [forms.md](references/forms.md) — правила 13 и 14 полностью, `useFormValidation`, `FormDialog`
- [feedback.md](references/feedback.md) — уведомления, загрузка, ошибки, пусто, подтверждение и «Отменить»
- [data-display.md](references/data-display.md) — таблицы, деньги, проценты, даты, P&L, графики, длинные списки
- [layout.md](references/layout.md) — страница, диалог, Sheet, вкладки, узкие окна
- [a11y-motion.md](references/a11y-motion.md) — клавиатура, фокус, ARIA, контраст, анимация
- [copy.md](references/copy.md) — тексты интерфейса на ru и en, словарь терминов

Откуда правила: Vercel Web Interface Guidelines, скилл `unovue/shadcn-vue`,
Anthropic Agent Skills best practices, Rauno Freiberg «Interfaces»,
Pencil & Paper — таблицы с данными (ссылки — в `references/`).
