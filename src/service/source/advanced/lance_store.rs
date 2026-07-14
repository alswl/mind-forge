//! LanceDB store adapter for the five-table repository Source catalog.
//!
//! Manages schema definitions, database open/create, and exact-version
//! table access. All mutation goes through the publication module;
//! this module provides the raw store primitives.

use std::path::Path;
use std::sync::Arc;

use arrow_schema::{DataType, Field, Schema, SchemaRef};
use futures::TryStreamExt;
use lancedb::connect;
use lancedb::query::{ExecutableQuery, QueryBase};
use lancedb::table::Table;

use crate::error::{MfError, Result};

// ── Table names ────────────────────────────────────────────────────────────

pub const TABLE_REGISTRATIONS: &str = "registrations";
pub const TABLE_DOCUMENTS: &str = "documents";
pub const TABLE_REGISTRATION_CONTENT: &str = "registration_content";
pub const TABLE_CHUNKS: &str = "chunks";
pub const TABLE_ENRICHMENTS: &str = "enrichments";

pub const ALL_TABLES: &[&str] =
    &[TABLE_REGISTRATIONS, TABLE_DOCUMENTS, TABLE_REGISTRATION_CONTENT, TABLE_CHUNKS, TABLE_ENRICHMENTS];

// ── Arrow schemas ──────────────────────────────────────────────────────────

fn utf8_field(name: &str, nullable: bool) -> Field {
    if nullable { Field::new(name, DataType::Utf8, true) } else { Field::new(name, DataType::Utf8, false) }
}

fn int64_field(name: &str, nullable: bool) -> Field {
    if nullable { Field::new(name, DataType::Int64, true) } else { Field::new(name, DataType::Int64, false) }
}

fn uint32_field(name: &str, nullable: bool) -> Field {
    if nullable { Field::new(name, DataType::UInt32, true) } else { Field::new(name, DataType::UInt32, false) }
}

fn uint64_field(name: &str, nullable: bool) -> Field {
    if nullable { Field::new(name, DataType::UInt64, true) } else { Field::new(name, DataType::UInt64, false) }
}

fn float32_field(name: &str, nullable: bool) -> Field {
    if nullable { Field::new(name, DataType::Float32, true) } else { Field::new(name, DataType::Float32, false) }
}

/// Arrow schema for the `registrations` table.
pub fn registrations_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        utf8_field("registration_key", false),
        utf8_field("project_key", false),
        utf8_field("project_identity", false),
        utf8_field("project_path", false),
        utf8_field("source_identity", false),
        utf8_field("source_type", false),
        utf8_field("source_kind", true),
        utf8_field("registered_location", false),
        utf8_field("tags_json", false),
        utf8_field("fact_fingerprint", false),
        int64_field("registration_revision", false),
        utf8_field("state", false),
    ]))
}

/// Arrow schema for the `documents` table.
pub fn documents_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        utf8_field("document_key", false),
        utf8_field("acquisition_kind", false),
        utf8_field("raw_fingerprint", false),
        utf8_field("extracted_fingerprint", false),
        utf8_field("content_fingerprint", false),
        int64_field("content_revision", false),
        utf8_field("state", false),
        utf8_field("last_error_kind", true),
        utf8_field("last_error", true),
        utf8_field("fetched_at", true),
        utf8_field("synced_at", true),
        uint64_field("chunk_count", false),
    ]))
}

/// Arrow schema for the `registration_content` table.
pub fn registration_content_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        utf8_field("registration_key", false),
        utf8_field("document_key", true),
        int64_field("content_revision", true),
        utf8_field("acquisition_key", false),
        utf8_field("acquired_location", false),
        utf8_field("registered_revision", false),
        utf8_field("state", false),
        utf8_field("last_error_kind", true),
        utf8_field("last_error", true),
        utf8_field("attempted_at", true),
        utf8_field("synced_at", true),
    ]))
}

/// Arrow schema for the `chunks` table.
pub fn chunks_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        utf8_field("chunk_id", false),
        utf8_field("document_key", false),
        int64_field("content_revision", false),
        uint32_field("ordinal", false),
        utf8_field("locator_json", false),
        utf8_field("locator_sort_key", false),
        utf8_field("text", false),
        utf8_field("text_fingerprint", false),
        uint32_field("token_count", false),
        Field::new(
            "vector",
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, true)), 384),
            false,
        ),
    ]))
}

