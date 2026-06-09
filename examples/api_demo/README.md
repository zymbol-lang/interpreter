# Public API demo

Small, runnable examples that consume real public APIs using `std/net` (HTTP
client) and `std/json`. They double as a live integration smoke test for those
modules and as documentation of the idioms.

> **Requires network access.** All endpoints are free and need no authentication.

Run any script with the tree-walker (default) or the register VM (`--vm`); output
is identical on both engines:

```bash
zymbol run examples/api_demo/01_get_fact.zy
zymbol run --vm examples/api_demo/03_list_posts.zy
```

## Scripts

| Script | API | Shows |
|--------|-----|-------|
| `01_get_fact.zy` | catfact.ninja | simplest GET → `json::decode` → read a field |
| `02_get_user.zy` | api.github.com | parse several fields of a JSON object |
| `03_list_posts.zy` | jsonplaceholder.typicode.com | decode a JSON array, iterate with `@`, slice `[1..5]` |
| `04_post_json.zy` | httpbin.org | `json::encode` → `net::post_json` → decode the echo (roundtrip) |
| `05_robusto_es.zy` | catfact.ninja | graceful failure + Spanish API names via i18n re-export |
| `apis_es.zy` | — | i18n adapter: re-exports `net`/`json` as `obtener`/`enviar_json`/`decodificar`/`codificar` |

## Error handling

`std/net` returns network failures as a **soft `Error` value**, not a thrown
error — the call still returns normally. Test the result with the is-error
operator `$!` instead of try-catch:

```zymbol
resp = net::get(url)
? resp$! {
    >> "request failed" ¶
} _ {
    >> resp ¶
}
```

A wrong argument *type* (e.g. `net::get(42)`) is a programmer error and aborts
hard, as with any stdlib function.

## Notes

- A JSON object decodes to a named tuple — access fields with `.` (`u.login`),
  nested too (`resp.json.user`).
- JSON arrays decode to arrays — `posts$#` is the length, `posts[1]` is 1-based.
- To fail fast in a demo, use an invalid URL (`"no-es-una-url"`); an unreachable
  real host can block until the HTTP timeout.
