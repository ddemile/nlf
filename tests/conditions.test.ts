import { expect, test } from 'vitest'
import { evalCode } from '.'

test('expect if true block to be executed', () => {
  expect(() => evalCode("if true { a = 1 }\n expectToBeEqual(a, 1)")).not.toThrow()
})

test('expect if a > 2 block to not executed', () => {
  expect(() => evalCode("a = 3\nif a < 2 { a = 1 }\nexpectToBeEqual(a, 3)")).not.toThrow()
})