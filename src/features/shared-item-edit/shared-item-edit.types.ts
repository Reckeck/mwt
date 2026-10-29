export interface SharedItemEditDialogProps {
  event: SharedEventView
  /** The item to change; none adds a new planned item. */
  item?: EventItem | null
}
