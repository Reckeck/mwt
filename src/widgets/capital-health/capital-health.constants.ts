/** The light's dot, one per colour: the checkup screen, its dashboard widget and the today strip share it. */
export const HEALTH_DOT: Record<HealthColor, string> = {
  red: 'bg-destructive',
  yellow: 'bg-warning',
  green: 'bg-success',
  no_data: 'bg-muted-foreground',
}
