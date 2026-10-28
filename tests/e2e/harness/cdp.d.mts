export interface Page {
  send(method: string, params?: Record<string, unknown>): Promise<{ result?: any }>
  sleep(ms: number): Promise<void>
  /** Run `code` (an async function body) in the page; returns its JSON value, throws on a page error. */
  eval<T = any>(code: string): Promise<T>
  goto(url: string): Promise<void>
  viewport(width: number, height: number, touch: boolean): Promise<void>
  mouse(x: number, y: number): Promise<void>
  shot(path: string): Promise<void>
  close(): Promise<void>
}
export function findChromium(): string | null
export function spawnBrowser(binary: string, port: number): Promise<{ stop(): Promise<void> }>
export function attach(port: number): Promise<Page>
export function launch(binary: string, port: number): Promise<Page>
