export type JournalTone = 'text-success' | 'text-warning' | 'text-destructive' | 'text-muted-foreground'

const TONE: Record<JournalStatus, JournalTone> = {
  done: 'text-success',
  asking: 'text-warning',
  waiting: 'text-muted-foreground',
  partial: 'text-warning',
  failed: 'text-destructive',
  stopped: 'text-muted-foreground',
  rejected: 'text-destructive',
  skipped: 'text-muted-foreground',
  shown: 'text-muted-foreground',
}

/** The colour of a run's outcome word: green done, amber needs you, red broke. */
export const journalTone = (status: JournalStatus): JournalTone => TONE[status]
