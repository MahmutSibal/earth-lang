"""Generates the VS Code grammar from the spellings in src/keywords.rs."""
import json, re, sys
src = open(sys.argv[1], encoding="utf-8").read()
def table(name):
    body = src[src.index(f"const {name}"):]
    body = body[:body.index("];")]
    groups = {}
    for meaning, cols in re.findall(r"\((\w+::\w+), \[(.*?)\]\),", body, re.S):
        groups[meaning.split("::")[1]] = re.findall(r'"([^"]+)"', cols)
    return groups
kw = table("KEYWORDS"); bi = table("BUILTINS")
def alt(words):
    return "(?<![\\p{L}\\p{N}_])(?:" + "|".join(sorted(words, key=len, reverse=True)) + ")(?![\\p{L}\\p{N}_])"
constants = kw.pop("True") + kw.pop("False") + kw.pop("Nil")
operators = kw.pop("And") + kw.pop("Or") + kw.pop("Not")
function_kw = kw.pop("Function")
control = [w for ws in kw.values() for w in ws]
builtins = [w for ws in bi.values() for w in ws]
grammar = {
  "$schema": "https://raw.githubusercontent.com/martinring/tmlanguage/master/tmlanguage.json",
  "name": "Earth", "scopeName": "source.earth",
  "patterns": [
    {"name": "comment.line.number-sign.earth", "match": "#.*$"},
    {"name": "string.quoted.double.earth", "begin": "\"", "end": "\"",
     "patterns": [{"name": "constant.character.escape.earth", "match": "\\\\[\"\\\\nrt]"}]},
    {"match": alt(function_kw) + "\\s+([\\p{L}_][\\p{L}\\p{N}_]*)",
     "captures": {"0": {"name": "storage.type.function.earth"}, "1": {"name": "entity.name.function.earth"}}},
    {"name": "keyword.control.earth", "match": alt(control)},
    {"name": "constant.language.earth", "match": alt(constants)},
    {"name": "keyword.operator.word.earth", "match": alt(operators)},
    {"name": "support.function.builtin.earth", "match": alt(builtins)},
    {"name": "constant.numeric.earth", "match": "(?<![\\p{L}\\p{N}_])\\d+(?:\\.\\d+)?"},
    {"name": "keyword.operator.earth", "match": "\\*\\*=|\\*\\*|&&|\\|\\||==|!=|<=|>=|\\+=|-=|\\*=|/=|%=|[-+*/%<>=]"},
    {"match": "([\\p{L}_][\\p{L}\\p{N}_]*)\\s*(?=\\()", "captures": {"1": {"name": "entity.name.function.call.earth"}}}
  ]
}
json.dump(grammar, open(sys.argv[2], "w", encoding="utf-8"), ensure_ascii=False, indent=2)
print(len(control), len(constants), len(operators), len(builtins))
