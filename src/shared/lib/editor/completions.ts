/** Completion from the vault schema: field names for an object, values for a field. */

import type { Completion, CompletionContext, CompletionResult } from '@codemirror/autocomplete'
import type { EditorView } from '@codemirror/view'
import { descriptionOf, type JsonSchemaReader, type JsonSchema, type LeafKind } from '../json-schema-reader'
import { enclosingKeys, inBareArray, insideString, siblingKeys } from './json-path'

/** Replace `[from, to)` and put the caret at `caret` inside the new text. */
function replaceWith(text: string, caret: number) {
  return (view: EditorView, _completion: Completion, from: number, to: number) => {
    view.dispatch({
      changes: { from, to, insert: text },
      selection: { anchor: from + caret },
      userEvent: 'input.complete',
    })
  }
}

/** A literal that needs no schema lookup, just a caret inside the text. */
function literal(label: string, type: string, detail: string, caret: number): Completion {
  return { label, type, detail, apply: replaceWith(label, caret) }
}

/** Default value for a completed field, and where the caret lands inside it. */
function valueInsertion(kind: LeafKind, values: readonly string[]): { text: string; caret: number } {
  switch (kind) {
    case 'object':
      return { text: '{}', caret: 1 }
    case 'array':
      return { text: '[]', caret: 1 }
    case 'enum': {
      const first = values[0] ?? ''
      return { text: `"${first}"`, caret: 1 + Math.max(1, first.length - 1) }
    }
    case 'number':
      return { text: '0', caret: 0 }
    case 'boolean':
      return { text: 'false', caret: 0 }
    case 'string':
      return { text: '""', caret: 1 }
    default:
      return { text: 'null', caret: 0 }
  }
}

/**
 * Values worth offering for a field.
 *
 * `quoted` is false when the caret is already inside a string: the quotes are
 * part of the document there, so inserting `"dark"` would give `""dark""`.
 */
function valueCompletions(kind: LeafKind, values: readonly string[], nullable: boolean, quoted: boolean): Completion[] {
  if (kind === 'enum') {
    return values.map((value) => {
      const text = quoted ? `"${value}"` : value
      return {
        label: value,
        type: 'value',
        detail: nullable ? 'value or null' : 'value',
        apply: replaceWith(text, Math.min(text.length, quoted ? 1 + Math.max(1, value.length - 1) : value.length)),
      }
    })
  }
  switch (kind) {
    case 'boolean':
      return [literal('true', 'keyword', 'boolean', 4), literal('false', 'keyword', 'boolean', 5)]
    case 'string':
      return nullable
        ? [literal('""', 'text', 'empty string', 1), literal('null', 'keyword', 'no value', 4)]
        : [literal('""', 'text', 'empty string', 1)]
    case 'object':
      return [literal('{}', 'object', 'empty object', 1)]
    case 'array':
      return [literal('[]', 'array', 'empty array', 1)]
    case 'number':
      return [literal('0', 'number', 'number', 1)]
    default:
      return nullable ? [literal('null', 'keyword', 'no value', 4)] : []
  }
}

/** Field-name completion for the object enclosing the cursor. */
function keyCompletions(reader: JsonSchemaReader, node: JsonSchema, used: ReadonlySet<string>): Completion[] {
  const properties = reader.properties(node)
  return Object.keys(properties)
    .filter((name) => !used.has(name))
    .map((name) => {
      const field = properties[name]
      const kind = reader.leafKind(field)
      const values = reader.enumValues(field)
      const insertion = valueInsertion(kind, values)
      // `"name": value` — the caret ends up inside the value.
      const text = `${JSON.stringify(name)}: ${insertion.text}`
      const detail = descriptionOf(field) ?? reader.describe(field)
      return {
        label: name,
        type: 'property',
        detail,
        apply: replaceWith(text, name.length + 4 + insertion.caret),
      }
    })
}

/**
 * Schema-driven completion for the vault document.
 *
 * Exported so it can be exercised headlessly against a plain `EditorState`,
 * without standing up a DOM.
 */
export function vaultJsonCompletions(context: CompletionContext, reader: JsonSchemaReader): CompletionResult | null {
  const state = context.state
  // Nothing sensible fits between an array's brackets.
  if (inBareArray(state, context.pos)) return null

  const keys = enclosingKeys(state, context.pos)
  const word = context.matchBefore(/[\w-]*/)
  const from = word ? word.from : context.pos
  const options = { validFor: /^[\w-]*$/ }

  // Walk the document path into the schema. Arrays are transparent: a key
  // nested inside an array of objects belongs to the element.
  let node = reader.rootSchema
  for (const key of keys) {
    node = reader.concrete(node)
    if (reader.leafKind(node) === 'array') node = reader.concrete((node.items as JsonSchema) ?? {})
    if (reader.leafKind(node) !== 'object') return null
    const properties = reader.properties(node)
    if (!(key in properties)) return null
    node = properties[key]
  }

  // The last key may have landed on an array while the caret actually sits in
  // one of its element objects, so descend into the element.
  if (reader.leafKind(node) === 'array') {
    node = reader.concrete((node.items as JsonSchema) ?? {})
  }

  const resolved = reader.concrete(node)
  const kind = reader.leafKind(node)

  // A leaf at the caret means a value position: offer what the field accepts.
  if (kind !== 'object') {
    const quoted = !insideString(state, context.pos)
    // Inside an existing string only enumerated values are worth offering; a
    // free-text field has nothing to suggest there.
    if (!quoted && kind !== 'enum') return null
    return { ...options, from, options: valueCompletions(kind, reader.enumValues(node), reader.nullable(node), quoted) }
  }

  // Otherwise the caret is inside an object: offer its fields.
  return { ...options, from, options: keyCompletions(reader, resolved, new Set(siblingKeys(state, context.pos))) }
}
