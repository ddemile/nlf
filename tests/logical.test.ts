import { expect, test } from 'vitest'
import { evalCode } from '.'

test('expect false || true to be true', () => {
  expect(() => evalCode("expectToBeEqual(false || true, true)")).not.toThrow()
})

test('expect false || false to be false', () => {
  expect(() => evalCode("expectToBeEqual(false || false, false)")).not.toThrow()
})

test('expect false && true to be false', () => {
  expect(() => evalCode("expectToBeEqual(false && true, false)")).not.toThrow()
})

test('expect true && true to be false', () => {
  expect(() => evalCode("expectToBeEqual(true && false, false)")).not.toThrow()
})