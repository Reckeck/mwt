export interface AssetSelectProps {
  /** The assets to choose from: what a wallet holds, say. */
  assets: AssetRef[]
  /** Hint shown while nothing is picked, not choosable itself. */
  placeholder?: string
  /** Marks the field as wrong (red border), next to an error under it. */
  invalid?: boolean
}
