import { interpret } from "../src/interpreter";
import { lex } from "../src/lexer";
import { parse } from "../src/parser";

export function evalCode(code: string): any {
    return interpret(parse(lex(code)))
}