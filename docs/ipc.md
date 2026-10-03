# IPC-контракт (frontend ↔ backend)

Все команды — `#[tauri::command]` в `crates/app/src/commands/`. Типы — из
`crates/core/src/ipc_contract.rs` и домена core, генерируются ts-rs в
`src/lib/ipc.gen.d.ts` через `mise run gen-types`.

Ошибки: команды возвращают `Result<T, String>`; строка —
`ERR_CODE: message`, где `ERR_CODE` — метод `Error::code()`
(например `CRYPTO`, `MISSING`, `INVALID_INPUT`, `REQUIRES_UNLOCK`, `NOT_FOUND`).
Frontend парсит код и показывает локализованное сообщение.

## События

| Событие | Payload | Когда |
|---|---|---|
| `prices-updated` | `RefreshResult` | после `refresh_all` |
| `bybit-import-pending` | `{ staged: number }` | фоновый опрос Bybit нашёл новые сделки (D2.4) |

## Команды

### auth (`crates/app/src/commands/auth.rs`)
| Команда | Вход | Выход |
|---|---|---|
| `vault_status` | — | `VaultStatus { exists, unlocked, path, format }` |
| `create_vault` | `CreateVaultInput { path, password }` | `VaultInfo` |
| `unlock_vault` | `UnlockInput { path, password }` | `VaultInfo` |
| `lock_vault_cmd` | — | `()` |
| `change_password` | `ChangePasswordInput { current_password, new_password }` | `()` |

### stores
| `list_stores` | — | `Vec<Store>` |
| `create_store` | `CreateStoreInput { name, kind, color, note }` | `Store` |
| `update_store` | `UpdateStoreInput { id, name, color, note }` | `Store` |
| `archive_store` | `id, archived` | `()` (soft delete) |
| `delete_store` | `id` | `()` (каскадно чистит транзакции/holdings) |

### transactions
| `list_transactions` | `store_id?` | `Vec<Transaction>` |
| `add_transaction` | `TxnInput` | `Transaction` (income/expense) |
| `add_transfer` | `TransferInput { from_store_id, to_store_id, asset, amount, fee, network, tx_hash, comment, category }` | `[Transaction; 2]` (out+in, один `group_id`) |
| `add_trade` | `TradeInput { store_id, buy, sell, ... , comment }` | `[Transaction; 2]` (buy+sell legs) |
| `delete_transaction` | `id` | `()` |
| `update_comment` | `UpdateCommentInput { transaction_id, comment }` | `Transaction` |

### categories
| `add_category` | `category` | `String` (добавленная) |
| `list_categories` | — | `Vec<String>` |

### pricing
| `search_assets` | `query` | `Vec<AssetSearchHit { symbol, name, coingecko_id?, chain?, contract?, rank }>` |
| `refresh_all` | — | `RefreshResult { prices, fx, updated_at }` + событие |
| `open_explorer` | `url` | `()` (default browser) |
| `set_tx_prices` | `SetTxPricesInput { transaction_id, price_usd?, fx_override? }` | `()` |
| `get_cached_price` | `AssetPriceInput { symbol, chain?, contract? }` | `CachedPrice { price_usd, ts }` |
| `get_candles` | `GetCandlesInput { symbol, interval_min, limit≤1000 }` | `GetCandlesOutput { candles: Vec<CandlePoint { ts, open, high, low, close, volume }> }` (Binance klines, **без vault**) |

### portfolio
| `get_summary` | — | `PortfolioSummary { base_currency, fx_per_usd, total_usd, total_base, assets, by_chain, by_store }` |
| `get_history` | `base_currency?` | `Vec<HistoryPoint { ts, ts_ms, value_usd, value_base }>` |
| `get_realized_pnl` | — | `RealizedPnl` |
| `get_store_overview` | `store_id` | `StoreOverview { store, base_currency, fx_per_usd, total_usd, total_base, realized_pnl_base, positions, tx_count, first_tx_at, last_tx_at }` |
| `get_store_history` | `store_id, base_currency?` | `Vec<HistoryPoint>` |
| `get_store_pnl` | `store_id` | `RealizedPnl { total_base, legs[] }` |

### calc
| `calc_preview` | `CalcInput { mode, amount, price_in, price_out, target_price?, stop_price?, fee_pct? }` | `CalcResult { gross_in, fee, spent, gross_out, net_out, exit_value, pnl, roi_pct, risk, risk_pct }` |

### settings
| `get_settings` | — | `Settings` |
| `update_settings` | `UpdateSettingsInput` | `Settings` (в т.ч. `bybit_import_interval_sec`: `0` — выкл, иначе клампится в 60..86400) |

