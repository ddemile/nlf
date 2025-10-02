import type { BinaryExpression, CallExpression, EqualityExpression, Expression, LogicalExpression, RelationalExpression } from "./parser";

interface Statement {
    type: string;
    expression: Expression
}

const builtInFunctions = new Map<string, Function>();

builtInFunctions.set("println", (...args: string[]) => {
    console.log(...args)
})

builtInFunctions.set("pow", (a: number, b: number) => {
    return a ** b
})

builtInFunctions.set("expectToBeEqual", (a: number, b: number) => {
    if (a != b) {
        throw new Error(`Expected ${b}, got ${a}`)
    }
})

export function interpret(ast: any) {
    const variables = new Map();

    ast.body.forEach(evalStatement)

    function evalStatement(statement: Statement) {
        switch (statement.type) {
            case "expression":
                evalExpr(statement.expression)
                break;
            case "if":
                evalIf(statement)
                break;
            case "for":
                evalFor(statement)
                break;
            default:
                throw new Error("Unexpected statement " + statement.type)
        }
    }

    function evalFor(statement: any) {
        const left = evalExpr(statement.left)
        const right = evalExpr(statement.right)

        console.log(left, right)

        for (let index = left; index < right; index++) {
            statement.statements.forEach(evalStatement)
        }
    }

    function evalIf(statement: any) {
        if (evalExpr(statement.condition) == true) {
            statement.statements.forEach(evalStatement)
        }
    }

    function evalExpr(expr: Expression): any {
        switch (expr.type) {
            case "binary":
                return evalBinary(expr)
            case "literal":
                return expr.value
            case "assignment":
                if (expr.left.type != "variable") {
                    throw new Error("Expected variable for assignment " + JSON.stringify(expr))
                }
                return variables.set(expr.left.value, evalExpr(expr.right))
            case "variable":
                return variables.get(expr.value)
            case "call":
                return evalCall(expr)
            case "equality":
                return evalEquality(expr)
            case "relational":
                return evalRelational(expr)
            case "logical":
                return evalLogical(expr)
            default:
                throw new Error("Unexpected expression " + (expr as Expression).type)
        }
    }

    function evalBinary(expr: BinaryExpression): any {
        switch (expr.operator) {
            case "+":
                return evalExpr(expr.left) + evalExpr(expr.right)
            case "-":
                return evalExpr(expr.left) - evalExpr(expr.right)
            case "*":
                return evalExpr(expr.left) * evalExpr(expr.right)
            case "/":
                return evalExpr(expr.left) / evalExpr(expr.right)
        }
        throw new Error("Unknown operator " + expr.operator)
    }

    function evalCall(expr: CallExpression) {
        const func = builtInFunctions.get(expr.name)

        if (func) {
            return func(...expr.arguments.map(evalExpr))
        }

        throw new Error("Unknown function " + expr.name)
    }

    function evalEquality(expr: EqualityExpression): boolean {
        switch (expr.operator) {
            case "==":
                return evalExpr(expr.left) == evalExpr(expr.right) 
            case "!=":
                return evalExpr(expr.left) != evalExpr(expr.right) 
        }
    }

    function evalRelational(expr: RelationalExpression): boolean {
        switch (expr.operator) {
            case ">":
                return evalExpr(expr.left) > evalExpr(expr.right) 
            case ">=":
                return evalExpr(expr.left) >= evalExpr(expr.right)
            case "<":
                return evalExpr(expr.left) < evalExpr(expr.right) 
            case "<=":
                return evalExpr(expr.left) <= evalExpr(expr.right) 
        }
    }

    function evalLogical(expr: LogicalExpression): boolean {
        switch (expr.operator) {
            case "&&":
                return evalExpr(expr.left) && evalExpr(expr.right) 
            case "||":
                return evalExpr(expr.left) || evalExpr(expr.right)
        }
    }
}