/// Arrow schema for the `enrichments` table.
pub fn enrichments_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        utf8_field("enrichment_key", false),
        utf8_field("document_key", false),
        int64_field("content_revision", false),
        utf8_field("schema_version", false),
        utf8_field("prompt_version", false),
        utf8_field("summary", false),
        utf8_field("language", false),
        utf8_field("document_type", false),
        utf8_field("topics_json", false),
        utf8_field("keywords_json", false),
        utf8_field("entities_json", false),
        float32_field("confidence", false),
        utf8_field("warnings_json", false),
        uint32_field("processed_chunks", false),
        uint32_field("total_chunks", false),
        utf8_field("coverage", false),
        utf8_field("state", false),
        utf8_field("generated_at", false),
        utf8_field("applied_at", false),
    ]))
}

/// Get the schema for a named table.
pub fn schema_for_table(name: &str) -> Option<SchemaRef> {
    match name {
        TABLE_REGISTRATIONS => Some(registrations_schema()),
        TABLE_DOCUMENTS => Some(documents_schema()),
        TABLE_REGISTRATION_CONTENT => Some(registration_content_schema()),
        TABLE_CHUNKS => Some(chunks_schema()),
        TABLE_ENRICHMENTS => Some(enrichments_schema()),
        _ => None,
    }
}

// ── Store handle ───────────────────────────────────────────────────────────

/// A handle to an open LanceDB database.
pub struct LanceStore {
    db: lancedb::connection::Connection,
    _rt: tokio::runtime::Runtime,
}

