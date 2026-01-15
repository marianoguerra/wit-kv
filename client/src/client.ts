import { WitKvError } from './errors.js';
import type {
  ApiError,
  ContentFormat,
  DatabaseList,
  DeleteTypeOptions,
  GetOptions,
  KeyFilter,
  KeyList,
  KeyspaceList,
  ListOptions,
  MapOptions,
  MapResult,
  OperationOptions,
  ReduceOptions,
  ReduceResult,
  SetTypeOptions,
  TypeMetadata,
} from './types.js';

const MIME_WASM_WAVE = 'application/x-wasm-wave';
const MIME_OCTET_STREAM = 'application/octet-stream';

/** Options for building list query parameters */
interface ListParams {
  prefix?: string;
  start?: string;
  end?: string;
  limit?: number;
}

/**
 * Client for the wit-kv HTTP API.
 */
export class WitKvClient {
  private readonly baseUrl: string;
  private readonly defaultDatabase: string;

  /**
   * Create a new client.
   *
   * @param baseUrl - Base URL of the wit-kv server (e.g., "http://localhost:8080")
   * @param defaultDatabase - Default database name for operations (default: "default")
   */
  constructor(baseUrl: string, defaultDatabase = 'default') {
    // Remove trailing slash if present
    this.baseUrl = baseUrl.replace(/\/$/, '');
    this.defaultDatabase = defaultDatabase;
  }

  // ===========================================================================
  // Private Helpers
  // ===========================================================================

  /**
   * Get the Accept header value for the given format.
   */
  private acceptHeader(format: ContentFormat): string {
    return format === 'binary' ? MIME_OCTET_STREAM : MIME_WASM_WAVE;
  }

  /**
   * Build URL with optional query parameters for list operations.
   */
  private buildListUrl(basePath: string, params?: ListParams): string {
    const searchParams = new URLSearchParams();
    if (params?.prefix !== undefined) searchParams.set('prefix', params.prefix);
    if (params?.start !== undefined) searchParams.set('start', params.start);
    if (params?.end !== undefined) searchParams.set('end', params.end);
    if (params?.limit !== undefined) searchParams.set('limit', String(params.limit));
    const queryString = searchParams.toString();
    return queryString ? `${basePath}?${queryString}` : basePath;
  }

  /**
   * Fetch with format negotiation. Returns text or ArrayBuffer based on format.
   */
  private async fetchWithFormat(
    url: string,
    format: ContentFormat
  ): Promise<string | ArrayBuffer> {
    const response = await fetch(url, {
      headers: { Accept: this.acceptHeader(format) },
    });
    if (!response.ok) {
      throw await this.parseError(response);
    }
    return format === 'binary' ? response.arrayBuffer() : response.text();
  }

  /**
   * Get a value from the store.
   *
   * @param keyspace - Keyspace name
   * @param key - Key to retrieve
   * @param options - Operation options
   * @returns The value as a string (Wave format) or ArrayBuffer (binary format)
   */
  async get(
    keyspace: string,
    key: string,
    options?: GetOptions
  ): Promise<string | ArrayBuffer> {
    const db = options?.database ?? this.defaultDatabase;
    const format = options?.format ?? 'wave';
    const url = `${this.baseUrl}/api/v1/db/${encodeURIComponent(db)}/kv/${encodeURIComponent(keyspace)}/${encodeURIComponent(key)}`;

    return this.fetchWithFormat(url, format);
  }

  /**
   * Set a value in the store.
   *
   * @param keyspace - Keyspace name
   * @param key - Key to set
   * @param value - Value to store (Wave text format)
   * @param options - Operation options
   */
  async set(
    keyspace: string,
    key: string,
    value: string,
    options?: OperationOptions
  ): Promise<void> {
    const db = options?.database ?? this.defaultDatabase;

    const response = await fetch(
      `${this.baseUrl}/api/v1/db/${encodeURIComponent(db)}/kv/${encodeURIComponent(keyspace)}/${encodeURIComponent(key)}`,
      {
        method: 'PUT',
        headers: {
          'Content-Type': MIME_WASM_WAVE,
        },
        body: value,
      }
    );

    if (!response.ok) {
      throw await this.parseError(response);
    }
  }

