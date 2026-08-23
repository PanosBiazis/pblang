# SQL Schema Lab

This pure-PBLang example consolidates the SQL exercises from the earlier
`SQLscripts` folder into one disposable, tested SQLite workflow. It does not
connect to an existing database, preserve personal sample values, or execute
caller-supplied SQL.

## What it demonstrates

- versioned and idempotent schema migrations;
- primary, foreign, unique, check, not-null, and default constraints;
- an index and a relational view;
- parameterized inserts and updates;
- `WHERE ... AND ...`, ascending and descending ordering;
- transaction rollback on failure;
- schema inspection through `sqlite_master`;
- bounded validation before database effects.

The database is SQLite `:memory:` and disappears when the program closes. This
makes the example safe to run repeatedly without creating or changing local
database files.

## Run

```sh
./target/release/pblang run examples/sql_schema_lab/main.sapp
./target/release/pblang run examples/sql_schema_lab/main.sapp -- delta 91 advanced
```

## Source mapping and corrections

The earlier `createDB`, `createTable`, `createTest2Table`, `makeprimary`, and
`makeautoincr` exercises map to the four versioned migrations and SQLite's
`INTEGER PRIMARY KEY AUTOINCREMENT`. `InsertData` and `Updatetable` map to
parameterized record operations. `VIEWS` maps to `record_summary`, while
`showDB`, `showSpDB`, and `showTable` map to connection and schema reports.

MySQL-only `USE`, `ENGINE`, and `MODIFY COLUMN` statements are not copied into
SQLite. The invalid bitwise `&` predicates are corrected to logical `AND`, and
the incomplete `WHERE name` query is omitted. The long notes file remains
source material rather than executable SQL because it mixes dialects and
contains claims that are not portable across database engines.

This is an educational local example, not a migration engine for arbitrary SQL
dialects or a production database administration tool.
