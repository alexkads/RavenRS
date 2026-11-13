# ravenrs-app-postgrest

Records in PostgreSQL through PostgREST, as an `ObjectSpace`, for applications made with `ravenrs-app`: your own PostgREST, Supabase, or Neon's Data API. The station holds the token of the person signed in, never the database's password, and the database's grants and row level security decide what it sees.

```rust,no_run
use std::rc::Rc;
use ravenrs_app_postgrest::{PostgrestDatabase, ResourceMap};
# let order = ravenrs_app::EntityInfo::new("Order").field(ravenrs_app::FieldInfo::new("id", ravenrs_app::FieldKind::Integer).key());
let database = PostgrestDatabase::builder("https://project.supabase.co/rest/v1")
    .api_key("anon key")
    .token_source(Rc::new(|| Some("the access token".to_owned())))
    .resource(order, ResourceMap::new("orders"))?
    .build();
let mut space = database.space();
# Ok::<(), ravenrs_app::SpaceError>(())
```

Filters run on the server in PostgREST's words, and what those cannot say is filtered in memory. A commit is one call to `ravenrs_commit`, a function that applies every change in one transaction as the caller; install it once with `SERVER_SQL`. See the [application framework guide](../../docs/APP-FRAMEWORK.md).