  /**
   * Delete a value from the store.
   *
   * @param keyspace - Keyspace name
   * @param key - Key to delete
   * @param options - Operation options
   */
  async delete(
    keyspace: string,
    key: string,
    options?: OperationOptions
  ): Promise<void> {
    const db = options?.database ?? this.defaultDatabase;

    const response = await fetch(
      `${this.baseUrl}/api/v1/db/${encodeURIComponent(db)}/kv/${encodeURIComponent(keyspace)}/${encodeURIComponent(key)}`,
      {
        method: 'DELETE',
      }
    );

    if (!response.ok) {
      throw await this.parseError(response);
    }
  }

  /**
   * List keys in a keyspace.
   *
   * @param keyspace - Keyspace name
   * @param options - List options
   * @returns Array of key names (wave format) or ArrayBuffer (binary format)
   */
  async list(
    keyspace: string,
    options?: ListOptions & GetOptions
  ): Promise<string[] | ArrayBuffer> {
    const db = options?.database ?? this.defaultDatabase;
    const format = options?.format ?? 'wave';
    const basePath = `${this.baseUrl}/api/v1/db/${encodeURIComponent(db)}/kv/${encodeURIComponent(keyspace)}`;
    const url = this.buildListUrl(basePath, options);

    const result = await this.fetchWithFormat(url, format);
    if (format === 'binary') {
      return result as ArrayBuffer;
    }
    return this.parseKeyList(result as string);
  }

  /**
   * List keys in a keyspace (raw response).
   *
   * @param keyspace - Keyspace name
   * @param options - List options
   * @returns KeyList object or ArrayBuffer (binary format)
   */
  async listRaw(
    keyspace: string,
    options?: ListOptions & GetOptions
  ): Promise<KeyList | ArrayBuffer> {
    const db = options?.database ?? this.defaultDatabase;
    const format = options?.format ?? 'wave';
    const basePath = `${this.baseUrl}/api/v1/db/${encodeURIComponent(db)}/kv/${encodeURIComponent(keyspace)}`;
    const url = this.buildListUrl(basePath, options);

    const result = await this.fetchWithFormat(url, format);
    if (format === 'binary') {
      return result as ArrayBuffer;
    }
    return { keys: this.parseKeyList(result as string) };
  }

  /**
   * Get type metadata for a keyspace.
   *
   * @param keyspace - Keyspace name
   * @param options - Operation options
   * @returns Type metadata
   */
  async getType(
    keyspace: string,
    options?: OperationOptions
  ): Promise<TypeMetadata> {
    const db = options?.database ?? this.defaultDatabase;

    const response = await fetch(
      `${this.baseUrl}/api/v1/db/${encodeURIComponent(db)}/types/${encodeURIComponent(keyspace)}`
    );

    if (!response.ok) {
      throw await this.parseError(response);
    }

    return response.json();
  }

  /**
   * Register a type for a keyspace.
   *
   * @param keyspace - Keyspace name
   * @param witDefinition - WIT type definition
   * @param options - Set type options
   * @returns Type metadata for the registered type
   */
  async setType(
    keyspace: string,
    witDefinition: string,
    options?: SetTypeOptions
  ): Promise<TypeMetadata> {
    const db = options?.database ?? this.defaultDatabase;
    const params = new URLSearchParams();

    if (options?.typeName !== undefined)
      params.set('type_name', options.typeName);
    if (options?.force) params.set('force', 'true');

    const queryString = params.toString();
    const url = `${this.baseUrl}/api/v1/db/${encodeURIComponent(db)}/types/${encodeURIComponent(keyspace)}${queryString ? '?' + queryString : ''}`;

    const response = await fetch(url, {
      method: 'PUT',
      headers: {
        'Content-Type': 'text/plain',
      },
      body: witDefinition,
    });

    if (!response.ok) {
      throw await this.parseError(response);
    }

    return response.json();
  }

  /**
   * Delete a type from a keyspace.
   *
   * @param keyspace - Keyspace name
   * @param options - Delete type options
   */
  async deleteType(
    keyspace: string,
    options?: DeleteTypeOptions
  ): Promise<void> {
    const db = options?.database ?? this.defaultDatabase;
    const params = new URLSearchParams();

    if (options?.deleteData) params.set('delete_data', 'true');

    const queryString = params.toString();
    const url = `${this.baseUrl}/api/v1/db/${encodeURIComponent(db)}/types/${encodeURIComponent(keyspace)}${queryString ? '?' + queryString : ''}`;

    const response = await fetch(url, {
      method: 'DELETE',
    });

    if (!response.ok) {
      throw await this.parseError(response);
    }
  }

