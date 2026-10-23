export interface AttachmentsBlockProps {
  owner: EntityRef
  /** «к операции RUB → USDT · 21 окт»: what a dropped file lands on. */
  ownerLabel: string
  /** The element a dragged file lights up and lands on (the whole card); the block itself without it. */
  dropTarget?: HTMLElement | null
  /** Listen for Ctrl V and drops only while the card is shown. */
  active?: boolean
}

/** A file being sealed and saved: its row has a bar and «Отменить». */
export interface PendingFile {
  key: number
  name: string
  cancelled: boolean
}

/** A file that did not make it: a red row with the reason and «Убрать». */
export interface FailedFile {
  key: number
  name: string
  reason: string
}
