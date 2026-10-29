/** Shapes of the `What's new` list in `whats-new.ts`. */
export interface WhatsNewPoint {
  /** i18n key under `whatsNew.<entry id>.`. */
  key: string
  /**
   * Route whose tour shows it, or `settings:<tab>` for a settings tab;
   * omitted when there is nothing to point at.
   */
  route?: string
}

export interface WhatsNewEntry {
  id: string
  /** `YYYY-MM-DD`. */
  date: string
  points: WhatsNewPoint[]
}
