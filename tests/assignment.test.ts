import { expect, test } from 'vitest'
import { evalCode } from '.'

test('assign a to 1 + 1', () => {
  expect(() => evalCode("a = 1 + 1\n expectToBeEqual(a, 2)")).not.toThrow()
})

test('assign a to a + 1', () => {
  expect(() => evalCode("a = 1\n a = a + 1 \n expectToBeEqual(a, 2)")).not.toThrow()
})