/// CodeMirror extensions for the raw vault JSON editor.
///
/// Everything mechanical comes from CodeMirror: syntax highlighting, undo
/// history, bracket matching and auto-close, indentation, folding, search and
/// the completion popup with its own key handling. What is added here is
/// (a) a theme mapped onto the app's CSS variables, so light/dark follow the
/// app for free, and (b) one completion source that maps the cursor's JSON
/// path onto the vault schema.
///
/// The path comes from CodeMirror's own Lezer syntax tree, so it is correct by
/// construction — there is no re-implemented string or brace scanning here.

export { vaultJsonCompletions } from './completions'
export { EDITOR_HEIGHT, vaultJsonEditor } from './extensions'
