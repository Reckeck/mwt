/** One line per shifted asset of a correction: a tx can shift several. */
export const lineKey = (shift: CorrectionShift) => `${shift.tx_id}:${shift.asset.symbol}:${shift.asset.contract ?? ''}`
