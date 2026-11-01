//! Report server for RavenRS Reporting: stores layouts with versions, renders
//! them over HTTP and delivers scheduled reports by email or to a folder.
//!
//! * [`config`] reads the server's configuration; secrets come from the environment.
//! * [`store`] keeps layouts, jobs, history and documents as files.
//! * [`cron`] parses schedules and finds the next run, time zones included.
//! * [`job`] is the job format and its validation.
//! * [`render`] resolves a layout's data and writes the output files.
//! * [`deliver`] sends files by email or writes them to a folder.
//! * [`scheduler`] runs what is due, with retries, history and retention.
//! * [`http`] is the API.
//!
//! The binary runs all of it from a configuration file. The server has one
//! administrative identity, its API token, and is meant to run as one instance.

pub mod config;
pub mod cron;
pub mod deliver;
pub mod http;
pub mod job;
pub mod render;
pub mod scheduler;
pub mod store;
