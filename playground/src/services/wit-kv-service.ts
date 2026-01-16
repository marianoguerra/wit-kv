import { WitKvClient, type TypeMetadata } from 'wit-kv-client';

export interface KeyspaceInfo {
  name: string;
  typeName: string;
  version: string;
  witDefinition?: string;
}

/**
 * Service for interacting with wit-kv server
 * Uses the same origin (no CORS needed)
 */
export class WitKvService {
  private client: WitKvClient;
  private database: string;

  // Cache type info per keyspace
  private typeCache = new Map<string, TypeMetadata>();

  constructor(database = 'default') {
    // Use same origin - no need to specify server URL
    this.client = new WitKvClient('', database);
    this.database = database;
  }

  getDatabase(): string {
    return this.database;
  }

  setDatabase(database: string): void {
    this.database = database;
    this.client = new WitKvClient('', database);
    this.typeCache.clear();
  }

  /**
   * Check server health
   */
  async health(): Promise<string> {
    return this.client.health();
  }

  /**
   * List all databases
   */
  async listDatabases(): Promise<string[]> {
    // We're using wave format (default), so result is string[]
    return (await this.client.listDatabases()) as string[];
  }

  /**
   * List all keyspaces (types) in current database
   */
  async listKeyspaces(): Promise<KeyspaceInfo[]> {
    // We're using wave format (default), so result is TypeMetadata[]
    const result = await this.client.listTypes();
    if (result instanceof ArrayBuffer) {
      throw new Error('Unexpected binary response');
    }
    return result.map((t) => ({
      name: t.name,
      typeName: t.type_name,
      version: `${t.type_version.major}.${t.type_version.minor}.${t.type_version.patch}`,
    }));
  }

  /**
   * List keys in a keyspace
   */
  async listKeys(
    keyspace: string,
    options?: { prefix?: string; limit?: number }
  ): Promise<string[]> {
    // We're using wave format (default), so result is string[]
    return (await this.client.list(keyspace, options)) as string[];
  }

  /**
   * Get type metadata for a keyspace (cached)
   */
  async getTypeInfo(keyspace: string): Promise<TypeMetadata> {
    const cached = this.typeCache.get(keyspace);
    if (cached) return cached;

    const info = await this.client.getType(keyspace);
    this.typeCache.set(keyspace, info);
    return info;
  }

  /**
   * Get a value in WAVE text format
   */
  async getValue(keyspace: string, key: string): Promise<string> {
    return (await this.client.get(keyspace, key, { format: 'wave' })) as string;
  }

  /**
   * Get a value in binary format
   */
  async getValueBinary(keyspace: string, key: string): Promise<ArrayBuffer> {
    return (await this.client.get(keyspace, key, {
      format: 'binary',
    })) as ArrayBuffer;
  }

  /**
   * Set a value (WAVE text format)
   */
  async setValue(keyspace: string, key: string, value: string): Promise<void> {
    await this.client.set(keyspace, key, value);
  }

  /**
   * Delete a value
   */
  async deleteValue(keyspace: string, key: string): Promise<void> {
    await this.client.delete(keyspace, key);
  }

  /**
   * Register a type for a keyspace
   */
  async setType(
    keyspace: string,
    witDefinition: string,
    typeName?: string
  ): Promise<TypeMetadata> {
    const result = await this.client.setType(keyspace, witDefinition, {
      typeName,
    });
    this.typeCache.delete(keyspace);
    return result;
  }

  /**
   * Delete a type (and optionally its data)
   */
  async deleteType(keyspace: string, deleteData = false): Promise<void> {
    await this.client.deleteType(keyspace, { deleteData });
    this.typeCache.delete(keyspace);
  }

  /**
   * Clear the type cache
   */
  clearTypeCache(): void {
    this.typeCache.clear();
  }
}

// Singleton instance
export const witKvService = new WitKvService();
