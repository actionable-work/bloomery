{lib}: let
  tokensFor = expression:
    lib.filter (token: token != "")
    (lib.splitString " "
      (builtins.replaceStrings
        ["\n" "\r" "\t" "(" ")"]
        [" " " " " " " ( " " ) "]
        expression));

  validIdentifier = value:
    builtins.match "[A-Za-z0-9][A-Za-z0-9.-]*(:[A-Za-z0-9][A-Za-z0-9.-]*)?" value != null;

  parse = expression:
    if !builtins.isString expression || expression == ""
    then null
    else let
      tokens = tokensFor expression;

      parsePrimary = remaining:
        if remaining == []
        then null
        else let
          token = builtins.head remaining;
          rest = builtins.tail remaining;
        in
          if token == "("
          then let
            nested = parseOr rest;
          in
            if nested == null || nested.rest == [] || builtins.head nested.rest != ")"
            then null
            else {
              ast = nested.ast;
              rest = builtins.tail nested.rest;
            }
          else if token == ")" || token == "AND" || token == "OR" || token == "WITH" || !(validIdentifier token)
          then null
          else {
            ast = {
              type = "id";
              value = token;
            };
            rest = rest;
          };

      parseWith = remaining: let
        primary = parsePrimary remaining;
      in
        if primary == null
        then null
        else if primary.rest != [] && builtins.head primary.rest == "WITH"
        then let
          afterWith = builtins.tail primary.rest;
        in
          if primary.ast.type != "id" || afterWith == [] || !(validIdentifier (builtins.head afterWith))
          then null
          else {
            ast = {
              type = "with";
              license = primary.ast.value;
              exception = builtins.head afterWith;
            };
            rest = builtins.tail afterWith;
          }
        else primary;

      parseChain = operator: parseOperand: left: remaining:
        if remaining != [] && builtins.head remaining == operator
        then let
          right = parseOperand (builtins.tail remaining);
        in
          if right == null
          then null
          else
            parseChain operator parseOperand {
              type = operator;
              left = left;
              right = right.ast;
            }
            right.rest
        else {
          ast = left;
          rest = remaining;
        };

      parseAnd = remaining: let
        first = parseWith remaining;
      in
        if first == null
        then null
        else parseChain "AND" parseWith first.ast first.rest;

      parseOr = remaining: let
        first = parseAnd remaining;
      in
        if first == null
        then null
        else parseChain "OR" parseAnd first.ast first.rest;

      parsed = parseOr tokens;
    in
      if parsed != null && parsed.rest == []
      then parsed.ast
      else null;

  render = ast:
    if ast.type == "id"
    then ast.value
    else if ast.type == "with"
    then "${ast.license} WITH ${ast.exception}"
    else "(${render ast.left} ${ast.type} ${render ast.right})";

  compatibilityEntry = expression: let
    ast = parse expression;
  in
    if ast == null || !(ast.type == "id" || ast.type == "with")
    then null
    else render ast;

  evaluateNode = {
    ast,
    allow,
    deny,
  }:
    if ast.type == "AND"
    then
      evaluateNode {
        inherit allow deny;
        ast = ast.left;
      }
      && evaluateNode {
        inherit allow deny;
        ast = ast.right;
      }
    else if ast.type == "OR"
    then
      evaluateNode {
        inherit allow deny;
        ast = ast.left;
      }
      || evaluateNode {
        inherit allow deny;
        ast = ast.right;
      }
    else let
      key =
        if ast.type == "id"
        then ast.value
        else render ast;
    in
      (allow == [] || builtins.elem key allow) && !(builtins.elem key deny);
in {
  inherit parse compatibilityEntry;
  validExpression = expression: parse expression != null;
  evaluate = {
    expression,
    allow ? [],
    deny ? [],
  }: let
    ast = parse expression;
  in
    ast != null && evaluateNode {inherit ast allow deny;};
}
