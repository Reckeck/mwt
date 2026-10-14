/** The editor itself: every CodeMirror extension it runs with, wired to the completions and the save key. */

import {
  autocompletion,
  closeBrackets,
  closeBracketsKeymap,
  closeCompletion,
  completionKeymap,
  currentCompletions,
} from '@codemirror/autocomplete'
import { defaultKeymap, history, historyKeymap, indentLess, indentMore } from '@codemirror/commands'
import { json, jsonParseLinter } from '@codemirror/lang-json'
import { bracketMatching, foldGutter, foldKeymap, indentOnInput, syntaxHighlighting } from '@codemirror/language'
import { lintKeymap, linter } from '@codemirror/lint'
import { highlightSelectionMatches, searchKeymap } from '@codemirror/search'
import { EditorState, type Extension } from '@codemirror/state'
import {
  drawSelection,
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
  rectangularSelection,
} from '@codemirror/view'
import type { JsonSchemaReader } from '../json-schema-reader'
import { vaultJsonCompletions } from './completions'
import { vaultHighlight, vaultJsonTheme } from './theme'

/** Height the component hands the editor; the view fills its parent. */
export const EDITOR_HEIGHT = '60dvh'

/**
 * Dismiss the popup when the caret moves away from it.
 *
 * Arrow keys are deliberately excluded: while the list is open they navigate
 * it, which is the usual editor behaviour. A click inside the document, or one
 * of the jump keys, is an unambiguous intent to move the caret instead.
 */
const dismissOnCaretMove = EditorView.domEventHandlers({
  mousedown: (_event, view) => {
    if (currentCompletions(view.state)) closeCompletion(view)
    return false
  },
  keyup: (event, view) => {
    if (!currentCompletions(view.state)) return false
    const jumps = ['Home', 'End', 'PageUp', 'PageDown']
    if (jumps.includes(event.key)) closeCompletion(view)
    return false
  },
})

/**
 * The editor: language, theme, history, search, folding and completion.
 *
 * `reader` wraps the schema fetched from the backend and is what the
 * completion source consults, so the document shape is never written down on
 * this side.
 */
export function vaultJsonEditor(reader: JsonSchemaReader, onSave: () => void): Extension {
  return [
    lineNumbers(),
    highlightActiveLineGutter(),
    highlightActiveLine(),
    highlightSelectionMatches(),
    drawSelection(),
    rectangularSelection(),
    history(),
    foldGutter(),
    indentOnInput(),
    bracketMatching(),
    closeBrackets(),
    EditorState.allowMultipleSelections.of(true),
    json(),
    // Without this the tokens render in generated, unstyleable classes and the
    // document looks like plain text.
    syntaxHighlighting(vaultHighlight),
    EditorView.lineWrapping,
    vaultJsonTheme,
    dismissOnCaretMove,
    autocompletion({
      override: [(context) => vaultJsonCompletions(context, reader)],
      // Opening stays explicit: Ctrl+Space, or the idle timer in the component.
      activateOnTyping: false,
    }),
    // Marks malformed JSON inline with the engine's own message.
    linter(jsonParseLinter()),
    keymap.of([
      // Saving is explicit; `preventDefault` also stops the webview's own
      // "save page" shortcut from firing.
      {
        key: 'Mod-s',
        preventDefault: true,
        run: () => {
          onSave()
          return true
        },
      },
      ...closeBracketsKeymap,
      // Tab indents. It comes before `defaultKeymap` so it wins, and it is not
      // part of `completionKeymap` — Tab never accepted a suggestion here, only
      // Enter does.
      { key: 'Tab', run: indentMore, shift: indentLess },
      ...defaultKeymap,
      ...searchKeymap,
      ...historyKeymap,
      ...foldKeymap,
      ...completionKeymap,
      ...lintKeymap,
    ]),
  ]
}
