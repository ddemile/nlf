import { interpret } from "./interpreter";
import { lex } from "./lexer"
import { parse } from "./parser"

const code = await Bun.file("src/program.nlf").text()

const tokens = lex(code);

await Bun.file("tokens.json").write(JSON.stringify(tokens, null, 2))

const ast = parse(tokens)

console.log(ast)

await Bun.file("ast.json").write(JSON.stringify(ast, null, 2))

interpret(ast)