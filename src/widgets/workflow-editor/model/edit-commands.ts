/** The editing commands, shared by the shortcuts and the toolbar buttons. */
export interface EditCommandSet {
  undo: () => void
  redo: () => void
  duplicate: () => void
  copy: () => void
  paste: () => void
  canUndo: () => boolean
  canRedo: () => boolean
}