  /**
   * List all types in a database.
   *
   * @param options - Operation options
   * @returns Array of type metadata (wave format) or ArrayBuffer (binary format)
   */
  async listTypes(options?: GetOptions): Promise<TypeMetadata[] | ArrayBuffer> {
    const db = options?.database ?? this.defaultDatabase;
    const format = options?.format ?? 'wave';
    const url = `${this.baseUrl}/api/v1/db/${encodeURIComponent(db)}/types`;

    const result = await this.fetchWithFormat(url, format);
    if (format === 'binary') {
      return result as ArrayBuffer;
    }
    return this.parseKeyspaceList(result as string);
  }

  /**
   * List all types in a database (raw response).
   *
   * @param options - Operation options
   * @returns KeyspaceList object or ArrayBuffer (binary format)
   */
  async listTypesRaw(options?: GetOptions): Promise<KeyspaceList | ArrayBuffer> {
    const db = options?.database ?? this.defaultDatabase;
    const format = options?.format ?? 'wave';
    const url = `${this.baseUrl}/api/v1/db/${encodeURIComponent(db)}/types`;

    const result = await this.fetchWithFormat(url, format);
    if (format === 'binary') {
      return result as ArrayBuffer;
    }
    return { keyspaces: this.parseKeyspaceList(result as string) };
  }

  /**
   * List all databases.
   *
   * @param options - Get options
   * @returns Array of database names (wave format) or ArrayBuffer (binary format)
   */
  async listDatabases(options?: Omit<GetOptions, 'database'>): Promise<string[] | ArrayBuffer> {
    const format = options?.format ?? 'wave';
    const url = `${this.baseUrl}/api/v1/databases`;

    const result = await this.fetchWithFormat(url, format);
    if (format === 'binary') {
      return result as ArrayBuffer;
    }
    return this.parseDatabaseList(result as string);
  }

  /**
   * List all databases (raw response).
   *
   * @param options - Get options
   * @returns DatabaseList object or ArrayBuffer (binary format)
   */
  async listDatabasesRaw(options?: Omit<GetOptions, 'database'>): Promise<DatabaseList | ArrayBuffer> {
    const format = options?.format ?? 'wave';
    const url = `${this.baseUrl}/api/v1/databases`;

    const result = await this.fetchWithFormat(url, format);
    if (format === 'binary') {
      return result as ArrayBuffer;
    }
    const names = this.parseDatabaseList(result as string);
    return { databases: names.map(name => ({ name })) };
  }

  /**
   * Check server health.
   *
   * @returns Health status string
   */
  async health(): Promise<string> {
    const response = await fetch(`${this.baseUrl}/health`);
    return response.text();
  }

  // ===========================================================================
  // Map/Reduce Operations
  // ===========================================================================

  /**
   * Execute a map operation on a keyspace.
   *
   * Map operations apply a WASM component to each value in a keyspace.
   * The component must export `filter(value: T) -> bool` and `transform(value: T) -> T1`.
   *
   * @param keyspace - Keyspace to operate on
   * @param module - WASM component bytes (ArrayBuffer, Uint8Array, or Blob)
   * @param witDefinition - WIT definition text for the module's types
   * @param inputType - Name of the input type in the WIT definition
   * @param options - Map options (outputType, filter, database)
   * @returns Map result with processed, transformed, filtered counts and results
   *
   * @example
   * ```typescript
   * const module = await fetch('point-filter.wasm').then(r => r.arrayBuffer());
   * const wit = `
   *   package my:types;
   *   interface types {
   *     record point { x: s32, y: s32 }
   *   }
   * `;
   *
   * const result = await client.map('points', module, wit, 'point', {
   *   filter: { prefix: 'user:' }
   * });
   *
   * console.log(`Processed ${result.processed}, transformed ${result.transformed}`);
   * for (const [key, value] of result.results) {
   *   console.log(`${key}: ${value}`);
   * }
   * ```
   */
  async map(
    keyspace: string,
    module: ArrayBuffer | Uint8Array | Blob,
    witDefinition: string,
    inputType: string,
    options?: MapOptions
  ): Promise<MapResult> {
    const db = options?.database ?? this.defaultDatabase;

    // Build the config JSON (using snake_case for API compatibility)
    const config: Record<string, unknown> = {
      wit_definition: witDefinition,
      input_type: inputType,
    };

    if (options?.outputType !== undefined) {
      config.output_type = options.outputType;
    }

    if (options?.filter !== undefined) {
      config.filter = this.buildFilterConfig(options.filter);
    }

    // Create multipart form data
    const formData = new FormData();
    let moduleBlob: Blob;
    if (module instanceof Blob) {
      moduleBlob = module;
    } else if (module instanceof Uint8Array) {
      // Create a copy as ArrayBuffer to avoid SharedArrayBuffer issues
      moduleBlob = new Blob([module.slice().buffer], { type: 'application/wasm' });
    } else {
      moduleBlob = new Blob([module], { type: 'application/wasm' });
    }
    formData.append('module', moduleBlob, 'module.wasm');
    formData.append('config', JSON.stringify(config));

    const response = await fetch(
      `${this.baseUrl}/api/v1/db/${encodeURIComponent(db)}/map/${encodeURIComponent(keyspace)}`,
      {
        method: 'POST',
        body: formData,
      }
    );

    if (!response.ok) {
      throw await this.parseError(response);
    }

    const result = await response.json();
    return this.parseMapResult(result);
  }

