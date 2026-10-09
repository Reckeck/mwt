export type TooltipSide = 'top' | 'right' | 'bottom' | 'left'

export type TooltipAlign = 'start' | 'center' | 'end'

export interface TooltipProps {
  side?: TooltipSide
  align?: TooltipAlign
  sideOffset?: number
}
