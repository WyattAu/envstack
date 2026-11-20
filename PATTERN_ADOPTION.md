# PATTERN_ADOPTION

Envstack's declared relationship to the WyattAu pattern canon
(`pattern-standards`, references/catalog.md). Machine-checked by
`pattern-standards/scripts/adoption-check.py`.

| Pattern | Status | Evidence |
|---|---|---|
| T1 | adopted | src/lib.rs:324 (`extract<T: DeserializeOwned>` turns the merged layers into one typed config at the boundary; interior code sees constructed values only) |
| P47 | n/a | single-shot load: `ConfigStack` merges once inside extract, no live-reload path exists (src/lib.rs:324), so torn config reads cannot occur |
