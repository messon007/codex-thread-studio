# SQL and jq queries

Queries use the complete current table or JSON input, independently of the visible page. Results are read-only snapshots. A failed query retains your editor text and previous results so you can correct it and run again. Opening help or choosing an example never runs a query; example buttons only fill the editor. Ctrl+Enter or Command+Enter runs it.

## SQL: SQLite over the current table

Use `FROM t` with positional aliases `c1`, `c2`, and so on. Use `FROM data` with the names shown in **Columns and query help**. Both refer to the same current table; other document tables are unavailable. Blank names receive a generated name, duplicate names receive suffixes, and long names are bounded. The displayed mapping is authoritative.

Double quotes identify a column, while single quotes delimit text:

```sql
SELECT "Part Name" AS part FROM data LIMIT 20;
SELECT c1 FROM t WHERE c1 = 'O''Reilly' LIMIT 20;
```

A double quote inside a column name is doubled: `"quote""name"`. `_row` is the original 1-based record number, and `_line` is its 1-based source line. Include these explicitly if you need source coordinates in a result.

Every source cell has SQLite TEXT affinity, including numeric-looking values and empty strings. Numeric comparisons and sorting should convert the appropriate column explicitly:

```sql
SELECT c1, c2 FROM t
WHERE CAST(c2 AS REAL) > 10
ORDER BY CAST(c2 AS REAL) DESC
LIMIT 20;
```

Replace `c2` if the table has only one column. A conversion is not numeric validation: SQLite can convert nonnumeric text to zero or accept a numeric prefix. Empty text is `''`, not SQL `NULL`. Use `= ''` for empty text and `IS NULL` for nulls created by a query.

### Equality, membership and text search

These operations have different meanings:

```sql
-- Whole-cell equality
SELECT c1 FROM t WHERE c1 = 'DCDC-1297522';

-- Whole-cell equality against any member of a list
SELECT c1 FROM t WHERE c1 IN ('DCDC-1297522', 'other');

-- Literal, case-sensitive substring
SELECT c1 FROM t WHERE instr(c1, 'DCDC-1297522') > 0;

-- Wildcard substring: % matches any sequence, _ matches one character
SELECT c1 FROM t WHERE c1 LIKE '%DCDC-1297522%';

-- Match a literal percent sign using an explicit escape character
SELECT c1 FROM t WHERE c1 LIKE '%!%%' ESCAPE '!';
```

`'DCDC-1297522' IN c2` treats `c2` as a **table name** in SQLite. That explains `no such table: c2`; `IN` does not search inside text. Decide whether equality, list membership or substring search expresses your intent. The recovery button offered for this simple error fills an `instr` query for review; it does not execute it.

`LIKE` ignores ASCII case by default, but non-ASCII case remains significant. `instr` is case sensitive and treats `%` and `_` literally.

### Supported operations and limits

The engine is bundled SQLite through pinned `rusqlite 0.41.1`, restricted by MarkNoria's authorizer. Supported syntax includes `SELECT`, `WHERE`, `AND`/`OR`/`NOT`, comparisons, `IN`, `LIKE`, `CASE`, `CAST`, `DISTINCT`, `ORDER BY`, `LIMIT`/`OFFSET`, `GROUP BY` and `HAVING`:

```sql
SELECT c1, COUNT(*) AS count
FROM t GROUP BY c1 HAVING COUNT(*) > 1
ORDER BY count DESC LIMIT 20;

SELECT SUM(CAST(c1 AS REAL)) AS total FROM t;
```

The function allowlist is:

- Aggregation: `count`, `sum`, `total`, `avg`, `min`, `max`, `group_concat`, `string_agg`.
- Null/conditional: `coalesce`, `ifnull`, `nullif`, `iif`, `if`.
- Text and inspection: `like`, `glob`, `lower`, `upper`, `length`, `instr`, `substr`, `substring`, `replace`, `trim`, `ltrim`, `rtrim`, `typeof`, `unicode`, `hex`.
- Numeric: `round`, `abs`.
- Date/time: `date`, `time`, `datetime`, `strftime`, `julianday`, `unixepoch`.
- JSON: `json_extract`, `json_valid`, `json_array_length`.
- Windows: `row_number`, `rank`, `dense_rank`, `percent_rank`, `cume_dist`, `ntile`, `lag`, `lead`, `first_value`, `last_value`, `nth_value`.

