import { afterEach, describe, expect, it, vi } from 'vitest'

import { goBackOrToRoot } from './header-back'

function setup() {
  return { back: vi.fn(), push: vi.fn() }
}

describe('goBackOrToRoot', () => {
  afterEach(() => window.history.replaceState(null, ''))

  it('goes back when there is a previous in-app entry', () => {
    window.history.replaceState({ back: '/transactions' }, '')
    const router = setup()
    goBackOrToRoot(router, { name: 'transactions-calendar', query: {} })
    expect(router.back).toHaveBeenCalled()
    expect(router.push).not.toHaveBeenCalled()
  })

  it('a wallet deep link goes to the Capital list, not Home', () => {
    window.history.replaceState({ back: null }, '')
    const router = setup()
    goBackOrToRoot(router, { name: 'wallet', query: {} })
    expect(router.push).toHaveBeenCalledWith({ name: 'stores', query: {} })
    expect(router.back).not.toHaveBeenCalled()
  })

  it('keeps the money filter and drops the view', () => {
    window.history.replaceState(null, '')
    const router = setup()
    goBackOrToRoot(router, { name: 'money-where', query: { from: '2026-01-01', view: 'flow' } })
    expect(router.push).toHaveBeenCalledWith({ name: 'money', query: { from: '2026-01-01' } })
  })

  it('falls back to the dashboard when the route has no section', () => {
    window.history.replaceState(null, '')
    const router = setup()
    goBackOrToRoot(router, { name: 'settings', query: {} })
    expect(router.push).toHaveBeenCalledWith({ name: 'dashboard' })
  })
})
