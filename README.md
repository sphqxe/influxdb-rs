# influxdb

This crate is a fork of [this] crate, updated to use `futures` v0.3, `reqwest` v0.10, `tokio` v0.2, `syn`/`quote` v1.0 for `influxdb-derive`, and utilize `async`/`await` syntax which was stabilized in Rust 1.39. All credits go to [Jake Goulding] and [Carol Nichols] for the original concept and code base.


influxdb provides an asynchronous Rust interface to an [InfluxDB][] database.


This crate supports insertion of strings already in the InfluxDB Line Protocol.
The `influxdb-derive` crate provides convenient serialization of Rust structs
to this format.

At the moment, only data inserts are supported. Queries and deserialization to a Rust struct will be added at a later date.

[InfluxDB]: https://www.influxdata.com/
[this]: https://github.com/panoptix-za/influxdb-rs
[Carol Nichols]: https://github.com/carols10cents
[Jake Goulding]: https://github.com/shepmaster

## Examples

To serialize a struct into the InfluxDB Line Protocol format, use the
`influxdb-derive` crate's macros as shown below with `MyMeasure`.

Then create an instance of `influxdb::AsyncDb` and add instances of your
struct. Check out the code in the `examples` directory to see how this code
interacts with futures.

```rust
use influxdb;
use influxdb_derive;
use chrono::Utc;

use std::time::SystemTime;
use influxdb::{Measurement, AsyncDb};

// `Measurement` is the trait that `AsyncDb` needs in order to insert
#[derive(Measurement)]
// The default measurement name will be the struct name; this optional
// annotation allows customization of the name sent to InfluxDB.
#[influx(rename = "my_measure")]
struct MyMeasure {
    // Specify which struct fields are InfluxDB tags.
    // Tags must be `String`s or `&str`s.
    #[influx(tag)]
    region: String,
    // Specify which struct fields are InfluxDB fields.
    // Supported types are integers, floats, strings, and booleans.
    // The rename annotation works with struct fields as well.
    #[influx(field, rename = "amount")]
    count: i32,
    // Specify which struct field is the InfluxDB timestamp.
    #[influx(timestamp)]
    when: i64,
    // Struct fields that aren't annotated won't be sent to InfluxDB.
    other: i32,
}

#[tokio::main]
async fn main() {

    let async_db = AsyncDb::new(
        "http://localhost:8086/", // URL to InfluxDB
        "my_database"             // Name of the database in InfluxDB
    ).expect("Unable to create AsyncDb");

    let now = Utc::now().timestamp_millis();
    let batch = vec![
        MyMeasure { region: String::from("us-east"), count: 3, when: now, other: 0 },
        MyMeasure { region: String::from("us-west"), count: 20, when: now, other: 1 },
    ];

    let insert = async_db.add_data(&batch).await;
}
```

## Caveats

- Because InfluxDB acknowledges requests by ending the HTTP session before it
  has actually performed the requested action, occasionally tests may fail.
  Examples include:
  - The database has not been created when an indexing request is sent
  - The data has not been indexed when a query request is sent
- String escaping has not been implemented; attempting to send the following
  characters will result in malformed Line Protocol data being sent:
  - In measurements: commas or spaces
  - In tag keys, tag values, and field keys: commas, equal signs, or spaces
  - In string field values: quotes
- Currently, queries return values as `serde_json::Value`s. This is a leaky
  abstraction, and not all `serde_json::Value`s are possible.

## Features not currently implemented

- HTTPS/TLS
- InfluxDB Authorization
- DB Queries
- Chunked responses
- UDP data insertion

## License

influxdb-rs is distributed under the terms of both the MIT license and the
Apache License (Version 2.0).

## Authors

This crate was created by Jake Goulding and Carol (Nichols || Goulding) of
[Integer 32][], sponsored by Stephan Buys of [Panoptix][].

[Integer 32]: http://www.integer32.com/
[Panoptix]: http://www.panoptix.co.za/
