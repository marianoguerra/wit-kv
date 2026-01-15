import { WitKvError } from './errors.js';
import type {
  ApiError,
  ContentFormat,
  DeleteTypeOptions,
  GetOptions,
  ListOptions,
  OperationOptions,
  SetTypeOptions,
  TypeMetadata,
} from './types.js';

const MIME_WASM_WAVE = 'application/x-wasm-wave';
const MIME_OCTET_STREAM = 'application/octet-stream';

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

    const response = await fetch(
      `${this.baseUrl}/api/v1/db/${encodeURIComponent(db)}/kv/${encodeURIComponent(keyspace)}/${encodeURIComponent(key)}`,
      {
        method: 'GET',
        headers: {
          Accept: format === 'binary' ? MIME_OCTET_STREAM : MIME_WASM_WAVE,
        },
      }
    );

    if (!response.ok) {
      throw await this.parseError(response);
    }

    return format === 'binary' ? response.arrayBuffer() : response.text();
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
   * @returns Array of key names
   */
  async list(
    keyspace: string,
    options?: ListOptions & OperationOptions
  ): Promise<string[]> {
    const db = options?.database ?? this.defaultDatabase;
    const params = new URLSearchParams();

    if (options?.prefix !== undefined) params.set('prefix', options.prefix);
    if (options?.start !== undefined) params.set('start', options.start);
    if (options?.end !== undefined) params.set('end', options.end);
    if (options?.limit !== undefined) params.set('limit', String(options.limit));

    const queryString = params.toString();
    const url = `${this.baseUrl}/api/v1/db/${encodeURIComponent(db)}/kv/${encodeURIComponent(keyspace)}${queryString ? '?' + queryString : ''}`;

    const response = await fetch(url);

    if (!response.ok) {
      throw await this.parseError(response);
    }

    return response.json();
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
   * @returns Array of type metadata
   */
  async listTypes(options?: OperationOptions): Promise<TypeMetadata[]> {
    const db = options?.database ?? this.defaultDatabase;

    const response = await fetch(
      `${this.baseUrl}/api/v1/db/${encodeURIComponent(db)}/types`
    );

    if (!response.ok) {
      throw await this.parseError(response);
    }

    return response.json();
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
}
