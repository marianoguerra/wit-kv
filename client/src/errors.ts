/**
 * Error thrown by wit-kv client operations.
 */
export class WitKvError extends Error {
  /**
   * Error code from the server.
   */
  readonly code: string;

  /**
   * HTTP status code.
   */
  readonly status: number;

  /**
   * Additional error details.
   */
  readonly details?: Record<string, unknown>;

  constructor(
    code: string,
    message: string,
    status: number,
    details?: Record<string, unknown>
  ) {
    super(message);
    this.name = 'WitKvError';
    this.code = code;
    this.status = status;
    this.details = details;
  }

  /**
   * Check if this is a "not found" error.
   */
  isNotFound(): boolean {
    return (
      this.code === 'DATABASE_NOT_FOUND' ||
      this.code === 'KEYSPACE_NOT_FOUND' ||
      this.code === 'KEY_NOT_FOUND' ||
      this.code === 'TYPE_NOT_FOUND'
    );
  }

  /**
   * Check if this is a conflict error.
   */
  isConflict(): boolean {
    return (
      this.code === 'KEYSPACE_EXISTS' || this.code === 'TYPE_VERSION_MISMATCH'
    );
  }
}