Other SQLite functions can be denied even when SQLite normally supplies them. Writes, schema access, `PRAGMA`, `ATTACH`, extension loading and filesystem functions are unavailable. Enter one statement; there is no parameter-binding UI.

SQL input is limited to 16 KiB, source and result schemas to 256 columns, query execution to 5 seconds or 20 million SQLite instructions, and stored results to 32 MiB. Input preparation has a separate 15-second limit. Expression depth, SQLite memory, cell size and result transport have additional bounds. A result page contains at most 50 rows, 512 cells and 128 KiB. Selecting fewer columns or narrowing a query can avoid result limits; `LIMIT` does not guarantee an expensive sort or aggregation will finish within the execution budget.

Errors show SQLite's line and character column when the engine supplies an offset. Missing-column errors should be checked against the displayed mapping. Syntax errors should be checked around the reported location. Authorization errors can indicate a function outside the allowlist.

## jq: the embedded jaq dialect

MarkNoria embeds `jaq-core 3.1.0`, `jaq-std 3.0.1` and `jaq-json 2.0.1`. It is jq-compatible filter syntax, not a shell or the jq CLI. The standard and JSON definitions/functions are registered, except environment access; no I/O extension or module file loader is registered. Do not assume complete compatibility with another jq version.

Enter a filter directly, without a leading `jq`, shell quotes or CLI flags:

```jq
.
type
keys
.["field name"]
```

`type` works on any JSON value; `keys` expects an object or array. `.["field name"]` looks up an object key containing spaces or punctuation. Strings use double quotes.

### Input shape and slurp

With **Slurp off**, the filter runs separately for each top-level JSON value. A JSON array is one value. For an array of objects:

```jq
.[] | select(.active == true)
map({name: .name, value: .value})
```

JSONL/NDJSON typically has one object per line. Filter those records directly:

```jq
select(.level == "error")
```

The parser accepts a sequence of JSON values, not arbitrary raw text lines. Input values retain JSON types rather than becoming SQL text cells.

With **Slurp on**, all top-level input values become one array before evaluation. For JSONL records with numeric `value` fields:

```jq
map(.value | tonumber) | add
sort_by(.value)
length
```

Slurp is needed to sort or aggregate across separate records. Slurping a document that already contains one array creates a nested array containing that original array; it does not flatten the array. Slurp collects the complete input and is subject to the memory budget.

### Operations, restrictions and errors

Supported language constructs include pipes, comparisons, arithmetic, arrays/objects, `//` defaults, optional access `?`, `if/then/else/end` and local filter definitions. Useful registered filters include `select`, `map`, `keys`, `length`, `type`, `has`, `contains`, `sort_by`, `group_by`, `unique`, `add`, `tonumber`, `tostring`, `fromjson`, `tojson`, `test` and `capture`.

There are no `--arg` bindings, `input`/`inputs`, `env`/`$ENV`, module imports/includes, file reads, shell commands or network requests. Raw-input and raw-output flags are unavailable. Output strings remain quoted JSON; multiple outputs are displayed separately instead of implicitly collecting them into an array. Result pages may split a large value and are not independently guaranteed to be complete JSON documents.

The filter budget is 16 KiB; the source budget is 128 MiB. A supervised worker has a 5-second wall-time budget and 512 MiB RSS limit. Output is limited to four times source size, clamped between 8 and 256 MiB, with pages of at most 48 KiB.

Syntax and undefined-filter errors include a line and character column when jaq supplies a span in your filter. Runtime errors may have no source location. Check spelling and argument count for undefined filters. For type/iteration errors, inspect the input with `type` or `keys`, check whether you need `.[]`, and check Slurp. Error messages are displayed as inert text; query text and previous results remain available for retry.

Implementation references: [SQL core](../src/core/table_query.rs), [jq core](../src/core/jq_query.rs), [UI examples and explanations](../web/query-help.mjs). The [query help regression](../tests/server/query-help-check.mjs) executes the exact UI examples against these engines, including a one-column table, quoted/duplicate headers and JSON/JSONL slurp cases.
