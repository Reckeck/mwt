/** The editor in the app's colours: syntax highlight and chrome on the CSS variables, so light/dark follow. */

import { tags as t } from '@lezer/highlight'
import { HighlightStyle } from '@codemirror/language'
import { EditorView } from '@codemirror/view'

/**
 * Token colours.
 *
 * This has to be a `HighlightStyle` plus `syntaxHighlighting()`, not a set of
 * `.cm-*` rules in the theme: with no highlighter registered, CodeMirror falls
 * back to emitting generated class names that cannot be targeted from CSS, and
 * the document renders uncoloured.
 *
 * Colours are CSS variables, so a `.dark` ancestor switches the editor together
 * with the app and no JS has to know which theme is active.
 */
export const vaultHighlight = HighlightStyle.define([
  { tag: t.propertyName, color: 'var(--syntax-key)' },
  { tag: [t.string, t.special(t.string)], color: 'var(--syntax-string)' },
  { tag: [t.number, t.integer, t.float], color: 'var(--syntax-number)' },
  { tag: [t.bool, t.atom], color: 'var(--syntax-literal)' },
  { tag: [t.null, t.comment], color: 'var(--syntax-comment)', fontStyle: 'italic' },
  { tag: [t.keyword, t.operator, t.modifier], color: 'var(--syntax-literal)' },
  { tag: [t.punctuation, t.separator, t.bracket], color: 'var(--muted-foreground)' },
  { tag: [t.definition(t.variableName), t.variableName], color: 'var(--foreground)' },
  { tag: t.invalid, color: 'var(--syntax-invalid)', textDecoration: 'underline wavy' },
])

/**
 * Theme referencing only CSS variables, so a `.dark` ancestor switches the
 * editor together with the app and no JS has to know which theme is active.
 */
export const vaultJsonTheme = EditorView.theme({
  '&': {
    height: '100%',
    color: 'var(--foreground)',
    backgroundColor: 'transparent',
    fontSize: '0.75rem',
  },
  '&.cm-focused': { outline: 'none' },
  '.cm-content': { caretColor: 'var(--foreground)', padding: '0.75rem' },
  '.cm-line': { padding: '0' },
  '.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--foreground)', borderLeftWidth: '2px' },
  '.cm-selectionBackground, .cm-content ::selection': {
    backgroundColor: 'var(--syntax-selection)',
  },
  '.cm-activeLine': { backgroundColor: 'var(--syntax-active-line)' },
  '.cm-gutters': {
    backgroundColor: 'transparent',
    color: 'var(--muted-foreground)',
    border: 'none',
    borderRight: '1px solid var(--border)',
  },
  '.cm-activeLineGutter': { backgroundColor: 'var(--syntax-active-line)' },
  // Popup and panel chrome, mapped onto the same variables.
  '.cm-tooltip': {
    backgroundColor: 'var(--popover)',
    color: 'var(--popover-foreground)',
    border: '1px solid var(--border)',
    borderRadius: '0.5rem',
    overflow: 'hidden',
  },
  '.cm-tooltip-autocomplete > ul': { fontFamily: 'inherit', fontSize: '0.75rem', maxHeight: '14rem' },
  '.cm-tooltip-autocomplete > ul > li': { padding: '2px 8px' },
  '.cm-tooltip-autocomplete > ul > li[aria-selected]': {
    backgroundColor: 'var(--accent)',
    color: 'var(--accent-foreground)',
  },
  '.cm-completionDetail': { marginLeft: '8px', opacity: '0.55', fontStyle: 'normal' },
  '.cm-completionMatchedText': { textDecoration: 'none', color: 'var(--primary)', fontWeight: '600' },
  '.cm-panels': { backgroundColor: 'var(--popover)', color: 'var(--popover-foreground)' },
  '.cm-panels input, .cm-panels button': {
    backgroundColor: 'var(--card)',
    color: 'var(--card-foreground)',
    border: '1px solid var(--border)',
    borderRadius: '0.25rem',
  },
  '.cm-searchMatch': { backgroundColor: 'var(--syntax-active-line)' },
  '.cm-searchMatch.cm-searchMatch-selected': { backgroundColor: 'var(--syntax-selection)' },
})
