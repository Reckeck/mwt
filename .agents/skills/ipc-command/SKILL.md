---
name: ipc-command
description: >
  Порядок добавления или изменения Tauri-команды (IPC) в trade-app — от DTO в
  ipc_contract.rs до вызова с фронтенда. Использовать при задачах вида «добавь
  команду», «верни поле X в ответе», «измени контракт», «вызови с фронта», а
  также при починке gen-types и рассинхронизации ipc.gen.d.ts. Триггеры: tauri
  command, invoke, IPC, ipc_contract, ipc.gen.d.ts, gen-types, ts-rs,
  generate_handler, DTO, новый эндпоинт, RefreshResult, require_vault,
  err_to_string, ошибка из UI вида "CODE: message".
---

# Добавление/изменение Tauri-команды

Архитектурная граница жёсткая: **`crates/core` не знает про Tauri**,
`crates/app/src/commands/*` — тонкая IPC-обёртка, которая только переводит DTO
в вызовы core-сервиса. Никогда не клади доменную логику в `commands/`.

## Слои и кто за что отвечает

| Слой | Файл | Что там |
|---|---|---|
| DTO | `crates/core/src/ipc_contract.rs` | входы/выходы команд, `#[derive(Serialize, Deserialize, ts_rs::TS)]` |
| Домен | `crates/core/src/service.rs` (`UnlockedVault`) | вся логика + юнит-тесты |
| Контейнер | `crates/core/src/storage/mod.rs` | `SnapshotFormat`, encode/decode, миграция версий |
| IPC | `crates/app/src/commands/<модуль>.rs` | хендлер `#[tauri::command]` |
| Регистрация | `crates/app/src/lib.rs` | `tauri::generate_handler![…]` |
| TS-типы | `src/lib/ipc.gen.d.ts` | **генерируется**, руками не править |
| Вызов | `src/lib/ipc.ts` → `call<T>(cmd, args)` | единственная точка `invoke` |
| Документация | `docs/ipc.md` | таблица команд, чеклист |

## Пошаговый чеклист

### 1. DTO в `crates/core/src/ipc_contract.rs`

Секции разделены комментариями-разделителями (`// -- auth ---…`). Клади новый
тип в свою секцию, не в конец файла.

```rust
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct ArchiveStoreInput {
    pub id: String,
    pub archived: bool,
}
```

Правила:
- **`#[derive(ts_rs::TS)]` обязателен.** Без него тип не попадёт в
  `ipc.gen.d.ts` и фронтенд не получит тип (тихо, без ошибки компиляции).
- **`Default` обязателен**, если `ipc.gen.d.ts` содержит необязательные поля —
  генератор требует его для `export_all_to`. Это же делает
  `CachedPrice::default()` осмысленным.
- Поля — `snake_case` (serde по умолчанию); TS-обвязка это учитывает.
- Внутренние модели (`AssetRef`, `Store`, `Transaction`) наружу не отдавай —
  если фронту нужен доменный тип, сделай DTO с нужными полями.
- `Amount` в TS — это `string` (кастомный impl в `amount.rs`), не `number`.

### 2. Логика + тест в `crates/core/src/service.rs`

Метод на `UnlockedVault`, возвращающий `Result<_, crate::error::Error>`:

```rust
pub fn archive_store(&mut self, id: &str, archived: bool) -> Result<()> { … }
```

Здесь же — юнит-тест в том же файле (`#[cfg(test)] mod tests`). Core тестируется
без Tauri: vault создаётся во временной директории, пароль любой.

### 3. Хендлер в `crates/app/src/commands/<модуль>.rs`

Сигнатура-шаблон (не отступать от неё):

```rust
#[tauri::command]
pub async fn archive_store(
    input: ArchiveStoreInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_mut().unwrap();
    v.archive_store(&input.id, input.archived)
        .map_err(err_to_string)
}
```

Обязательные инварианты:
- Возвращай `Result<T, String>`. Строка — через
  `err_to_string(e)` = `format!("{}: {}", e.code(), e)`.
- `require_vault(&state)` для всего, что трогает vault; иначе —
  `Error::RequiresUnlock`.
- Ошибку vault-лока (`lock_vault`) мапь на `Error::Crypto("lock poisoned")`.
- Args приходят как `input: XInput` (один объект), а не плоско. Это позволяет
  добавлять поля без ломающих изменений вызова.

### 4. Зарегистрировать в `crates/app/src/lib.rs`

Добавить в `tauri::generate_handler![…]` **в ту же группу и с тем же
комментарием-разделителем**, что и остальные команды модуля. Забытый
`generate_handler!` — самая частая ошибка: команда компилируется, но
`invoke` на фронте падает «command not found».

### 5. Перегенерировать TS-типы

```bash
mise run gen-types
```

Проверь дифф `src/lib/ipc.gen.d.ts` — новый тип **должен** появиться. Нет
диффа → нет `#[derive(TS)]` или DTO не экспортирован в
`src/bin/gen_types.rs`.

### 6. Вызвать с фронтенда

