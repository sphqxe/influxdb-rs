//! # influxdb
//!
//! influxdb provides an asynchronous Rust interface to an
//! [InfluxDB][] database.
//!
//! This crate supports insertion of strings already in the InfluxDB
//! Line Protocol. The `influxdb-derive` crate provides convenient
//! serialization of Rust structs to this format.
//!
//! [InfluxDB]: https://www.influxdata.com/
//!
//! # Examples
//!
//! To serialize a struct into the InfluxDB Line Protocol format, use the
//! `influxdb-derive` crate's macros as shown below with `MyMeasure`.
//!
//! Then create an instance of `influxdb::AsyncDb` and add instances of
//! your struct. Check out the code in the `examples` directory to see how
//! this code interacts with futures.
//!
//! ```
//! use influxdb;
//! #[macro_use]
//! use influxdb_derive;
//!
//! use std::time::SystemTime;
//! use influxdb::{Measurement, AsyncDb};
//!
//! // `Measurement` is the trait that `AsyncDb` needs in order to insert
//! #[derive(Measurement)]
//! // The default measurement name will be the struct name; this optional
//! // annotation allows customization of the name sent to InfluxDB.
//! #[influx(rename = "my_measure")]
//! struct MyMeasure {
//!     // Specify which struct fields are InfluxDB tags.
//!     // Tags must be `String`s or `&str`s.
//!     #[influx(tag)]
//!     region: String,
//!     // Specify which struct fields are InfluxDB fields.
//!     // Supported types are integers, floats, strings, and booleans.
//!     // The rename annotation works with struct fields as well.
//!     #[influx(field, rename = "amount")]
//!     count: i32,
//!     // Specify which struct field is the InfluxDB timestamp.
//!     #[influx(timestamp)]
//!     when: SystemTime,
//!     // Struct fields that aren't annotated won't be sent to InfluxDB.
//!     other: i32,
//! }
//!
//! fn main() {
//!     let mut core = tokio_core::reactor::Core::new()
//!         .expect("Unable to create reactor core");
//!
//!     let async_db = AsyncDb::new(
//!         "http://localhost:8086/", // URL to InfluxDB
//!         "my_database"             // Name of the database in InfluxDB
//!     ).expect("Unable to create AsyncDb");
//!
//!     let now = SystemTime::now();
//!     let batch = vec![
//!         MyMeasure { region: String::from("us-east"), count: 3, when: now, other: 0 },
//!         MyMeasure { region: String::from("us-west"), count: 20, when: now, other: 1 },
//!     ];
//!
//!     let insert = async_db.add_data(&batch).await;
//! }
//! ```

use reqwest::{Client, Response};
use serde::{Serialize, Deserialize};
use log::error;

#[macro_use]
extern crate quick_error;

pub mod measurement;
pub use measurement::Measurement;

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Url(error: url::ParseError) {
            description(error.description())
            display("Unable to parse URL: {}", error)
            from()
            cause(error)
        }
        Reqwest(error: reqwest::Error) {
            description(error.description())
            display("Unable to perform HTTP request: {}", error)
            from()
            cause(error)
        }
        Serde(error: serde_json::Error) {
            description(error.description())
            display("Unable to deserialize JSON: {}", error)
            from()
            cause(error)
        }
        BadRequest(what: String) {
            description("The InfluxDB server responded with an error")
            display("The InfluxDB server responded with an error: {}", what)
        }
        AddrParse(error: std::net::AddrParseError) {
            description(error.description())
            display("Unable to parse the address: {}", error)
            from()
            cause(error)
        }
    }
}

pub struct AsyncDb {
    name: String,
    query_endpoint: url::Url,
    write_endpoint: url::Url,
    client: reqwest::Client,
}

impl AsyncDb {
    pub fn new(base_url: &str, name: &str) -> Result<Self, Error> {
        let base_url = url::Url::parse(base_url)?;
        let query_endpoint = base_url.join("/query")?;
        let mut write_endpoint = base_url.join("/write")?;
        write_endpoint.query_pairs_mut().append_pair("db", &name);

        Ok(AsyncDb {
            name: name.into(),
            query_endpoint: query_endpoint,
            write_endpoint: write_endpoint,
            client: Client::new(),
        })
    }

    pub async fn add_data<T: Measurement>(&self, measure: T) -> Result<Response, Error> {
        let mut bytes_to_send = String::new();
        measure.to_data(&mut bytes_to_send);
        let response = self.client.post(self.write_endpoint.clone()).body(bytes_to_send).send().await.map_err(Error::Reqwest);
        match response {
            Ok(r) => {
                if r.status().is_client_error() {
                    Err(Error::BadRequest(r.text().await.unwrap()))
                } else {
                    Ok(r)
                }
            },
            Err(e) => Err(e)
        }
    }
//
//    pub fn query(&self, query: &str) -> Query {
//        let mut query_endpoint = self.query_endpoint.clone();
//        query_endpoint.query_pairs_mut()
//            .append_pair("db", &self.name)
//            .append_pair("q", query);
//
//        let response =
//            self.client.get(query_endpoint.as_str().parse().expect("Invalid query URL"))
//            .map_err(Error::Hyper)
//            .and_then(check_response_code)
//            .and_then(response_to_json);
//
//        Query(Box::new(response))
//    }
}

//#[derive(Debug, Deserialize)]
//pub struct QueryResponse {
//    pub results: Vec<QueryResult>,
//}
//
//#[derive(Debug, Deserialize)]
//pub struct QueryResult {
//    #[serde(default)]
//    pub series: Vec<Series>,
//    pub error: Option<String>,
//    pub statement_id: usize,
//}
//
//#[derive(Debug, Deserialize)]
//pub struct Series {
//    pub name: String,
//    pub columns: Vec<String>, // TODO: `time` is always added?
//    pub values: Vec<Vec<serde_json::Value>>, // TODO: matches with columns?
//    // TODO: Don't expose serde types publically
//}

#[derive(Debug, Deserialize)]
pub struct InfluxServerError {
    pub error: String,
}