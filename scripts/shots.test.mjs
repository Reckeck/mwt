import assert from 'node:assert/strict'
import { test } from 'node:test'

import { THEME_IDS, resolveThemes } from './shots/steps.mjs'

test('no value renders light and dark', () => {
  assert.deepEqual(resolveThemes(null), ['light', 'dark'])
})

test('all renders every built-in theme', () => {
  assert.equal(resolveThemes('all').length, 30)
  assert.ok(THEME_IDS.includes('dracula'))
})

test('light, dark and ids are accepted', () => {
  assert.deepEqual(resolveThemes('light'), ['light'])
  assert.deepEqual(resolveThemes('dracula,dark'), ['dracula', 'dark'])
})

test('an unknown id lists the valid ones', () => {
  assert.throws(() => resolveThemes('nope'), /unknown theme 'nope'.*dracula/)
})
