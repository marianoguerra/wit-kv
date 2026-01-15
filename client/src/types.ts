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
