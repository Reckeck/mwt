/// JSON helpers for the vault editor.
///
/// Deliberately not a parser: the document is parsed by CodeMirror's JSON
/// grammar, and these two helpers only answer the two questions the save
/// button needs — is the draft well-formed, and is it different from what is
/// already on disk?

export interface ParseResult {
  ok: boolean
  error: string | null
}

/** Parse check that keeps the engine's message, which names the bad offset. */
export function parseJson(input: string): ParseResult {
  try {
    JSON.parse(input)
    return { ok: true, error: null }
  } catch (err) {
    return { ok: false, error: err instanceof Error ? err.message : String(err) }
  }
}

/**
 * Canonical form for comparing two JSON texts, or `null` when unparsable.
 *
 * Whitespace and key order must not count as an edit, or reformatting the
 * document would leave the status permanently "not in sync" — the backend
 * always answers with its own canonical formatting, which will never match
 * what the user typed.
 */
export function normalizeJson(input: string): string | null {
  try {
    return JSON.stringify(JSON.parse(input))
  } catch {
    return null
  }
}
