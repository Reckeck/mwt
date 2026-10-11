export interface AutomationSimulationDialogProps {
  /** One automation to look at; `null` = all of them. */
  automationId?: string | null
  /** Title of that automation, for the header. */
  name?: string
}
