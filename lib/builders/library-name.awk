# Read only lib.name from a Cargo manifest. Tokenization keeps strings, comments,
# dotted keys, and inline tables separate; unrelated TOML values are skipped.
function fail(message) { print "Cargo library name: " message > "/dev/stderr"; exit 1 }
function add(value, type) { token[++count] = value; kind[count] = type }
function string_value(quote,    triple, value, c, escape, digits, width) {
    triple = substr(input, offset, 3) == quote quote quote
    offset += triple ? 3 : 1
    if (triple && substr(input, offset, 1) == "\n") offset++
    value = ""
    while (offset <= length(input)) {
        if (triple && substr(input, offset, 3) == quote quote quote) { offset += 3; return value }
        c = substr(input, offset++, 1)
        if (!triple && c == quote) return value
        if (quote == "\"" && c == "\\") {
            escape = substr(input, offset++, 1)
            if (triple && escape ~ /[ \t\r\n]/) {
                while (offset <= length(input) && substr(input, offset, 1) ~ /[ \t\r\n]/) offset++
            } else if (escape == "u" || escape == "U") {
                width = escape == "u" ? 4 : 8
                digits = substr(input, offset, width)
                offset += width
                value = value sprintf("%c", strtonum("0x" digits))
            } else if (escape == "n") value = value "\n"
            else if (escape == "t") value = value "\t"
            else if (escape == "r") value = value "\r"
            else value = value escape
        } else value = value c
    }
    fail("unterminated string")
}
function tokenize(    c, start) {
    offset = 1
    while (offset <= length(input)) {
        c = substr(input, offset, 1)
        if (c == "#") { while (offset <= length(input) && substr(input, offset, 1) != "\n") offset++ }
        else if (c == "\n") { add(c, "newline"); offset++ }
        else if (c ~ /[ \t\r]/) offset++
        else if (c == "\"" || c == "'") add(string_value(c), "string")
        else if (index("[]=,.{}", c)) { add(c, "punctuation"); offset++ }
        else {
            start = offset
            while (offset <= length(input) && substr(input, offset, 1) !~ /[ \t\r\n#\[\]=,.{}"']/) offset++
            add(substr(input, start, offset - start), "bare")
        }
    }
}
function key_path(    path) {
    path = token[position++]
    while (token[position] == ".") { position++; path = path "/" token[position++] }
    return path
}
function skip_newlines() { while (kind[position] == "newline") position++ }
function value(path,    key, text, type) {
    if (token[position] == "{") {
        position++
        skip_newlines()
        while (token[position] != "}" && position <= count) {
            key = key_path()
            if (token[position++] != "=") fail("invalid inline table")
            value(path "/" key)
            skip_newlines()
            if (token[position] == ",") position++
            skip_newlines()
        }
        position++
    } else if (token[position] == "[") {
        position++
        skip_newlines()
        while (token[position] != "]" && position <= count) {
            value("")
            if (token[position] == ",") position++
            skip_newlines()
        }
        position++
    } else {
        text = token[position]
        type = kind[position++]
        if (path == "lib/name") {
            if (type != "string" || text == "" || text ~ /[\r\n]/) fail("lib.name must be a nonempty string")
            result = text
        }
        # Dates, floats, and other unrelated bare values can span several tokens.
        while (position <= count && kind[position] != "newline" && token[position] != "," && token[position] != "]" && token[position] != "}") position++
    }
}
{ input = input $0 "\n" }
END {
    tokenize()
    position = 1
    section = ""
    while (position <= count) {
        skip_newlines()
        if (position > count) break
        if (token[position] == "[") {
            position++
            array_table = token[position] == "["
            if (array_table) position++
            section = key_path()
            if (token[position++] != "]") fail("invalid table header")
            if (array_table) { position++; section = "@array/" section }
        } else {
            key = key_path()
            if (token[position++] != "=") fail("invalid assignment")
            value(section == "" ? key : section "/" key)
        }
    }
    print result == "" ? defaultName : result
}
