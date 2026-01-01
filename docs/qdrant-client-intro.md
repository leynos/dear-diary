# qdrant-client

Rust gRPC client for [Qdrant](https://qdrant.tech/), a high-performance vector
database. Enables vector search, collection management, and payload operations.

## Core Types

| Type | Purpose |
|------|---------|
| `Qdrant` | Main API client for server communication. Handles collection, point, search, and payload operations. |
| `Payload` | JSON-like metadata attached to points. Converts to/from `serde_json` objects (with `serde` feature). |
| `QdrantError` | Error enum covering connection issues, (de)serialization failures, and API errors. |
| `PointStruct` | A point: ID + vector + optional payload. |
| `CreateCollectionBuilder` | Builder for collection creation. |
| `VectorParamsBuilder` | Configures vector size and distance metric. |
| `UpsertPointsBuilder` | Builder for batch or single upsert operations. |
| `SearchPointsBuilder` | Builder for search queries. |

## Modules

| Module | Purpose |
|--------|---------|
| `config` | Client configuration |
| `qdrant` | API types |
| `deser` | Deserialize into any serde type |

## Usage Patterns

### Connecting to Qdrant

```rust
use qdrant_client::Qdrant;

let client = Qdrant::from_url("http://localhost:6334")
    .api_key(std::env::var("QDRANT_API_KEY"))
    .timeout(std::time::Duration::from_secs(10))
    .build()?;
```

### Creating a Collection

Qdrant organises data into [collections][collections] of [points][points].

```rust
use qdrant_client::qdrant::{CreateCollectionBuilder, Distance, VectorParamsBuilder};

let response = client
    .create_collection(
        CreateCollectionBuilder::new("my_collection")
            .vectors_config(VectorParamsBuilder::new(512, Distance::Cosine)),
    )
    .await?;
```

`VectorParamsBuilder::new` takes:

- Vector dimensionality (e.g., `512`)
- `Distance` metric for similarity measurement (`Cosine`, `Euclid`, `Dot`)

### Upserting Points

Points comprise an ID, one or more vectors, and an optional payload.

```rust
use qdrant_client::qdrant::{PointStruct, UpsertPointsBuilder};

let points = vec![
    PointStruct::new(
        42,                    // Unique point ID
        vec![0.0_f32; 512],    // Vector
        [                      // Payload (key-value pairs)
            ("great", true.into()),
            ("level", 9000.into()),
            ("text", "Hi Qdrant!".into()),
            ("list", vec![1.234f32, 0.815].into()),
        ],
    ),
];

let response = client
    .upsert_points(UpsertPointsBuilder::new("my_collection", points))
    .await?;
```

### Searching Points

```rust
use qdrant_client::qdrant::SearchPointsBuilder;

let search_request = SearchPointsBuilder::new(
    "my_collection",       // Collection name
    vec![0.0_f32; 512],    // Query vector
    4,                     // Limit (top-k results)
).with_payload(true);

let response = client.search_points(search_request).await?;
```

- `.with_payload(true)` — include full payload in results
- `.filter()` — apply a `Filter` to constrain results (see [`Filter` docs](https://qdrant.tech/documentation/concepts/search/))

## Key Types Reference

### Payload

```rust
pub struct Payload(/* private fields */);
```

JSON-like metadata attached to points. Supports filtering during vector search.

**Methods:**

- `new()` — construct empty payload
- `from(HashMap)` — construct from hash map
- `insert(key, value)` — insert/replace value at key
- `deserialize::<T>()` — deserialize into `T` (requires `serde::Deserialize`)
- `try_from(Value)` — convert from `serde_json::Value`

#### Serde Integration

*Requires `serde` feature.*

Convert between `serde_json::Value` and `Payload`:

```rust
use serde_json::{Value, json};

let value = json!({
    "city": "Berlin",
});

let payload = Payload::try_from(value).expect("not a JSON object");
let value = Value::from(payload);
```

Non-object JSON values return `QdrantError::JsonToPayload`.

Convert between `Map<String, Value>` and `Payload`:

```rust
use serde_json::{Map, Value};

let mut object = Map::new();
object.insert("city".to_string(), "Berlin".into());

let payload = Payload::from(object);
let object = Map::from(payload);
```

### Qdrant

```rust
pub struct Qdrant {
    pub config: QdrantConfig,
    // ...
}
```

**Operation categories:**

- Collection operations — manage collections, aliases, cluster configuration
- Point operations — manage points and vectors
- Payload operations — manage point payloads
- Search operations — search and explore points
- Query operations — universal search
- Index operations — field and payload indices
- Snapshot operations — instance or collection snapshots
- Shard key operations — manage shard keys

**Common methods:**

- `create_collection()` — create a new collection
- `collection_info()` — get collection info
- `list_collections()` — list all collections
- `collection_exists()` — check collection existence
- `update_collection()` — update collection settings

### QdrantError

```rust
pub enum QdrantError {
    ResponseError { status: Status },
    ResourceExhaustedError { status: Status, retry_after_seconds: u64 },
    ConversionError(String),
    InvalidUri(InvalidUri),
    NoSnapshotFound(String),
    Io(Error),
    Reqwest(Error),
    JsonToPayload(Value),
    PayloadDeserialization(DeserPayloadError),
}
```

## Traits

- `serde::Deserialize` / `serde::Serialize` — required for payload conversion
  with the `serde` feature

No custom traits are needed for defining points. The API uses builder patterns
throughout.

## Quick start

1. **Connect** — `Qdrant::from_url("...").build()?`, optionally with API key
   and timeout
2. **Create collection** — `CreateCollectionBuilder` + `VectorParamsBuilder`
   (name, vector size, distance metric)
3. **Upsert points** — `PointStruct` via `UpsertPointsBuilder`
4. **Search** — `SearchPointsBuilder` with query vector and parameters
5. **Handle payloads** — use `serde_json` conversions (with `serde` feature)
6. **Handle errors** — match on `QdrantError` variants

All operations are async and use builder patterns. For filtering, indexing, and
snapshots, consult the extended `Qdrant` client methods.

## Further reading

- [Collections][collections]
- [Points][points]
- [Payload](https://qdrant.tech/documentation/concepts/payload/)
- [Search](https://qdrant.tech/documentation/concepts/search/)

[collections]: https://qdrant.tech/documentation/concepts/collections/
[points]: https://qdrant.tech/documentation/concepts/points/
