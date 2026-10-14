/** What a mock command is, and the reply delay every command shares. */

/** One call of a command: its name, the raw arguments, and their `input` if any. */
interface MockCall {
  cmd: string
  args: Record<string, unknown> | undefined
  body: Record<string, unknown> | undefined
}
export type MockCommands = Record<string, (call: MockCall) => Promise<unknown>>

export function resolve<T>(value: T): Promise<T> {
  return new Promise((res) => setTimeout(() => res(structuredClone(value)), 30))
}
