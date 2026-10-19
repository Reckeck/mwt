import type { Component } from 'vue'

/**
 * One thing the user can do with an object. The same list feeds the
 * right-click menu and the «⋯» button, so the two never drift apart.
 */
export interface UiAction {
  id: string
  label: string
  icon?: Component
  /** Shown on the right, e.g. `Ctrl+E`. */
  shortcut?: string
  /** Red text and icon; such items go last, after a separator. */
  destructive?: boolean
  /** Hidden when it does not apply to this object at all. */
  hidden?: boolean
  /** Shown but unavailable; a string is the reason, shown as the tooltip. */
  disabled?: boolean | string
  /** A separator is drawn above this item. */
  separated?: boolean
  run?: () => void
  /** A sub-menu instead of an action. */
  children?: UiAction[]
}

export interface ContextMenuProps {
  actions: UiAction[]
  /** Turns the menu off (the native one stays off too). */
  disabled?: boolean
  /**
   * The actions are filled in by the right click itself (one menu over a whole
   * list, each row says which item it is): an empty list does not turn it off.
   */
  lazy?: boolean
}
