# Данные: таблицы, деньги, P&L, графики, списки

## Числа

| Что | Как показать |
|---|---|
| сумма | `<Money :amount :currency>`; в строке — `formatMoney(value, { currency })` |
| сумма со знаком и цветом | `<Money colored>` |
| прибыль/убыток | `<PnlText :amount :currency :pct>` |
| процент | `formatPct(value)`, со знаком — `formatPct(value, { signed: true })`; значение **уже в процентах** (`12.5` → «12,50 %»), не доля |
| целое число (сделки, штуки) | `formatQuantity(value)` |
| количество актива | `formatQuantity(value)` — без хвостовых нулей (`1`, `0,00042`) |
| цена за единицу | `formatPrice(value, currency)` — 2 знака от 1 и больше, точнее для мелких |
| дата / дата и время | `formatDate`, `formatDateTime`; «сегодня в 14:05» — `formatAsOf` |
| цвет по знаку | `signClass(value)` / `SIGN_TONE_CLASS[signTone(value)]` |

- Никаких `toFixed`, `toLocaleString(undefined)` и ручного «−»: локаль
  интерфейса и группировка разрядов есть только в `format.ts`.
- Деньги из Rust приходят строкой (`Amount`), в `f64` не превращай для
  логики — только для отображения.
- Одна и та же величина на разных экранах форматируется одинаково (итог
  кошелька на карточке, на странице и в отчёте — одно число знаков).
- Ноль нейтральный: не «прибыль» и не зелёный.
- Цифры, которые сравнивают глазами, — `tabular-nums` (уже в `Money`,
  `StatCard`, правых колонках `DataTable`).

## Таблица — `DataTable`

Колонки — `computed`: заголовки через `t()` должны смениться вместе с языком.

```ts
const columns = computed<ColumnDef<Position>[]>(() => [
  { accessorKey: 'symbol', header: t('pos.asset') },
  {
    id: 'qty',
    header: t('pos.qty'),
    accessorFn: (row) => Number(row.quantity),
    cell: ({ row }) => formatQuantity(row.original.quantity),
    meta: { align: 'right' },
  },
  {
    id: 'pnl',
    header: t('pos.pnl'),
    accessorFn: (row) => Number(row.pnl_base),
    cell: ({ row }) => h(PnlText, { amount: row.original.pnl_base }),
    meta: { align: 'right' },
  },
])
```

```vue
<DataTable :columns="columns" :data="rows" :initial-sorting="[{ id: 'pnl', desc: true }]" sticky-header />
```

- Числа — по правому краю (`meta.align: 'right'`), текст и даты — по левому,
  по центру ничего. Заголовок выровнен как его колонка.
- Сортировка по клику на заголовок; разумная сортировка по умолчанию
  (новые сверху, крупные сверху).
- Тонкие разделители строк, без зебры; шапка закреплена, если таблица
  прокручивается.
- Пусто из-за запроса (нет данных вообще, нет за период) — `SectionState
  :empty` вокруг таблицы; `emptyText` таблицы — только для пустоты внутри уже
  показанной таблицы (например, после локального фильтра по строке поиска).
- Где лежит: таблица отчёта или экрана из нескольких сущностей — `widgets/`;
  таблица одной сущности без действий — `entities/<name>/ui`.
- Строка-ссылка: `clickable` + `@row-click` (клавиатура: Tab + Enter уже
  работают).
- Действия строки — `ActionMenu` в последней колонке.

## Длинные списки

Больше ~50 строк — страница за страницей с кнопкой «Показать ещё N»
(`ui.showMore`), как в `TransactionsList` (по 100, новый фильтр — снова с
первой страницы) и `LossSummary`. Виртуализация (`@tanstack/vue-virtual`) —
только для строк одной высоты: строки разной высоты в виртуальном списке
прыгают. Обрезка без кнопки («первые 8») прячет данные молча.

## Графики (vue-echarts)

- Высота — на обёртке: `<div class="h-80"><Chart … /></div>` (vue-echarts
  8 ставит `height: 100%` стилем, который перебивает `h-*` на самом
  `<Chart>`).
- Новый тип графика — регистрация в `shared/lib/echarts.ts`.
- У каждого графика: подписи осей через форматтер денег, подсказка с
  валютой, легенда при нескольких сериях, состояние «Нет данных» вместо
  пустых осей (`SectionState :empty`).
- Цвета серий — из темы (`echarts-theme.ts`), различимые при дальтонизме;
  рост/падение дополнительно отличаются формой или подписью.
- Тема графика следит за темой приложения (реактивно, не один раз при
  создании).

Источники: Pencil & Paper «Enterprise data tables»
(pencilandpaper.io/articles/ux-pattern-analysis-enterprise-data-tables),
Vercel Web Interface Guidelines (github.com/vercel-labs/web-interface-guidelines).