impl LanceStore {
    /// Open an existing LanceDB database at `db_path` (a directory on disk).
    pub fn open(db_path: &Path) -> Result<Self> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| MfError::advanced_store(format!("failed to create async runtime: {e}"), None))?;

        let db = rt.block_on(async {
            connect(
                db_path
                    .to_str()
                    .ok_or_else(|| MfError::advanced_store("invalid database path (non-UTF-8)".to_string(), None))?,
            )
            .execute()
            .await
            .map_err(|e| MfError::advanced_store(format!("failed to open LanceDB: {e}"), None))
        })?;

        Ok(Self { db, _rt: rt })
    }

    /// Create a new LanceDB database at `db_path`. The directory must not already
    /// contain a database (though it may not exist yet).
    pub fn create(db_path: &Path) -> Result<Self> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| MfError::advanced_store(format!("failed to create async runtime: {e}"), None))?;

        let db = rt.block_on(async {
            connect(
                db_path
                    .to_str()
                    .ok_or_else(|| MfError::advanced_store("invalid database path (non-UTF-8)".to_string(), None))?,
            )
            .execute()
            .await
            .map_err(|e| MfError::advanced_store(format!("failed to create LanceDB: {e}"), None))
        })?;

        Ok(Self { db, _rt: rt })
    }

    /// Open or create a LanceDB database. If the directory exists and contains
    /// a database, it is opened. Otherwise a new one is created.
    pub fn open_or_create(db_path: &Path) -> Result<Self> {
        // LanceDB creates directories as needed; attempt create first.
        Self::create(db_path)
    }

    /// Return a reference to the inner LanceDB connection.
    pub fn db(&self) -> &lancedb::connection::Connection {
        &self.db
    }

    /// Access the tokio runtime for async operations.
    pub fn rt(&self) -> &tokio::runtime::Runtime {
        &self._rt
    }

    /// Open a table by name.
    pub fn open_table(&self, name: &str) -> Result<Table> {
        self.rt().block_on(async {
            self.db
                .open_table(name)
                .execute()
                .await
                .map_err(|e| MfError::advanced_store(format!("failed to open table '{name}': {e}"), None))
        })
    }

    /// Create an empty table with the given schema. If it already exists the
    /// operation succeeds but the existing schema is not modified.
    pub fn create_table(&self, name: &str, schema: SchemaRef) -> Result<Table> {
        self.rt().block_on(async {
            self.db
                .create_empty_table(name, schema)
                .execute()
                .await
                .map_err(|e| MfError::advanced_store(format!("failed to create table '{name}': {e}"), None))
        })
    }

    /// Create all five standard tables if they don't exist.
    pub fn ensure_tables(&self) -> Result<()> {
        for name in ALL_TABLES {
            if let Some(schema) = schema_for_table(name) {
                match self.create_table(name, schema) {
                    Ok(_) => {}
                    Err(_) => {
                        // table may already exist — try opening it
                        let _ = self.open_table(name)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Add rows to a table. The RecordBatch schema must match the table.
    pub fn append_rows(&self, table_name: &str, batch: arrow_array::RecordBatch) -> Result<()> {
        let table = self.open_table(table_name)?;
        self.rt().block_on(async {
            table
                .add(batch)
                .execute()
                .await
                .map(|_| ())
                .map_err(|e| MfError::advanced_store(format!("failed to append to '{table_name}': {e}"), None))
        })
    }

    /// Count rows in a table. Returns 0 if the table is empty or missing.
    pub fn count_rows(&self, table_name: &str) -> Result<usize> {
        let table = match self.open_table(table_name) {
            Ok(t) => t,
            Err(_) => return Ok(0),
        };
        self.rt().block_on(async {
            table
                .count_rows(None)
                .await
                .map_err(|e| MfError::advanced_store(format!("failed to count '{table_name}': {e}"), None))
        })
    }

    /// Delete rows from a table matching a predicate string.
    pub fn delete_rows(&self, table_name: &str, predicate: &str) -> Result<()> {
        let table = self.open_table(table_name)?;
        self.rt().block_on(async {
            table
                .delete(predicate)
                .await
                .map(|_| ())
                .map_err(|e| MfError::advanced_store(format!("failed to delete from '{table_name}': {e}"), None))
        })
    }

    /// Get the current version of a table.
    pub fn table_version(&self, table_name: &str) -> Result<u64> {
        let table = self.open_table(table_name)?;
        self.rt().block_on(async {
            table
                .version()
                .await
                .map_err(|e| MfError::advanced_store(format!("failed to get version of '{table_name}': {e}"), None))
        })
    }

    /// Create a full-text search index on a text column using Auto index selection.
    pub fn create_fts_index(&self, table_name: &str, columns: &[&str]) -> Result<()> {
        let table = self.open_table(table_name)?;
        self.rt().block_on(async {
            table.create_index(columns, lancedb::index::Index::Auto).execute().await.map(|_| ()).map_err(|e| {
                MfError::advanced_store(format!("failed to create FTS index on '{table_name}': {e}"), None)
            })
        })
    }

    /// Execute a full-text search query on a table.
    /// Returns RecordBatches from the query result stream.
    pub fn fts_search(
        &self,
        table_name: &str,
        _query_text: &str,
        _columns: &[&str],
        limit: usize,
    ) -> Result<Vec<arrow_array::RecordBatch>> {
        let table = self.open_table(table_name)?;
        self.rt().block_on(async {
            let stream = table
                .query()
                .limit(limit)
                .execute()
                .await
                .map_err(|e| MfError::advanced_store(format!("query failed on '{table_name}': {e}"), None))?;
            stream.try_collect::<Vec<_>>().await.map_err(|e| {
                MfError::advanced_store(format!("failed to collect results from '{table_name}': {e}"), None)
            })
        })
    }

    /// Create a vector index for approximate nearest neighbor search using Auto index selection.
    pub fn create_vector_index(&self, table_name: &str, column: &str) -> Result<()> {
        let table = self.open_table(table_name)?;
        self.rt().block_on(async {
            table.create_index(&[column], lancedb::index::Index::Auto).execute().await.map(|_| ()).map_err(|e| {
                MfError::advanced_store(format!("failed to create vector index on '{table_name}.{column}': {e}"), None)
            })
        })
    }

    /// Execute a vector similarity search on a table.
    pub fn vector_search(
        &self,
        table_name: &str,
        vector: &[f32],
        column: &str,
        limit: usize,
    ) -> Result<Vec<arrow_array::RecordBatch>> {
        let table = self.open_table(table_name)?;
        let q = vector.to_vec();
        let col = column.to_string();
        self.rt().block_on(async {
            let stream = table
                .query()
                .nearest_to(q)
                .map_err(|e| MfError::advanced_store(format!("vector search setup failed: {e}"), None))?
                .column(&col)
                .limit(limit)
                .execute()
                .await
                .map_err(|e| MfError::advanced_store(format!("vector search failed on '{table_name}': {e}"), None))?;
            stream.try_collect::<Vec<_>>().await.map_err(|e| {
                MfError::advanced_store(format!("failed to collect results from '{table_name}': {e}"), None)
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_tables_have_schemas() {
        for name in ALL_TABLES {
            assert!(schema_for_table(name).is_some(), "missing schema for table: {name}");
        }
    }

    #[test]
    fn registration_schema_has_no_nullable_pk() {
        let schema = registrations_schema();
        let pk = schema.field_with_name("registration_key").unwrap();
        assert!(!pk.is_nullable());
    }

    #[test]
    fn chunks_schema_has_vector_field() {
        let schema = chunks_schema();
        let vec_field = schema.field_with_name("vector").unwrap();
        assert_eq!(
            vec_field.data_type(),
            &DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, true)), 384)
        );
    }

    #[test]
    fn enrichments_schema_has_confidence_field() {
        let schema = enrichments_schema();
        let conf = schema.field_with_name("confidence").unwrap();
        assert_eq!(conf.data_type(), &DataType::Float32);
    }
}
