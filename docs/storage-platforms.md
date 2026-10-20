# Файлы vault на всех платформах

Как приложение находит, открывает, пишет и показывает файл vault на Linux,
Windows, macOS, Android и iOS. Модуль состоит из двух половин:

| Половина | Где | Что делает |
|---|---|---|
| core | `crates/core/src/vault_io/` | без Tauri, под юнит-тестами: локальный файл (атомарная запись, lock, `.bak`), путь или URI, импорт копии, логика зеркала |
| app | `crates/app/src/platform_fs/` | единственное место в app с файловыми диалогами, `content://` и каталогом vault; выбор реализации по ОС |
| Android | `crates/plugin-saf/` | Kotlin-плагин: выбор документа с долговременным правом доступа |

## Главное правило

**Рабочий vault — всегда локальный файл.** Core пишет его через
`vault_io::local::atomic_write` (tmp, fsync, rename в той же папке), держит
рядом `<vault>.lock` и кладёт `.bak` и `.vN.bak`. Всё это требует папки рядом
с файлом. У документа, выбранного через Android Storage Access Framework,
её нет: это `content://…` URI, который `std::fs` не открывает.

Поэтому платформенные различия сведены к трём вопросам:

1. **Откуда взялся файл.** На десктопе используется сам выбранный путь. На
   мобильных выбранный документ копируется в память приложения (`import`).
2. **Где файлы по умолчанию.** Это `platform_fs::vaults_dir` = `app_data_dir()`.
3. **Копия во внешнем файле** (по желанию). После каждого сохранения
   запечатанные байты vault пишутся в выбранный документ (`mirror`).

## Платформы

| | Linux | Windows | macOS | Android | iOS |
|---|---|---|---|---|---|
| `VaultStatus.storage` | `files` | `files` | `files` | `app` | `app` |
| Каталог по умолчанию | `~/.local/share/dev.mwt.desktop/` | `%APPDATA%\dev.mwt.desktop\` | `~/Library/Application Support/dev.mwt.desktop/` | память приложения `dev.mwt.app` | память приложения |
| «Открыть файл» | путь из диалога, файл открывается на месте | так же | так же | `ACTION_GET_CONTENT` → `content://` → копия в каталог по умолчанию | `file://` (security-scoped) → копия |
| «Создать» | путь из диалога сохранения или введённый; голое имя → каталог по умолчанию | так же | так же | только имя → `<каталог>/<имя>.wallet` | так же |
| Недавние | `recent_vaults.json` | так же | так же | `recent_vaults.json` + все `*.wallet` каталога | так же |
| Копия во внешнем файле | любой путь (диалог) | так же | так же | `ACTION_OPEN_DOCUMENT`/`CREATE_DOCUMENT` + `takePersistableUriPermission` (плагин `saf`) | диалог; право может не пережить перезапуск |
| Lock живого процесса | `/proc/<pid>` | lock не проверяется (перехватывается) | так же | `/proc/<pid>` | lock не проверяется |
| Экспорт CSV/JSON | путь | путь | путь | `ACTION_CREATE_DOCUMENT` → запись в URI | URL из диалога |

## Поток данных

```
UI ─ pick_vault_file ─► platform_fs::pick_vault
                         ├ desktop: путь (+ .wallet при сохранении)
                         └ mobile:  URI → read_location → import_into(vaults_dir) → путь копии
   ─ unlock_vault(path) ─► resolve_vault_to_open   (URI из старых недавних тоже импортируется)
                         ─► links::sync_before_open (reconcile с копией: push/pull/SYNC_CONFLICT)
                         ─► UnlockedVault::open     (core: read, lock, миграция, .vN.bak)
                         ─► attach_mirror           (если привязан: поток копирования)
   ─ create_vault(path) ─► resolve_new_vault        (голое имя → vaults_dir, URI запрещён)
мутация → UnlockedVault::persist → dirty + будит vault_writer (пакет)
vault_writer: 500 мс тишины (не дольше 3 с) → prepare_flush (под замком: отметки sync, encode)
            → FlushJob::write (без замка: seal ключом сессии → atomic_write)
            └► VaultMirror::push(bytes) → поток → write_location(URI, "wt")
```

### Пакетная запись

Изменения копятся в памяти и пишутся одним сохранением на пачку
(`crates/core/src/service/saving.rs`, `crates/app/src/vault_writer.rs`).
Чтения идут из памяти, поэтому интерфейс видит изменение сразу.

