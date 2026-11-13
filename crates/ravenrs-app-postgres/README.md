# ravenrs-app-postgres

Records in PostgreSQL as an `ObjectSpace`, for applications made with `ravenrs-app`. Open it from the address a database's panel gives, Supabase, Neon or your own server:

```rust,no_run
use ravenrs_app_postgres::{PostgresBackend, PostgresDatabase, TableMap};
# let order = ravenrs_app::EntityInfo::new("Order");
let backend = PostgresBackend::connect("postgresql://app:secret@db.example.com/app?sslmode=require")?;
let database = PostgresDatabase::builder(backend).managed(order, TableMap::new("orders"))?.build();
# Ok::<(), Box<dyn std::error::Error>>(())
```

TLS goes through rustls and always checks the certificate; `sslrootcert=<file>` adds an authority, as Supabase's direct connection needs. Behind a pooler in transaction mode (Supavisor on 6543, Neon's `-pooler` hosts, `pgbouncer=true`), statements go unprepared. A connection that dropped outside a transaction is opened again; inside one, the commit fails whole. A `postgres::Client` you opened yourself works too, with `PostgresDatabase::builder(client)`.

Numbered keys come from the highest key in the table, so only one process should create records on a table. See the [application framework guide](../../docs/APP-FRAMEWORK.md).
