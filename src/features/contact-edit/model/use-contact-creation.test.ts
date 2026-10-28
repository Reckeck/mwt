import { nextTick } from 'vue'
import { describe, expect, it } from 'vitest'

import { useContactCreation } from './use-contact-creation'

const petya = { id: 'c-petya', name: 'Петя' } as Contact

describe('useContactCreation', () => {
  it('opens the form with the typed name and resolves with the saved contact', async () => {
    const creation = useContactCreation()
    const made = creation.create('Петя')
    expect(creation.open.value).toBe(true)
    expect(creation.name.value).toBe('Петя')

    creation.saved(petya)
    creation.open.value = false
    await nextTick()
    await expect(made).resolves.toBe(petya)
  })

  it('resolves with null when the form is closed unsaved', async () => {
    const creation = useContactCreation()
    const made = creation.create('Петя')
    await nextTick()
    creation.open.value = false
    await nextTick()
    await expect(made).resolves.toBeNull()
  })
})
