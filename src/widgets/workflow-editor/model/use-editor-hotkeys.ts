import type { Ref } from 'vue'
import { useEventListener } from '@vueuse/core'

import { isTyping, keyLetter, overlayOpen } from './hotkeys'

export interface EditorHotkeyActions {
  save: (event: KeyboardEvent) => void
  emulate: () => void
  toggleHelp: () => void
  quickAdd: () => void
  undo: () => void
  redo: () => void
  duplicate: () => void
  copy: () => void
  paste: () => void
  toggleSelected: () => void
  selectAll: () => void
  fit: () => void
  remove: () => void
}

/**
 * The editor's shortcuts. They listen on the window, so they work wherever the
 * focus is after a click on the canvas; letters go by key position, so they
 * work on any layout. `paused` is true while the node search owns the keys.
 */
export function useEditorHotkeys(root: Ref<HTMLElement | null>, paused: () => boolean, actions: EditorHotkeyActions) {
  function onKeydown(event: KeyboardEvent) {
    if (event.defaultPrevented || !root.value || overlayOpen(root.value)) return
    // The node search has its own keys: Enter there adds the node it found, so
    // Ctrl+Enter must not also start an emulation behind it.
    if (paused()) return
    const target = event.target as Node | null
    if (target instanceof HTMLElement && target !== document.body && !root.value.contains(target)) return
    const ctrl = event.ctrlKey || event.metaKey
    const key = keyLetter(event)
    if (ctrl && key === 's') {
      event.preventDefault()
      actions.save(event)
      return
    }
    if (ctrl && key === 'enter') {
      event.preventDefault()
      actions.emulate()
      return
    }
    if (event.key === 'F1') {
      event.preventDefault()
      actions.toggleHelp()
      return
    }
    if (ctrl && event.code === 'Space') {
      event.preventDefault()
      actions.quickAdd()
      return
    }
    if (isTyping(event)) return
    if (!ctrl && event.shiftKey && key === 'a') {
      // Ctrl+Space is often taken by the input method switcher.
      event.preventDefault()
      actions.quickAdd()
    } else if (ctrl && key === 'z') {
      event.preventDefault()
      if (event.shiftKey) actions.redo()
      else actions.undo()
    } else if (ctrl && key === 'y') {
      event.preventDefault()
      actions.redo()
    } else if (ctrl && key === 'd') {
      event.preventDefault()
      actions.duplicate()
    } else if (ctrl && key === 'c') {
      actions.copy()
    } else if (ctrl && key === 'v') {
      actions.paste()
    } else if (ctrl && key === 'e') {
      event.preventDefault()
      actions.toggleSelected()
    } else if (ctrl && key === 'a') {
      event.preventDefault()
      actions.selectAll()
    } else if (!ctrl && !event.shiftKey && key === 'f') {
      actions.fit()
    } else if (event.key === 'Delete' || event.key === 'Backspace') {
      event.preventDefault()
      actions.remove()
    }
  }
  useEventListener(window, 'keydown', onKeydown)
}

/** A click on the canvas leaves the name or a field, so the shortcuts work again. */
export function leaveFields(event: PointerEvent) {
  const focused = document.activeElement as HTMLElement | null
  if (!focused || focused === document.body) return
  if ((event.target as HTMLElement | null)?.closest('input, textarea, select, [contenteditable="true"]')) return
  if (['INPUT', 'TEXTAREA', 'SELECT'].includes(focused.tagName) || focused.isContentEditable) focused.blur()
}
