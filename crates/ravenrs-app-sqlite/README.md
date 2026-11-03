# ravenrs-app-sqlite

Keeps the records of a [`ravenrs-app`](https://docs.rs/ravenrs-app) application in SQLite. It creates its tables, or maps onto tables an application already owns without touching their schema, and shares that application's connection. Filters run in the database as bound parameters whenever they can.
