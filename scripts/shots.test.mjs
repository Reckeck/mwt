import assert from 'node:assert/strict'
import { test } from 'node:test'

import { THEME_IDS, compareTheme, resolveThemes } from './shots/steps.mjs'

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

const dracula = { mode: 'dark', background: '#282A36', moneyIncome: '#50FA7B' }

test('compareTheme: a match, in any case and spacing, is null', () => {
  assert.equal(compareTheme(dracula, { mode: 'dark', background: ' #282a36', moneyIncome: '#50fa7b' }), null)
})

test('compareTheme: a colour mismatch names the token, expected and actual', () => {
  const message = compareTheme(dracula, { ...dracula, background: '#ffffff' })
  assert.match(message, /--background expected #282A36, got #ffffff/)
  assert.doesNotMatch(message, /money-income/)
})

test('compareTheme: a mode mismatch names both modes', () => {
  assert.match(compareTheme(dracula, { ...dracula, mode: 'light' }), /data-theme expected dark, got light/)
})
