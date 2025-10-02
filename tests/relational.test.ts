import { expect, test } from 'vitest'
import { evalCode } from '.'

test('expect 2 > 2 to be false', () => {
  expect(() => evalCode("expectToBeEqual(2 > 2, false)")).not.toThrow()
})

test('expect 20 >= 20 to be true', () => {
  expect(() => evalCode("expectToBeEqual(20 >= 20, true)")).not.toThrow()
})

test('expect 3 < 4 to be true', () => {
  expect(() => evalCode("expectToBeEqual(3 < 4, true)")).not.toThrow()
})

test('expect 3 =< 2 to be false', () => {
  expect(() => evalCode("expectToBeEqual(3 <= 2, false)")).not.toThrow()
})