  /**
   * Execute a reduce operation on a keyspace.
   *
   * Reduce operations fold all values in a keyspace into a single state using a WASM component.
   * The component must export `init-state() -> State` and `reduce(state: State, value: T) -> State`.
   *
   * @param keyspace - Keyspace to operate on
   * @param module - WASM component bytes (ArrayBuffer, Uint8Array, or Blob)
   * @param witDefinition - WIT definition text for the module's types
   * @param inputType - Name of the input/value type in the WIT definition
   * @param stateType - Name of the state type in the WIT definition
   * @param options - Reduce options (filter, database)
   * @returns Reduce result with processed count, errors, and final state
   *
   * @example
   * ```typescript
   * const module = await fetch('sum-scores.wasm').then(r => r.arrayBuffer());
   * const wit = `
   *   package my:types;
   *   interface types {
   *     record person { age: u8, score: u32 }
   *     record total { sum: u64, count: u32 }
   *   }
   * `;
   *
   * const result = await client.reduce('users', module, wit, 'person', 'total');
   *
   * console.log(`Processed ${result.processed} values`);
   * console.log(`Final state: ${result.state}`);
   * ```
   */
  async reduce(
    keyspace: string,
    module: ArrayBuffer | Uint8Array | Blob,
    witDefinition: string,
    inputType: string,
    stateType: string,
    options?: ReduceOptions
  ): Promise<ReduceResult> {
    const db = options?.database ?? this.defaultDatabase;

    // Build the config JSON (using snake_case for API compatibility)
    const config: Record<string, unknown> = {
      wit_definition: witDefinition,
      input_type: inputType,
      state_type: stateType,
    };

    if (options?.filter !== undefined) {
      config.filter = this.buildFilterConfig(options.filter);
    }

    // Create multipart form data
    const formData = new FormData();
    let moduleBlob: Blob;
    if (module instanceof Blob) {
      moduleBlob = module;
    } else if (module instanceof Uint8Array) {
      // Create a copy as ArrayBuffer to avoid SharedArrayBuffer issues
      moduleBlob = new Blob([module.slice().buffer], { type: 'application/wasm' });
    } else {
      moduleBlob = new Blob([module], { type: 'application/wasm' });
    }
    formData.append('module', moduleBlob, 'module.wasm');
    formData.append('config', JSON.stringify(config));

    const response = await fetch(
      `${this.baseUrl}/api/v1/db/${encodeURIComponent(db)}/reduce/${encodeURIComponent(keyspace)}`,
      {
        method: 'POST',
        body: formData,
      }
    );

    if (!response.ok) {
      throw await this.parseError(response);
    }

    const result = await response.json();
    return this.parseReduceResult(result);
  }

  /**
   * Build a filter config object with snake_case keys for the API.
   */
  private buildFilterConfig(filter: KeyFilter): Record<string, unknown> {
    const config: Record<string, unknown> = {};
    if (filter.key !== undefined) config.key = filter.key;
    if (filter.prefix !== undefined) config.prefix = filter.prefix;
    if (filter.start !== undefined) config.start = filter.start;
    if (filter.end !== undefined) config.end = filter.end;
    if (filter.limit !== undefined) config.limit = filter.limit;
    return config;
  }

