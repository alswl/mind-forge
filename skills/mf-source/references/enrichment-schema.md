# Enrichment JSON Schema (v1)

The `mf source advanced enrich apply` command accepts this exact shape.

```json
{
  "schema_version": "1",
  "prompt_version": "<canonical skill version fingerprint>",
  "document_key": "<SHA-256 hex>",
  "content_revision": 1,
  "summary": "One to three sentence factual summary of the document.",
  "language": "en",
  "document_type": "reference",
  "topics": [
    "embeddings",
    "information-retrieval",
    "vector-search"
  ],
  "keywords": [
    "cosine similarity",
    "dense retrieval",
    "embeddings",
    "hybrid search",
    "RRF",
    "semantic search",
    "sparse retrieval",
    "vector database"
  ],
  "entities": [
    {
      "name": "LanceDB",
      "type": "technology",
      "description": "Serverless vector database for AI applications"
    }
  ],
  "confidence": 0.85,
  "warnings": [],
  "processed_chunks": 8,
  "total_chunks": 8,
  "coverage": "complete"
}
```

## Field constraints

| Field | Type | Constraints |
|-------|------|-------------|
| `schema_version` | string | Must be `"1"` |
| `prompt_version` | string | Skill version fingerprint, ≤100 chars |
| `document_key` | string | SHA-256 hex, 64 chars |
| `content_revision` | integer | Must equal current document revision |
| `summary` | string | ≤2000 chars, non-empty |
| `language` | string | ISO 639-1 two-letter code |
| `document_type` | string | See allowed values below |
| `topics` | array[string] | 5-10 items, each ≤100 chars, sorted |
| `keywords` | array[string] | 5-15 items, each ≤100 chars, sorted |
| `entities` | array[object] | 0-50 items |
| `entities[].name` | string | ≤200 chars, non-empty |
| `entities[].type` | string | See allowed values below |
| `entities[].description` | string? | ≤500 chars, optional |
| `confidence` | number | 0.0 ≤ x ≤ 1.0 |
| `warnings` | array[string] | 0-20 items, each ≤500 chars |
| `processed_chunks` | integer | ≤ total_chunks, ≥0 |
| `total_chunks` | integer | Matches document chunk count |
| `coverage` | string | `"complete"` or `"partial"` |

## Allowed values

**document_type**: `article`, `report`, `manual`, `tutorial`, `reference`,
`specification`, `blog_post`, `readme`, `changelog`, `presentation`,
`dataset`, `code`, `other`

**entity.type**: `person`, `organization`, `product`, `technology`,
`location`, `event`, `concept`, `other`

**language**: ISO 639-1 two-letter codes (e.g. `en`, `zh`, `ja`, `ko`,
`fr`, `de`, `es`)

## Validation rules

1. All required fields must be present
2. `document_key` and `content_revision` must match the target document
3. Schema `"1"` is the only accepted version
4. Unknown fields are rejected (no forward-compatible passthrough)
5. Field sizes are strictly enforced; oversized fields are rejected
6. `confidence` outside [0.0, 1.0] is rejected
7. Entities must have unique `name` within the array
8. Source registration facts (name, tags, kind, location) must not be
   overwritten; the apply command will reject any attempt to mutate them
