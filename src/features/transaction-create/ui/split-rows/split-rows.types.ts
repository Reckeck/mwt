import type { ConvertMode } from '../../transaction-form.types'

export interface SplitRowsProps {
  /** `convert` or `topup`. */
  kind: TxKind
  mode: ConvertMode
  /** Convert: the asset given, for the «give» fields' hint. */
  giveAsset?: AssetRef | null
}
