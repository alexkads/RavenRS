# ravenrs-app-postgres

Records in PostgreSQL as an `ObjectSpace`, for applications made with `ravenrs-app`. You make the connection, so TLS and credentials are yours to choose.

```rust,no_run
use ravenrs_app_postgres::{PostgresDatabase, TableMap};
# let order = ravenrs_app::EntityInfo::new("Order");
let client = postgres::Client::connect("host=localhost user=app", postgres::NoTls)?;
let database = PostgresDatabase::builder(client).managed(order, TableMap::new("orders"))?.build();
# Ok::<(), Box<dyn std::error::Error>>(())
```

Numbered keys come from the highest key in the table, so only one process should create records on a table. See the [application framework guide](../../docs/APP-FRAMEWORK.md).
