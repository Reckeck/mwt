---
name: frontend-feature
description: >
  Архитектура фронтенда Multi Wallet Tracker (Vue 3 + Vite + Pinia + TanStack
  Query + vue-i18n + ECharts): куда положить код по FSD, что держать в Query, а
  что в Pinia, как вызвать бэкенд (`call`, `onEvent`, мок для pnpm dev), ключи
  запросов и инвалидация, локали ru/en, маршруты, графики, проверки. Загружать
  при новом экране, сторе, запросе, мутации, маршруте, переводе, графике.
  Триггеры: Pinia, useQuery, useMutation, invalidateQueries, queryKey, i18n,
  локали, перевод, router, маршрут, страница, ECharts, pnpm dev, мок IPC.
---

# Фронтенд: где код и как он говорит с бэкендом

Вид и поведение интерфейса (компоненты, формы, уведомления, таблицы) —
скилл `ui-design-system`. Методология слоёв — `feature-sliced-design`; здесь
только то, как она применена в проекте.

Стек: Vue 3.5 (`<script setup lang="ts">`), Vite, TypeScript strict, Pinia,
TanStack Query, Tailwind v4, shadcn-vue на Reka UI, vue-i18n, vue-echarts.

## Куда класть код

```
src/app/providers/   роутер, QueryClient (+ глобальные уведомления мутаций)
src/pages/<route>/   экран = тонкая композиция; свои части — pages/<route>/ui, model
src/widgets/<name>/  секция страницы из нескольких сущностей/фич
src/features/<name>/ действие пользователя (создать, изменить, удалить, обновить)
src/entities/<name>/ представление сущности (store, transaction, asset, currency, contact…)
src/shared/          ui (компоненты), lib (format, notify, form…), api/ipc.ts, store (Pinia), config (i18n)
```

- Импорт только вниз: `app → pages → widgets → features → entities → shared`;
  соседние слайсы — только через их `index.ts` (правило 9).
- Внутри слайса: `ui/<Component>/` (папка + `index.ts`), `model/use-*.ts`
  (композаблы со состоянием), `lib/*.ts` (чистые функции, тестируемые),
  `*.types.ts`, `*.constants.ts`. Из `<script setup>` ничего не экспортируется.
- В FSD-скилле widgets «не рекомендуются» — в проекте они есть и нужны
  (правило 10).
- Файл длиннее лимита (`.vue` 250, `.ts` 300) делится по смыслу — `check-ui`.

## Состояние: Query или Pinia

- **TanStack Query** — всё, что можно перечитать из Rust: обзор кошелька,
  операции, отчёты, планирование.
- **Pinia** (`shared/store`) — только клиентское и общее для экранов: статус
  vault, настройки приложения и vault, буфер предзаполнения формы.
- Одно и то же в двух местах — источник рассинхрона.

Шаблон запроса, у которого меняется ключ (фильтр, период, страница):

```ts
const report = useQuery({
  queryKey: computed(() => ['report', 'cashflow', filter.value]),
  queryFn: () => call<CashflowReport>('report_cashflow', { input: filter.value }),
  placeholderData: keepPreviousData, // экран не пустеет на смене ключа (правило 17)
})
```

Общие списки (дерево кошельков, контакты, категории) — композаблы из
`entities` (`useStoreTree`, `useContacts`, `useCategories`), а не
`call('get_store_tree')` в каждом компоненте: одни ключи и опции.

Ключи запросов короткие и общие: `['store', id, includeChildren]`,
`['store-history', id]`, `['summary']`, `['stores']`, наборы — `LEDGER_KEYS`,
`PLANNING_KEYS` (`shared/lib/planning`), `STORE_TREE_QUERY_KEY`.
Инвалидация по префиксу, поэтому ключ — массив от общего к частному.

```ts
const archive = useMutation({
  mutationFn: (store: Store) => call('archive_store', { id: store.id, archived: !store.archived }),
  onSuccess: () => {
    for (const queryKey of [['stores'], ['store'], ['summary']]) void queryClient.invalidateQueries({ queryKey })
  },
  meta: { success: 'wallet.archived', error: 'wallet.archiveFailed' }, // уведомление — см. ui-design-system
})
```

## Вызов бэкенда

```ts
import { call, onEvent, parseError } from '@/shared/api/ipc'
```

- Только через `call<T>()` / `onEvent<T>()`; `invoke` из `@tauri-apps/api` в
  компоненте — ошибка.
- Аргументы: скаляры camelCase (`{ storeId }`), DTO — `{ input: { …snake_case… } }`.
  Новая команда — скилл `ipc-command`.
- Типы из `ipc.gen.d.ts` доступны глобально, без импорта; файл генерирует
  `mise run gen-types`.
- Ошибка — `AppError { code, message }` (`parseError`); текст для человека —
  `anyErrorMessage(err, t)` / `codeErrorMessage` (`shared/lib/vault-error`),
  ключи `errors.<CODE>` в обоих локалях.
- Событие бэкенда — только `useBackendEvent(event, handler)`
  (`shared/api/use-backend-event`): отписка с областью видимости. Голый
  `onEvent` в `setup` копит обработчики на каждый заход (`check-ui`
  `raw-listener`). Событие инвалидирует свои ключи, не `invalidateQueries()`.
- Вне Tauri `call` лениво грузит мок (`shared/api/mock/`,
  пароль `secret`). Команду, которую нужно видеть в `pnpm dev`, добавь в мок.

## Локализация

- `ru.json` и `en.json` — плоские ключи (`"nav.dashboard"`), наборы
  совпадают; проверяет `check-onboarding`.
- Любой видимый текст — через `t()`; пользовательские данные (имя кошелька)
  — как есть. Правила текстов — `ui-design-system/references/copy.md`.
- Вне компонента (обработчики, сторы) — `i18n.global.t` из
  `@/shared/config/i18n` или `notify` с ключом.
- Язык переключается без перезагрузки (`shared/lib/locale.ts`), `auto`
  следует за системой.

## Маршрут

Новый экран — папка `src/pages/<route>/` + `index.ts`, запись в
`src/app/providers/router/index.ts` с `meta: { i18n: 'nav.…' }` (по ключу —
заголовок окна и `h1` оболочки). Вложенные — `children` у `AppShell`;
`meta.public: true` — экран вне оболочки (`/unlock`). У каждого маршрута тур
или запись в `TOURLESS_ROUTES` (скилл `onboarding`). Переименовал маршрут —
оставь редирект со старого пути.

## Графики

vue-echarts; модули регистрируются в `shared/lib/echarts.ts`, тема — в
`echarts-theme.ts`. Правила вида — `ui-design-system/references/data-display.md`.

## Проверки

```bash
mise run lint        # Biome: линт + формат .ts (.vue форматируй вручную: одинарные кавычки, без ;)
mise run typecheck   # vue-tsc strict
mise run check-ui    # компоненты, токены, уведомления, размер файлов
pnpm dev             # мок IPC в браузере
mise run check       # всё перед коммитом
```

- `verbatimModuleSyntax`: типы — `import type`.
- Имена от двух символов (Biome `useNamingConvention`; исключение — `t`);
  неиспользуемый параметр — с `_`.

Коммит — только по просьбе пользователя (скилл `jj-workflow`).
