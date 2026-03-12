use thiserror::Error;

pub mod influx;
pub mod sqlite;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("SQLite query failed: {0}")]
    SqliteError(#[from] sqlx::Error),

    #[error("InfluxDB operation failed: {0}")]
    InfluxError(#[from] influxdb2::RequestError),

    #[error("Record not found for device: {0}")]
    NotFound(String),

    #[error("Data parsing error: {0}")]
    ParseError(String),
}

// Result type alias gọn nhẹ cho module DB
pub type DbResult<T> = Result<T, DbError>;
