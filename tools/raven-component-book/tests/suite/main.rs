//! Guards that the book keeps giving the design tool what it needs: every story draws and
//! measures, the measurements match what RavenRS paints, and the files of the design system
//! are the shape the design tool reads.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "tests fail loudly")]

mod book;
mod export;
mod guardian;
mod novidades;
mod testable;
mod window;
