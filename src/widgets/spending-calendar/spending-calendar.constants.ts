/** Cell colours by how a day went against its allowance. */
export const STATUS_CLASSES: Record<DayStatus, string> = {
  none: 'border-border',
  future: 'border-dashed border-border text-muted-foreground',
  under: 'border-success/30 bg-success/10',
  near: 'border-warning/40 bg-warning/15',
  over: 'border-destructive/50 bg-destructive/15',
}
