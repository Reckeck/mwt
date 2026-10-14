/// The in-memory backend for `pnpm dev` without the desktop shell (password `secret`).
///
/// Loaded lazily by `call()` only outside Tauri, so none of it ships in the app.
/// A command the UI should show in the browser needs a branch here.

export { mock } from './dispatch'
