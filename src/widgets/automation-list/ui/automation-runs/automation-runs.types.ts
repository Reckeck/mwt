export interface AutomationRunsProps {
  runs: AutomationRun[]
  /** The automation's name, for the `How much arrived` dialog. */
  name: string
  /** A wallet's name by id, for the refusal text. */
  storeName: (id: string) => string
}
