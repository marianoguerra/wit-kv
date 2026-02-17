/**
 * TypeScript types generated from kv.wit
 * Package: wit-kv:storage@0.2.0
 * Interface: types
 */

/**
 * Semantic version following WIT package versioning convention.
 * WIT: record semantic-version { major: u32, minor: u32, patch: u32 }
 */
export interface SemanticVersion {
  major: number;
  minor: number;
  patch: number;
}

/**
 * Stored value envelope - wraps the actual value with metadata.
 * WIT: record stored-value { version: u8, type-version: semantic-version, value: list<u8>, memory: option<list<u8>> }
 */
export interface StoredValue {
  /** Format version for future compatibility */
  version: number;
  /** Type version at time of storage (semantic version) */
  typeVersion: SemanticVersion;
  /** Canonical ABI encoded value bytes */
  value: Uint8Array;
  /** Linear memory bytes (for variable-length types: strings, lists) */
  memory?: Uint8Array;
}

/**
 * Binary export format - self-describing canonical ABI encoding
 * for transferring complete values including linear memory.
 * WIT: record binary-export { value: list<u8>, memory: option<list<u8>> }
 */
export interface BinaryExport {
  /** Canonical ABI encoded value bytes */
  value: Uint8Array;
  /** Linear memory bytes (for variable-length types) */
  memory?: Uint8Array;
}

/**
 * Keyspace type metadata.
 * WIT: record keyspace-metadata
 */
export interface KeyspaceMetadata {
  /** User-visible keyspace name */
  name: string;
  /** Full WIT qualified name (namespace:package/interface#type) */
  qualifiedName: string;
  /** Full WIT type definition text */
  witDefinition: string;
  /** Type name within the WIT definition */
  typeName: string;
  /** Semantic version for the type schema */
  typeVersion: SemanticVersion;
  /** CRC32 hash of WIT definition */
  typeHash: number;
  /** Unix timestamp of creation */
  createdAt: number;
}

/**
 * List of keys in a keyspace.
 * WIT: record key-list { keys: list<string> }
 */
export interface KeyList {
  /** The keys in the keyspace */
  keys: string[];
}

/**
 * List of keyspaces (type registrations) in a database.
 * WIT: record keyspace-list { keyspaces: list<keyspace-metadata> }
 */
export interface KeyspaceList {
  /** The keyspaces with their metadata */
  keyspaces: KeyspaceMetadata[];
}

/**
 * Database information.
 * WIT: record database-info { name: string }
 */
export interface DatabaseInfo {
  /** Database name */
  name: string;
}

/**
 * List of databases.
 * WIT: record database-list { databases: list<database-info> }
 */
export interface DatabaseList {
  /** The databases */
  databases: DatabaseInfo[];
}

/**
 * Convert snake_case API response to camelCase WIT types.
 */
export function toKeyspaceMetadata(api: {
  name: string;
  qualified_name: string;
  wit_definition: string;
  type_name: string;
  type_version: { major: number; minor: number; patch: number };
  type_hash: number;
  created_at: number;
}): KeyspaceMetadata {
  return {
    name: api.name,
    qualifiedName: api.qualified_name,
    witDefinition: api.wit_definition,
    typeName: api.type_name,
    typeVersion: api.type_version,
    typeHash: api.type_hash,
    createdAt: api.created_at,
  };
}
