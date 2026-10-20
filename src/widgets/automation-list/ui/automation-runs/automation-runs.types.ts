export interface AutomationRunsProps {
  runs: AutomationRun[]
  /** The automation's name, for the «Сколько пришло» dialog. */
  name: string
  /** A wallet's name by id, for the refusal text. */
  storeName: (id: string) => string
}
