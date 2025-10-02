export type Token = { type: string, value?: any }

export function lex(code: string): Token[] {
    let cursor = 0;

    const tokens: Token[] = []

    function number() {
        const start = cursor - 1;

        while (isDigit(code[cursor])) {
            cursor++;
        }

        if (code[cursor] === "." && isDigit(code[cursor + 1])) {
            cursor++;
            while (isDigit(code[cursor])) {
                cursor++;
            }
        }

        tokens.push({ type: "number", value: parseFloat(code.slice(start, cursor)) })
    }

    function alpha() {
        const start = cursor - 1;

        while (isAlpha(code[cursor]) || isDigit(code[cursor])) {
            cursor++;
        }

        tokens.push({ type: "identifier", value: code.slice(start, cursor) })
    }

    function checkLiteral(operator: string, type?: string) {        
        if (code.startsWith(operator, cursor - 1)) {
            tokens.push({ type: type ?? operator, value: type ? operator : null })
            cursor += operator.length - 1
            return true;
        }

        return false;
    }

    function is(string: string) {        
        if (code.startsWith(string, cursor - 1)) {
            cursor += string.length - 1
            return true;
        }

        return false;
    }

    while (cursor < code.length) {
        const char = code[cursor++];
        switch (char) {
            case " ":
            case "\0":
            case "\n":
            case "\r":
            case "\t":
                break;
            case "=":
                if (code[cursor++] == "=") {
                    tokens.push({ type: "==" })
                } else {
                    tokens.push({ type: char })
                }
                break;
            case "+":
            case "-":
            case "*":
            case "/":
            case "(":
            case ")":
            case "{":
            case "}":
            case ",":
                tokens.push({ type: char })
                break;
            default:
                if (checkLiteral("true", "boolean")) break;
                if (checkLiteral("false", "boolean")) break;
                if (is("if")) {
                    tokens.push({ type: "if" })
                    break;
                }
                if (is("for")) {
                    tokens.push({ type: "for" })
                    break;
                }
                if (is("..")) {
                    tokens.push({ type: "range" })
                    break;
                }
                if (isDigit(char)) {
                    number()
                    break;
                }
                if (isAlpha(char)) {
                    alpha()
                    break;
                }
                if (checkLiteral("!=")) break;
                if (checkLiteral("&&")) break;
                if (checkLiteral("||")) break;
                if (checkLiteral(">=")) break;
                if (checkLiteral(">")) break;
                if (checkLiteral("<=")) break;
                if (checkLiteral("<")) break;
                throw new Error(`Unexpected token ${char}`)

        }
    }

    return tokens;
} 

function isDigit(char: string): boolean {
    return char >= "0" && char <= "9"
}

function isAlpha(char: string) {
    return char >= "a" && char <= "z" || char >= "A" && char <= "Z" || char == "_"
}