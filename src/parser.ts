import type { Token } from "./lexer";

export type Expression = LiteralExpression | BinaryExpression | AssignmentExpression | CallExpression | EqualityExpression | RelationalExpression | LogicalExpression

type BinaryOperator = "+" | "-" | "/" | "*"
type EqualityOperator = "==" | "!="
type RelationalOperator = ">" | ">=" | "<" | "<="
type LogicalOperator = "||" | "&&"

export interface LiteralExpression {
    type: "literal" | "variable";
    value: any
}

export interface BinaryExpression {
    type: "binary";
    left: Expression;
    operator: BinaryOperator
    right: Expression
}

export interface AssignmentExpression {
    type: "assignment";
    left: LiteralExpression;
    right: Expression
}

export interface CallExpression {
    type: "call";
    name: string;
    arguments: Expression[]
}

export interface EqualityExpression {
    type: "equality";
    left: Expression;
    operator: EqualityOperator
    right: Expression
}

export interface RelationalExpression {
    type: "relational";
    left: Expression;
    operator: RelationalOperator
    right: Expression
}


export interface LogicalExpression {
    type: "logical";
    left: Expression;
    operator: LogicalOperator
    right: Expression
}

function parse_internal(tokens: Token[]): [any[], number] {
    let cursor = 0;
    const statements = []

    while (cursor < tokens.length) {
        const token = tokens[cursor]
        
        switch (token.type) {
            case "for":
                cursor++;
                let left: LiteralExpression = { type: "literal", value: 0 }
                if (["number", "identifier"].includes(tokens[cursor].type)) {
                    left = literalExpression() as LiteralExpression
                }
                if (tokens[cursor++]?.type != "range") {
                    throw new Error("Expected range operator (..)")
                }
                let right: LiteralExpression = { type: "literal", value: 0 }
                if (["number", "identifier"].includes(tokens[cursor].type)) {
                    right = literalExpression() as LiteralExpression
                }                

                let forStatements = block()

                statements.push({ type: "for", left, right, statements: forStatements })
                break;
            case "if":
                cursor++;
                const condition = callExpression()

                let innerStatements = block()

                statements.push({ type: "if", condition, statements: innerStatements })
                break;
            case "}":
                return [statements, cursor]
            default:
                statements.push({ type: "expression", expression: expression() })
        }
    }

    return [statements, cursor]

    function block() {
        if (tokens[cursor++]?.type != "{") {
            throw new Error("Expected {")
        }
        let [innerStatements, innerCursor] = parse_internal(tokens.slice(cursor))
        cursor += innerCursor;
        if (tokens[cursor]?.type != "}") {
            throw new Error("Expected }")
        }
        
        cursor++;

        return innerStatements
    }

    function expression(): Expression {
        return assignmentExpression()
    }

    function assignmentExpression(): Expression {
        let left = callExpression() as LiteralExpression
        if (tokens[cursor]?.type == "=") {
            cursor++;
            return {
                type: "assignment",
                left,
                right: callExpression()
            }
        }
        return left;
    }

    function callExpression(): Expression {
        const token = tokens[cursor];

        if (token.type == "identifier" && tokens[cursor + 1].type == "(") {
            cursor ++;

            const args: Expression[] = []

            do {
                cursor++;

                if (tokens[cursor]?.type == ")") {
                    throw new Error("Unexpected ,")
                }

                const expr = callExpression()

                args.push(expr)
            } while (tokens[cursor]?.type == ",")

            if (tokens[cursor]?.type != ")") {
                throw new Error("Expected )")
            }

            cursor++;

            return {
                type: "call",
                name: token.value,
                arguments: args
            }
        }

        return equalityExpression()
    }

    function equalityExpression(): Expression {
        let left = logicalExpression();

        while (["==", "!="].includes(tokens[cursor]?.type)) {
            left = {
                type: "equality",
                left,
                operator: tokens[cursor++].type as EqualityOperator,
                right: logicalExpression()
            }
        }

        return left;
    }

    function logicalExpression(): Expression {
        let left = relationalExpression();

        while (["&&", "||"].includes(tokens[cursor]?.type)) {
            left = {
                type: "logical",
                left,
                operator: tokens[cursor++].type as LogicalOperator,
                right: relationalExpression()
            }
        }

        return left;
    }

    function relationalExpression(): Expression {
        let left = termExpression();

        while ([">", ">=", "<", "<="].includes(tokens[cursor]?.type)) {
            left = {
                type: "relational",
                left,
                operator: tokens[cursor++].type as RelationalOperator,
                right: termExpression()
            }
        }

        return left;
    }

    function termExpression() {
        let left = factorExpression()
        while (["+", "-"].includes(tokens[cursor]?.type)) {
            left = {
                type: "binary",
                left,
                operator: tokens[cursor++].type as BinaryOperator,
                right: factorExpression()
            }
        }
        return left;
    }

    function factorExpression() {
        let left = literalExpression()
        while (["*", "/"].includes(tokens[cursor]?.type)) {
            left = {
                type: "binary",
                left, 
                operator: tokens[cursor++].type as BinaryOperator,
                right: literalExpression()
            }
        }
        return left;
    }

    function literalExpression(): Expression {
        const token = tokens[cursor++];

        if (token.type == "number" ) {
            return {
                type: "literal",
                value: token.value
            }
        }
        if (token.type == "boolean") {
            return {
                type: "literal",
                value: token.value == "true" ? true : false
            }
        }
        if (token.type == "identifier") {
            return {
                type: "variable",
                value: token.value
            }
        }
        if (token.type == "(") {
            const expr = expression()
            if (tokens[cursor]?.type != ")") {
                throw new Error(") expected")
            }
            cursor++;
            return expr;
        }
        throw new Error("Unexpected token " + JSON.stringify(token))
    }
}

export function parse(tokens: Token[]) {
    return {
        type: "program",
        body: parse_internal(tokens)[0]
    }
}