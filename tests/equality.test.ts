import { expect, test } from 'vitest'
import { evalCode } from '.'

test('expect 10 != 5 to be true', () => {
  expect(() => evalCode("expectToBeEqual(10 != 5, true)")).not.toThrow()
})

test('expect 5 == 4 + 1 to be true', () => {
  expect(() => evalCode("expectToBeEqual(5 == 4 + 1, true)")).not.toThrow()
})

test('expect true == false to be false', () => {
  expect(() => evalCode("expectToBeEqual(true == false, false)")).not.toThrow()
})