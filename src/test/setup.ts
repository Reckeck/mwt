import { config } from '@vue/test-utils'
import { vi } from 'vitest'

// Node's own (file-less) `localStorage` shadows happy-dom's and is undefined; the i18n
// module reads it on import, so an in-memory one goes in first.
function memoryStorage(): Storage {
  const data = new Map<string, string>()
  return {
    get length() {
      return data.size
    },
    clear: () => data.clear(),
    getItem: (key) => data.get(key) ?? null,
    key: (index) => [...data.keys()][index] ?? null,
    removeItem: (key) => void data.delete(key),
    setItem: (key, value) => void data.set(key, String(value)),
  }
}
vi.stubGlobal('localStorage', memoryStorage())
vi.stubGlobal('sessionStorage', memoryStorage())

// No test reaches Tauri or the in-memory mock backend: `call` rejects until a test
// gives it an answer (`vi.mocked(call).mockResolvedValueOnce(...)`), `onEvent`
// subscribes to nothing.
vi.mock('@/shared/api/ipc', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/shared/api/ipc')>()),
  isTauri: false,
  call: vi.fn(async (cmd: string) => {
    throw new Error(`unmocked ipc call: ${cmd}`)
  }),
  onEvent: vi.fn(async () => () => undefined),
  onBackButton: vi.fn(async () => () => undefined),
}))

// The real ru locale, so components render the same strings as in the app.
const { i18n } = await import('@/shared/config/i18n')
i18n.global.locale.value = 'ru'
config.global.plugins = [i18n]
