import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import ProgressBar from './ProgressBar.vue'

describe('ProgressBar marker', () => {
  it('draws no marker by default', () => {
    expect(
      mount(ProgressBar, { props: { value: 40 } })
        .find('[data-progress-marker]')
        .exists(),
    ).toBe(false)
  })

  it('puts the limit tick at its percent, clamped', () => {
    const at = (marker: number) =>
      mount(ProgressBar, { props: { value: 40, marker } })
        .find('[data-progress-marker]')
        .attributes('style')
    expect(at(80)).toContain('left: 80%')
    expect(at(150)).toContain('left: 100%')
  })
})