  /**
   * Parse a map result from the API response (snake_case to camelCase).
   */
  private parseMapResult(result: Record<string, unknown>): MapResult {
    return {
      processed: result.processed as number,
      transformed: result.transformed as number,
      filtered: result.filtered as number,
      errors: result.errors as [string, string][],
      results: result.results as [string, string][],
    };
  }

  /**
   * Parse a reduce result from the API response (snake_case to camelCase).
   */
  private parseReduceResult(result: Record<string, unknown>): ReduceResult {
    return {
      processed: result.processed as number,
      errorCount: result.error_count as number,
      errors: result.errors as [string, string][],
      state: result.state as string,
    };
  }

  private async parseError(response: Response): Promise<WitKvError> {
    try {
      const body: ApiError = await response.json();
      return new WitKvError(
        body.error.code,
        body.error.message,
        response.status,
        body.error.details
      );
    } catch {
      return new WitKvError(
        'UNKNOWN_ERROR',
        `HTTP ${response.status}: ${response.statusText}`,
        response.status
      );
    }
  }

  /**
   * Parse a key-list WAVE format string.
   * Format: {keys: ["key1", "key2"]}
   */
  private parseKeyList(wave: string): string[] {
    // Simple WAVE parser for key-list format
    // Match the keys array content
    const match = wave.match(/\{keys:\s*\[(.*?)\]\}/s);
    if (!match) {
      return [];
    }

    const content = match[1].trim();
    if (!content) {
      return [];
    }

    // Parse quoted strings
    const keys: string[] = [];
    const stringRegex = /"([^"\\]*(?:\\.[^"\\]*)*)"/g;
    let stringMatch;
    while ((stringMatch = stringRegex.exec(content)) !== null) {
      // Unescape the string
      keys.push(stringMatch[1].replace(/\\(.)/g, '$1'));
    }

    return keys;
  }

  /**
   * Parse a keyspace-list WAVE format string.
   * Format: {keyspaces: [{name: "...", ...}, ...]}
   */
  private parseKeyspaceList(wave: string): TypeMetadata[] {
    // For keyspace-list, we need to parse the nested records
    // This is a simplified parser that extracts the keyspaces array
    const keyspaces: TypeMetadata[] = [];

    // Find all keyspace records
    // Pattern: {name: "...", qualified-name: "...", ...}
    const recordRegex = /\{name:\s*"([^"\\]*(?:\\.[^"\\]*)*)",\s*qualified-name:\s*"([^"\\]*(?:\\.[^"\\]*)*)",\s*wit-definition:\s*"([^"\\]*(?:\\.[^"\\]*)*)",\s*type-name:\s*"([^"\\]*(?:\\.[^"\\]*)*)",\s*type-version:\s*\{major:\s*(\d+),\s*minor:\s*(\d+),\s*patch:\s*(\d+)\},\s*type-hash:\s*(\d+),\s*created-at:\s*(\d+)\}/g;

    let match;
    while ((match = recordRegex.exec(wave)) !== null) {
      keyspaces.push({
        name: match[1].replace(/\\(.)/g, '$1'),
        qualified_name: match[2].replace(/\\(.)/g, '$1'),
        wit_definition: match[3].replace(/\\(.)/g, '$1'),
        type_name: match[4].replace(/\\(.)/g, '$1'),
        type_version: {
          major: parseInt(match[5], 10),
          minor: parseInt(match[6], 10),
          patch: parseInt(match[7], 10),
        },
        type_hash: parseInt(match[8], 10),
        created_at: parseInt(match[9], 10),
      });
    }

    return keyspaces;
  }

  /**
   * Parse a database-list WAVE format string.
   * Format: {databases: [{name: "db1"}, {name: "db2"}]}
   */
  private parseDatabaseList(wave: string): string[] {
    // Extract database names from the WAVE format
    const names: string[] = [];
    const recordRegex = /\{name:\s*"([^"\\]*(?:\\.[^"\\]*)*)"\}/g;

    let match;
    while ((match = recordRegex.exec(wave)) !== null) {
      names.push(match[1].replace(/\\(.)/g, '$1'));
    }

    return names;
  }
}
