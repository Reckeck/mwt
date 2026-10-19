export interface ListHeaderProps {
  /** One line on what the list holds and why. */
  intro: string
  /** The main action ("Новая цель"); omit for a list without one. */
  actionLabel?: string
  /**
   * Hide the main action: an empty list shows it once, in its empty state,
   * rather than twice on the screen.
   */
  hideAction?: boolean
}
