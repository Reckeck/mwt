# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Основной источник правил — `AGENTS.md` (общий для всех агентов); он импортируется
ниже целиком и имеет приоритет. Здесь — только дополнения, которых там нет.

@AGENTS.md

## Точечный запуск тестов

Тесты Rust — инлайн-модули `#[cfg(test)]` внутри `crates/core/src/**` (отдельной
папки `tests/` нет). Фронтенд-тестов нет; проверка фронта = Biome + vue-tsc.

```bash
cargo test -p trade-core                      # только ядро
cargo test -p trade-core service::            # по префиксу пути модуля
cargo test -p trade-core <name> -- --exact --nocapture
```

`CARGO_TARGET_DIR=.rust-target` задан в `mise.toml` — при запуске cargo вне
mise артефакты окажутся в `target/`, и сборка пойдёт с нуля.

## Сквозная картина (то, что требует чтения нескольких файлов)

- **Жизненный цикл vault.** `auth::unlock_vault` → `VaultFile::open` (AEAD-проверка
  пароля = расшифровка, верификатора нет) → `UnlockedVault` кладётся в
  `AppState.vault: Mutex<Option<UnlockedVault>>`. Каждая мутация в
  `UnlockedVault` (`service.rs`) заканчивается `persist()` — полное
  перешифрование и атомарная запись (tmp + fsync + rename). Отдельного «save»
  с фронта нет.
- **Два хранилища настроек.** Тема/язык/базовая валюта/избранное — в
  `appconfig.rs` (обычный JSON в конфиг-директории, доступен до разблокировки,
  читается в `App.vue` через `get_app_settings`). Всё доменное, включая кэш цен
  и FX, — внутри зашифрованного vault. Базовая валюта после open/create
  проталкивается в vault через `sync_base_currency`.
- **Формат снапшота.** `SnapshotFormat::for_profile()`: JSON в debug,
  MessagePack (`Pack`) в release — это формат *записи*; при чтении формат
  берётся из байта `payload_fmt` в 55-байтном заголовке. Старый bincode
  (`Bin`) только читается. `UnlockedVault::open` мигрирует старую версию
  схемы (`storage::decode_versioned`), кладёт `<vault>.vN.bak` и сразу
  переписывает файл.
- **Цены.** Команда снимает нужные данные под мьютексом, отпускает его, делает
  `PricingService::fetch_*` (сеть), снова берёт мьютекс и вызывает
  `apply_*`; затем шлёт событие `prices-updated`, на которое фронт
  инвалидирует TanStack Query.
- **Ошибки через IPC.** Rust отдаёт строку `"CODE: message"` (`Error::code()`),
  `parseError` в `src/shared/api/ipc.ts` разбирает её в `{ code, message }`.
- **`pnpm dev` без Tauri.** `call()` в `ipc.ts` при отсутствии
  `__TAURI_INTERNALS__` лениво грузит in-memory мок `src/shared/api/mock/` (пароль — `secret`). Новая
  команда, которую нужно гонять в браузере, требует ветки в этом моке.
- **Добавление команды** затрагивает: DTO в `ipc_contract.rs` → логика в
  `service.rs`/core → хендлер в `crates/app/src/commands/*` →
  `generate_handler![…]` в `crates/app/src/lib.rs` → `mise run gen-types` →
  `docs/ipc.md`. Подробно — скилл `ipc-command`.

## Обучение вместе с фичей

Правило 12 из `AGENTS.md`: видимая пользователю фича выходит с туром,
подсказками и пунктом «Что нового». Перед правкой UI загружай скилл
`onboarding`; гейт проверяет это скриптом `scripts/check-onboarding.mjs`.

## Интерфейс: общие компоненты и исход каждого действия

Правило 15 из `AGENTS.md`. Перед вёрсткой загружай скилл `ui-design-system`
(каталог компонентов, справочник `references/`). Уведомления показывает
`MutationCache` в `src/app/providers/query` по `meta` мутации — `toast` в
компоненте не вызывается. `mise run check-ui` держит базу старых долгов
(`scripts/check-ui.baseline.json`): исправил — `node scripts/check-ui.mjs
--update`.

## Коммиты из Claude Code

Правила jj из `AGENTS.md` действуют полностью: `git commit`/`git add` запрещены,
коммит только по просьбе пользователя. jj трейлер соавтора **не** подставляет —
дописывай сам последним абзацем `Co-authored-by: Claude Code <noreply@anthropic.com>`
(или строку атрибуции, заданную средой), см. скилл `commit-coauthor`.
