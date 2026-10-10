/** Cell colours by how a day went against its allowance. */
export const STATUS_CLASSES: Record<DayStatus, string> = {
  none: 'border-border',
  future: 'border-dashed border-border text-muted-foreground',
  under: 'border-emerald-500/30 bg-emerald-500/10',
  near: 'border-amber-500/40 bg-amber-500/15',
  over: 'border-destructive/50 bg-destructive/15',
}