Принудительно (`persist_now` / `flush_pending`, файл на диске до возврата):
создание vault, смена пароля, смена формата, копия перед заменой документа и
перед синхронизацией, передача vault, привязка внешней копии, блокировка
(при ошибке vault остаётся открытым), переключение на другой vault, выход
(`RunEvent::ExitRequested` — при ошибке выход отменяется и окно предлагает
«Повторить» / «Выйти без сохранения»), скрытие окна в трей и уход страницы в
фон (`visibilitychange` → `flush_vault`: Android может убить процесс в фоне).
`Drop` открытого vault пишет несохранённое последним рубежом.

Порядок записей не откатывается: каждое задание несёт ревизию, общий «шлюз»
помнит последнюю записанную, и запоздавшее старое задание пропускается.
Ошибка записи оставляет пачку несохранённой, писатель повторяет её через 1, 2,
5, 10 с и сообщает `vault-save-failed` (потом `vault-saved`).

**Окно потери:** при обычном выходе, блокировке и уходе в фон — ноль; при
падении процесса или отключении питания — не больше ~3 с последних правок.

## Копия во внешнем файле

- **Хранение ссылки.** `<app config>/vault_links.json`:
  `{ "<путь vault>": { target, last_synced_hash, last_synced_at, last_error } }`.
  Секретов там нет: сама копия — тот же зашифрованный файл.
- **Запись.** `mirror_sink`: свой поток на каждый привязанный vault.
  `persist` держит мьютекс vault, а облачный провайдер может писать секунды,
  поэтому запись идёт в фоне. Накопившиеся сохранения схлопываются в
  последнее. Ошибка записи сохраняется в `last_error` и приходит событием
  `vault-mirror-failed`; локальный vault при этом не страдает.
- **Чтение при входе.** `vault_io::mirror::reconcile(local, remote, last_synced_hash)`:

| Что изменилось с прошлой синхронизации | Действие |
|---|---|
| ничего, байты равны | `InSync` |
| только локальный (или копии нет) | `PushLocal` — записать копию |
| только копия (правка на другом устройстве) | `PullRemote` — локальный в `.bak`, копия становится vault |
| оба | `Conflict` → ошибка `SYNC_CONFLICT`, UI предлагает «Оставить это» / «Взять копию» (`resolve_vault_conflict`) |

  Недоступная копия (отозвано право, диск офлайн) не мешает войти: ошибка
  пишется в `last_error`, открывается локальный файл.
- **Привязка.** `link_vault_external { existing }`. При `existing = true`
  выбранный файл **заменяется** текущим vault (UI говорит об этом в
  подсказке); при `false` создаётся новый. Подтянуть чужую копию можно
  открытием файла (импорт) или через `PullRemote` при входе.

## Связи с другими модулями

- `storage` и `crypto` реэкспортируют `vault_io::local` (`atomic_write`,
  `read_all`, `VaultLock`, `backup_path`, `ensure_wallet_ext`, `VAULT_EXT`):
  старые пути импорта работают.
- `service::UnlockedVault` знает только про `VaultMirror` (трейт) и
  `set_mirror`; sandbox зеркало не получает.
- `commands/auth.rs` и `commands/vault_recents.rs` вызывают `platform_fs`
  вместо `PathBuf::from` и диалогов. `commands/vault_link.rs` — тонкие
  обёртки над `platform_fs::links`.
- `analytics::export_report_csv` и `settings::dev_export_vault` пишут через
  `platform_fs::save_bytes_dialog`.
- `app_dirs`/`legacy.rs` (перенос `dev.tradeapp.*`) не изменились: это
  разовый перенос каталогов на десктопе.
- `appconfig`, `notify_store`, `recent_vaults.json`, `vault_links.json` лежат
  в `app_config_dir()`: это обычные файлы приложения на всех ОС.

## Проверка

- `cargo test -p trade-core vault_io::`: пути всех ОС, `file://`,
  `content://`, имя файла, импорт без перезаписи, все ветки `reconcile`.
- `cargo test -p trade-core a_mirror_gets_exactly_the_saved_file`.
- `cargo test -p trade-app platform_fs`.
- `cargo clippy -p trade-app --lib --target aarch64-linux-android -- -D warnings`
  (нужен NDK) или `ANDROID_TARGETS=aarch64 mise run build:all -- android`.
- `pnpm dev`, затем `/unlock?storage=app`: экраны Android/iOS в моке; путь
  с `conflict` отвечает `SYNC_CONFLICT`.
