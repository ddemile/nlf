import { expect, test } from 'vitest'
import { evalCode } from '.'

test('expect pow(2, 3) to be 8', () => {
  expect(() => evalCode("expectToBeEqual(pow(2, 3), 8)")).not.toThrow()
})

test('expect expectToBeEqual to throw', () => {
  expect(() => evalCode("expectToBeEqual(1, 2)")).toThrow()
})