```ts
import { call } from '@/lib/ipc'

const overview = useQuery({
  queryKey: ['store', storeId.value],
  queryFn: () => call<StoreOverview>('get_store_overview', { storeId: storeId.value }),
})
```

- `call<T>(cmd, args)` — единственная обёртка над `invoke`. Прямой `invoke` в
  компоненте — ошибка.
- Тип `T` бери из `ipc.gen.d.ts` (глобальные типы, без импорта).
- После мутации инвалидируй затронутые query-ключи:
  `void queryClient.invalidateQueries({ queryKey: ['stores'] })`.
- Вне Tauri `call` уходит в мок внутри `src/lib/ipc.ts` — добавь туда ветку,
  если хочешь проверять UI в браузере через `pnpm dev`.

#### Имена аргументов: camelCase снаружи, snake_case внутри `input`

Это самое частое место ошибки. Действуют **два разных** правила:

```ts
// 1. Скалярные параметры команды → Tauri v2 ждёт camelCase
call<StoreOverview>('get_store_overview', { storeId: storeId.value })
//                                        ^^^^^^^^ Rust: store_id: String

// 2. DTO-структура → поле передаётся как `input`, ВНУТРИ ключи snake_case
call<GetCandlesOutput>('get_candles', {
  input: { symbol: 'BTC', interval_min: 60, limit: 300 },
})
//                        ^^^^^^^^^^^^ serde: snake_case, camelCase НЕ работает
```

То есть: `#[tauri::command] fn f(store_id: String)` → `{ storeId }`, а
`fn f(input: FInput)` → `{ input: { …snake_case… } }`. Причина: camelCase —
конвенция обёртки Tauri для **параметров функции**, тогда как внутри структуры
работает обычный serde, а он здесь snake_case.

- `Amount` приходит из Rust как **строка** — не парсь его в `number` для
  арифметики; форматируй через `formatMoney` из `@/lib/format`.

### 7. Обновить `docs/ipc.md`

Добавь строку в таблицу нужного модуля: `| command | input | output |`.
Раздел «Добавление новой команды (чеклист)» держи актуальным.

### 8. Проверки

```bash
mise run check
```

Обязательно: `mise run test` (core), `mise run typecheck` (фронт),
`mise run lint`, и дифф `ipc.gen.d.ts`.

## Железное правило: `MutexGuard` нельзя держать через `.await`

`AppState.vault` — `Mutex<Option<UnlockedVault>>`. Guard **не `Send`**, поэтому
`async fn`, который держит guard во время `.await`, не скомпилируется.

Правильный конвейер (эталон — `refresh_all` в `commands/pricing.rs`):

```rust
let data = {
    let mut g = require_vault(&state).map_err(err_to_string)?;   // 1. взять
    let v = g.as_mut().unwrap();
    /* скопировать нужное в локальные переменные */
    (copied_data, base)
};

let fresh = state.pricing.fetch_prices(&copied_data).await?;      // 2. await без guard

let mut g = require_vault(&state).map_err(err_to_string)?;        // 3. взять снова
let v = g.as_mut().unwrap();
/* применить */
```

Три шага: **взять → отпустить → await → взять снова**. Компилятор поймает
нарушение сам, но если ты сначала пишешь `.await` внутри блока с guard —
получишь путаную ошибку вендинга, а не понятную про mutex.

## Коды ошибок

`Error::code()` в `crates/core/src/error.rs` возвращает строку, которая
становится префиксом сообщения. Фронт парсит его в `parseError`
(`src/lib/ipc.ts`) и ищет ключ `errors.<CODE>` в локалях.

Новый вариант `Error` → добавь его код в `Error::code()` **и** ключ
`errors.<CODE>` в `src/i18n/locales/ru.json` и `en.json`. Иначе UI покажет
`UNKNOWN` или пустую строку.

Существующие коды: `SECRET`, `ALREADY_EXISTS`, `NOT_FOUND`, `UNSUPPORTED`,
`SERIALIZE`, `DESERIALIZE`, `CRYPTO`, `IO`, `NETWORK`, `INVALID_INPUT`,
`PRICING`, `MISSING`, `ARCHIVED`, `REQUIRES_UNLOCK`.

## Частые грабли

| Симптом | Причина |
|---|---|
| `invoke` → «command … not found» | не добавлен в `generate_handler!` |
| Аргумент приходит пустым/дефолтным | camelCase/snake_case перепутаны (см. §6) |
| TS-типа нет, ошибок нет | нет `#[derive(TS)]` / нет `Default` |
| `failed to serialize` / undefined на фронте | тип есть в Rust, но нет `#[serde(default)]` для нового поля |
| Форма не очищается после сохранения | забыт `invalidateQueries` |
| Ошибка локализуется как `UNKNOWN` | нет ключа `errors.<CODE>` в локалях |
| `MutexGuard` в `.await` не компилируется | нарушено правило трёх шагов выше |
| Список пуст в браузере (`pnpm dev`) | в моке `src/lib/ipc.ts` нет ветки для новой команды |