### sim (Strategy Lab)
| `save_strategy` | `SaveStrategyInput { strategy }` | `Strategy` |
| `list_strategies` | — | `Vec<Strategy>` |
| `delete_strategy` | `id` | `()` |
| `list_runs` | — | `Vec<SimRun>` |
| `run_strategy` | `RunStrategyInput { strategy_id, granularity_min, candles }` | `SimRun` |
| `get_run_candles` | `GetRunCandlesInput { run_id }` | `GetRunCandlesOutput { candles: Vec<CandlePoint> }` (синтетика — воспроизводится по `candle_params` (seed), реал — рефетч из Binance за диапазон прогна) |
| `get_gbm_paths` | `GbmPathsInput { run_id, paths≤20, horizon≤500 }` | `GbmPathsOutput { paths: Vec<Vec<GbmPathPoint { ts, price }>> }` (driftless GBM фан-чарт от `finished_at`; σ — из `candle_params` или realised vol на реальных свечах) |

### exchange (только чтение)
| `inspect_tx` | `InspectTxInput { tx_hash, network? }` | `InspectTxResult { fetched, info?, hints[], notice? }` |
| `bybit_executions` | `BybitExecutionsInput { category, symbol?, limit?, store_id? }` | `BybitExecutionsResult { executions[], notice? }` |
| `exchange_key_status` | `ExchangeKeyStatusInput { store_id }` | `ExchangeKeyStatus { configured, hint? }` |
| `set_exchange_key` | `SetExchangeKeyInput { store_id, api_key, api_secret }` | `ExchangeKeyStatus` |
| `clear_exchange_key` | `ClearExchangeKeyInput { store_id }` | `ExchangeKeyStatus` |

### Bybit import — staging (D2.4)
| `import_bybit_history` | `ImportBybitHistoryInput { store_id, category?, limit? }` | `ImportBybitHistoryOutput { staged, notice? }` — скачивает сделки и **кладёт их в staging**, операции не создаёт |
| `list_pending_imports` | — | `ListPendingImportsOutput { imports: Vec<PendingImport> }` (новые сверху) |
| `confirm_pending_imports` | `ConfirmPendingImportsInput { exec_ids }` (пусто = все) | `ConfirmPendingImportsOutput { confirmed }` — превращает staging в сделки (2 ноги, как `add_trade`) |
| `discard_pending_imports` | `DiscardPendingImportsInput { exec_ids }` (пусто = все) | `DiscardPendingImportsOutput { discarded }` |

Инварианты блока:
- **Ни одна команда не создаёт операции сама.** Только `confirm_pending_imports`;
  фоновый опрос (`bybit_import_interval_sec`, `0` = выкл) кладёт сделки в
  `pending_imports` и шлёт событие — пользователь решает сам.
- Дедупликация по `execId` (`PendingImport.exec_id`), поэтому повторный импорт
  и перезапуски фонового опроса не создают дублей.
- Мусорные сделки (нет `execId`, нечитаемые количество/цена, нулевое
  количество) отбрасываются на границе и в staging не попадают.
- `notice` — код причины: `no_key` | `unreachable` | `rate_limited` |
  `forbidden_region`. Ошибок на сетевых сбоях этот блок не бросает.

Команды этого блока **не бросают ошибок** для сетевых сбоев: вместо этого
возвращают пустой список и `notice`. Форма сделки остаётся ровно такой, какой
её оставил пользователь.

- `notice` для `inspect_tx`: `empty` | `unavailable` | `choose_network`.
- `notice` для `bybit_executions`: `no_key` | `unreachable` | `rate_limited` |
  `forbidden_region`.
- `store_id` в `bybit_executions` опционален: если не указан — подъём по предкам
  к узлу-бирже (Phase N).
- Секрет API никогда не возвращается наружу: `ExchangeKeyStatus` содержит только
  маску вида `abcd…wxyz`.

## Доменные типы в IPC (важно для UI)

- `Chain` — union строк; `Chain`/`ChainInfo` метаданные (CoinGecko id, explorer
  patterns) не передаются по IPC, а живут во фронтенд-константах (см. модель).
- `Amount` — в TS это просто `string` (все денежные поля).
- `StoreKind` — `"exchange" | "hot_wallet" | "cold_wallet" | "bank" | "cash" | "paper"`.
- `TxKind` — `"income" | "expense" | "transfer" | "trade" | "fee" | "paper_trade"`.

## Добавление новой команды (чеклист)

1. Тип входных данных в `ipc_contract.rs` (`#[derive(Serialize, Deserialize, TS)]`).
2. Метод `UnlockedVault` в `service.rs` + юнит-тест.
3. Хендлер в `crates/app/src/commands/<модуль>.rs` + регистрация в
   `generate_handler!` (`lib.rs`).
4. `mise run gen-types` → новый тип в `src/lib/ipc.gen.d.ts`.
5. Если команда async и держит vault — скопировать данные до `.await` (правило №3 из AGENTS.md).