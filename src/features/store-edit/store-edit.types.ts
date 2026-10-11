export interface StoreFormProps {
  /** The wallet being edited, as the overview returned it. */
  store: Store
}

export interface StoreDangerZoneProps extends StoreFormProps {
  childCount: number
  txCount: number
}
