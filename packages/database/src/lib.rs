//! Postgres bağlantısı ve üstündeki depolar.
//!
//! SurrealDB ve Qdrant çıkarıldı: ikisi de hiçbir servis tarafından
//! kullanılmıyordu ve seçim Postgres'ten yana yapıldı. Vektör veritabanı RAG
//! işine (#2) başlandığında geri gelebilir.

pub mod postgres;
pub mod prompt_store;

pub use postgres::{connect, PgPool};
pub use prompt_store::PostgresPromptStore;
