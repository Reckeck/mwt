---
name: frontend-feature
description: >
  Как добавлять и менять фичи во фронтенде trade-app (Vue 3 + Vite + Pinia +
  TanStack Query + vue-i18n + ECharts + Tailwind v4 + shadcn-vue/Reka UI).
  Использовать при любой правке в src/** — новый экран, форма, таблица, график,
  стор, локализация, тема. Триггеры: .vue, view, компонент, форма, стор,
  Pinia, useQuery, useMutation, invalidateQueries, i18n, локали, перевод, ru/en,
  ECharts, график, Tabs, Dialog, shadcn, Reka UI, Tailwind, тема, theme,
  маршрут, router, страница, модалка, таблица.
---

# Фронтенд: Vue 3 + Pinia + vue-query + i18n

Стек: Vue 3.5 (`<script setup lang="ts">`), Vite 8, TypeScript **strict**,
Pinia (глобальное состояние), TanStack Query (серверное состояние),
Tailwind v4, shadcn-vue поверх Reka UI, vue-i18n, ECharts через vue-echarts.

## Структура (важно: `src/features/` пуст)

```
src/views/          экраны — по одному .vue на маршрут роутера
src/components/ui/  примитивы shadcn-vue, ИМЕНА В НИЖНЕМ РЕГИСТРЕ (button.vue)
src/components/wallet/  доменные компоненты кошелька (WalletView.vue и т.п.)
src/stores/         Pinia: vault.ts, settings.ts, tradeLookup.ts
src/lib/ipc.ts      ЕДИНСТВЕННАЯ точка вызова Tauri: call<T>() + onEvent<T>()
src/lib/format.ts   formatMoney / formatDate
src/lib/echarts.ts  регистрация модулей ECharts
src/lib/chains.ts   метаданные сетей (CoinGecko id, explorer patterns)
src/i18n/locales/   ru.json + en.json
src/router/index.ts маршруты + заголовки через meta.i18n
```

Каталоги `src/features/{auth,charts,dashboard,pricing,settings,sim,stores,transactions}`
— **пустые заглушки**, кода в них нет и git их не трекает. Не ищи код там;
клади новые экраны в `src/views/`, переиспользуемые части — в `src/components/`.

## Слои состояния: Pinia vs TanStack Query

Это главное архитектурное решение, не нарушай его:

- **TanStack Query** — всё, что приходит из Rust: overview, pnl, transactions.
  Ключи — короткие строки: `['store', id]`, `['store-pnl', id]`,
  `['store-history', id]`, `['summary']`, `['stores']`.
- **Pinia** — только клиентское/кросс-экранное состояние: статус vault,
  настройки, буфер предзаполнения формы сделки.

Если состояние можно перезагрузить `call<T>()` — это query, не стор.
Дублирование одного и того же в Pinia и в query — источник рассинхрона.

После мутации **всегда** инвалидируй затронутые ключи, иначе UI покажет
устаревшие данные:

```ts
const toggleArchive = useMutation({
  mutationFn: () => call<void>('archive_store', { input: { id, archived } }),
  onSuccess: () => {
    void queryClient.invalidateQueries({ queryKey: ['store', id] })
    void queryClient.invalidateQueries({ queryKey: ['stores'] })
    void queryClient.invalidateQueries({ queryKey: ['summary'] })
  },
})
```

## Вызов бэкенда

```ts
import { call, onEvent, type AppError } from '@/lib/ipc'
```

- Прямой `invoke` из `@tauri-apps/api/core` в компоненте — ошибка.
- Аргументы: скаляры → **camelCase** (`{ storeId }`), DTO → `{ input: { …snake_case… } }`.
  Подробности — в скилле `ipc-command`.
- Типы берутся из `ipc.gen.d.ts` **глобально**, без импорта.
- Ошибки — `AppError { code, message }`; `code` уже разобран из `CODE: message`.
- Вне Tauri `call` уходит в мок в том же файле — новую команду добавь туда,
  если хочешь видеть её в `pnpm dev`.

Событие `prices-updated` подписывается через `onEvent<T>` (вне Tauri — no-op).

## Локализация (обязательна, оба файла)

`ru.json` и `en.json` — **плоские точечные ключи** (`"nav.dashboard"`,
`"tx.kind.income"`, `"errors.CRYPTO"`), без вложенных объектов. Сейчас 241
ключ в каждом; **наборы ключей обязаны совпадать**.

```ts
const { t } = useI18n()
t('nav.dashboard')
t(`tx.kind.${kind}`)
```

Правила:
- **Любой видимый текст** — через `t()`. Хардкод строк в шаблоне — ошибка ревью.
- Добавляя ключ, добавляй его **в оба локаля**. Забытый ключ в `en` уедет в
  fallback и покажет сырое имя ключа пользователю.
- Ключ ошибки = `errors.<CODE>`, где `<CODE>` — `Error::code()` из Rust.
  Новый код ошибки → новый ключ в обоих локалях.
- Шаблон не хардкодить: если это не i18n-строка, а пользовательский ввод
  (имя кошелька, комментарий) — рендерить как есть.
- Проверка синхронности ключей (быстрая, вручную или скриптом):

  ```bash
  python3 -c "import json;ru=json.load(open('src/i18n/locales/ru.json'));en=json.load(open('src/i18n/locales/en.json'));print('only ru:',set(ru)-set(en));print('only en:',set(en)-set(ru))"
  ```

  Пустой вывод с обеих сторон — ключи синхронны.

`i18n` настроен как `legacy: false`, `fallbackLocale: 'en'`, сообщения
`{ en, ru }`; язык переключается без перезагрузки через
`persistLocale` / `resolveLocale` в `src/i18n/index.ts` (значение `auto`
следует за браузером).

## Стиль компонентов

- Одноимённые примитивы импортируются как `import Button from '@/components/ui/button.vue'`
  (имя файла в нижнем регистре, компонент в PascalCase).
- Варианты примитива заданы через `cva` (`class-variance-authority`) +
  `withDefaults(defineProps<…>())`; `cn()` из `@/lib/utils` мержит классы.
  Новый примитив повторяет этот шаблон, включая
  `defineOptions({ inheritAttrs: false })`.
- Стили — **Tailwind-утилиты** в разметке. Новый CSS-файл/`<style>` без нужды
  не заводи; тема идёт через CSS-переменные (`bg-primary`, `text-destructive`,
  `border-border`, `ring-ring`).
- Иконки — `lucide-vue-next`.
- Алиас `@/` → `src/` настроен в `vite.config.ts` и `tsconfig.json`; импорты
  внутри `src` делай через `@/…`.
- `verbatimModuleSyntax: true` → для типов нужен `import type { … }`; для
  значений — обычный импорт. Импортированное, но не используемое, ломает
  `noUnusedLocals`.
- `noUnusedParameters` — неиспользуемый параметр именуй `_foo`.

## Графики

ECharts подключён через `vue-echarts`; нужные модули регистрируются в
`src/lib/echarts.ts` — новый тип графика добавь туда, иначе он не отрисуется.
Компонент-обёртка лежит в `src/components/wallet/`. При изменении графика
проверяй, что он корректно ведёт себя на **пустых данных** (состояние
`chartEmpty` уже есть в локалях).

## Маршрут

Новый экран → файл в `src/views/`, запись в `src/router/index.ts` c
`meta: { i18n: 'nav.…' }` (по этому ключу ставится заголовок вкладки).
Вложенные маршруты — `children` у `AppShell.vue`; `meta.public: true` — для
экрана вне shell (`/unlock`).

## Проверки

```bash
mise run lint          # biome check: линт + формат (biome.json)
mise run typecheck     # vue-tsc --noEmit, strict
pnpm dev               # быстрый визуальный контроль в браузере (мок IPC)
mise run dev           # полный Tauri-цикл
mise run check         # всё вместе перед коммитом
```

`mise run typecheck` ловит несоответствие между тем, что сгенерил Rust, и тем,
что ждёт фронт, — не откладывай его.

`mise run lint` (Biome) запрещает **однобуквенные имена**: у переменных и
параметров минимум 2 символа (`style/useNamingConvention`, regex `t|.{2,}`).
Исключение одно — `t` из `const { t } = useI18n()`. В Rust то же правило
держит `clippy::min_ident_chars` (`clippy.toml`), так что `i`/`j`/`x` запрещены
и там; неиспользуемую переменную помечай `_`-префиксом.

Коммит (см. скилл `jj-workflow`) — **только по явной просьбе пользователя**.