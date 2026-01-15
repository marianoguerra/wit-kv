export { WitKvClient } from './client.js';
export { WitKvError } from './errors.js';
export type {
  ApiError,
  ContentFormat,
  DatabaseInfo,
  DatabaseList,
  DeleteTypeOptions,
  GetOptions,
  KeyFilter,
  KeyList,
  KeyspaceList,
  ListOptions,
  MapConfig,
  MapOptions,
  MapResult,
  OperationOptions,
  ReduceConfig,
  ReduceOptions,
  ReduceResult,
  SemanticVersion,
  SetTypeOptions,
  TypeMetadata,
} from './types.js';

// WIT-generated types (camelCase, matches kv.wit definitions)
export type {
  BinaryExport,
  DatabaseInfo as WitDatabaseInfo,
  DatabaseList as WitDatabaseList,
  KeyList as WitKeyList,
  KeyspaceList as WitKeyspaceList,
  KeyspaceMetadata,
  SemanticVersion as WitSemanticVersion,
  StoredValue,
} from './wit-types.js';
export { toKeyspaceMetadata } from './wit-types.js';
