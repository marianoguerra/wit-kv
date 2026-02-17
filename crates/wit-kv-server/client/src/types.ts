/**
 * API error response body.
 */
export interface ApiError {
  error: {
    code: string;
    message: string;
    details?: Record<string, unknown>;
  };
}

/**
 * Semantic version.
 */
export interface SemanticVersion {
  major: number;
  minor: number;
  patch: number;
}

/**
 * Type metadata for a keyspace.
 */
export interface TypeMetadata {
  name: string;
  qualified_name: string;
  wit_definition: string;
  type_name: string;
  type_version: SemanticVersion;
  type_hash: number;
  created_at: number;
}

/**
 * List of keys response (mirrors key-list WIT type).
 */
export interface KeyList {
  keys: string[];
}

/**
 * List of keyspaces response (mirrors keyspace-list WIT type).
 */
export interface KeyspaceList {
  keyspaces: TypeMetadata[];
}

/**
 * Database information (mirrors database-info WIT type).
 */
export interface DatabaseInfo {
  name: string;
}

/**
 * List of databases response (mirrors database-list WIT type).
 */
export interface DatabaseList {
  databases: DatabaseInfo[];
}

/**
 * Options for listing keys.
 */
export interface ListOptions {
  prefix?: string;
  start?: string;
  end?: string;
  limit?: number;
}

/**
 * Content format for requests and responses.
 */
export type ContentFormat = 'wave' | 'binary';

/**
 * Common options for operations.
 */
export interface OperationOptions {
  /** Database name (defaults to 'default'). */
  database?: string;
}

/**
 * Options for get operations.
 */
export interface GetOptions extends OperationOptions {
  /** Response format. */
  format?: ContentFormat;
}

/**
 * Options for set type operations.
 */
export interface SetTypeOptions extends OperationOptions {
  /** Name of the type within the WIT definition. */
  typeName?: string;
  /** Force overwrite if type already exists. */
  force?: boolean;
}

/**
 * Options for delete type operations.
 */
export interface DeleteTypeOptions extends OperationOptions {
  /** Also delete all data in the keyspace. */
  deleteData?: boolean;
}

// =============================================================================
// Map/Reduce Types
// =============================================================================

/**
 * Key filter options for map/reduce operations.
 * Mirrors the key-filter WIT type.
 */
export interface KeyFilter {
  /** Single key to process (if set, other filters are ignored). */
  key?: string;
  /** Prefix filter for keys. */
  prefix?: string;
  /** Start key (inclusive). */
  start?: string;
  /** End key (exclusive). */
  end?: string;
  /** Maximum number of keys to process. */
  limit?: number;
}

/**
 * Configuration for a map operation.
 * Sent as JSON in the multipart 'config' field.
 */
export interface MapConfig {
  /** WIT definition text for the module's types. */
  witDefinition: string;
  /** Name of the input type in the WIT definition. */
  inputType: string;
  /** Name of the output type (defaults to inputType if not specified). */
  outputType?: string;
  /** Optional key filters. */
  filter?: KeyFilter;
}

/**
 * Configuration for a reduce operation.
 * Sent as JSON in the multipart 'config' field.
 */
export interface ReduceConfig {
  /** WIT definition text for the module's types. */
  witDefinition: string;
  /** Name of the input/value type in the WIT definition. */
  inputType: string;
  /** Name of the state type in the WIT definition. */
  stateType: string;
  /** Optional key filters. */
  filter?: KeyFilter;
}

/**
 * Result of a map operation.
 * Mirrors the map-result WIT type.
 */
export interface MapResult {
  /** Number of keys processed. */
  processed: number;
  /** Number of keys that passed the filter and were transformed. */
  transformed: number;
  /** Number of keys filtered out. */
  filtered: number;
  /** Errors encountered: list of [key, error message]. */
  errors: [string, string][];
  /** Transformed results: list of [key, wave-encoded value]. */
  results: [string, string][];
}

/**
 * Result of a reduce operation.
 * Mirrors the reduce-result WIT type.
 */
export interface ReduceResult {
  /** Number of values processed. */
  processed: number;
  /** Number of errors encountered. */
  errorCount: number;
  /** Errors encountered: list of [key, error message]. */
  errors: [string, string][];
  /** Final state as wave-encoded value. */
  state: string;
}

/**
 * Options for map operations.
 */
export interface MapOptions extends OperationOptions {
  /** Output type name (defaults to input type). */
  outputType?: string;
  /** Key filter options. */
  filter?: KeyFilter;
}

/**
 * Options for reduce operations.
 */
export interface ReduceOptions extends OperationOptions {
  /** Key filter options. */
  filter?: KeyFilter;
}
