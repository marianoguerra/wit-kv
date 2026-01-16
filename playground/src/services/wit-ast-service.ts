// Types from wit-ast bindings
export interface BinaryExport {
  value: Uint8Array;
  memory?: Uint8Array;
}

export interface ValueTree {
  nodes: WitValueNode[];
}

export type WitValueNode =
  | { tag: 'primitive'; val: PrimitiveValue }
  | { tag: 'record-val'; val: FieldRef[] }
  | { tag: 'tuple-val'; val: Uint32Array }
  | { tag: 'list-val'; val: Uint32Array }
  | { tag: 'enum-val'; val: string }
  | { tag: 'variant-val'; val: VariantRef }
  | { tag: 'option-val'; val: number | undefined }
  | { tag: 'result-val'; val: { tag: 'ok' | 'err'; val: number | undefined } }
  | { tag: 'flags-val'; val: string[] };

export type PrimitiveValue =
  | { tag: 'bool-val'; val: boolean }
  | { tag: 'u8-val'; val: number }
  | { tag: 'u16-val'; val: number }
  | { tag: 'u32-val'; val: number }
  | { tag: 'u64-val'; val: bigint }
  | { tag: 's8-val'; val: number }
  | { tag: 's16-val'; val: number }
  | { tag: 's32-val'; val: number }
  | { tag: 's64-val'; val: bigint }
  | { tag: 'f32-val'; val: number }
  | { tag: 'f64-val'; val: number }
  | { tag: 'char-val'; val: string }
  | { tag: 'string-val'; val: string };

export interface FieldRef {
  name: string;
  valueIdx: number;
}

export interface VariantRef {
  name: string;
  payloadIdx?: number;
}

export interface TypeDef {
  name: string;
  kind: TypeDefKind;
}

export type TypeDefKind =
  | { tag: 'type-alias'; val: TypeRef }
  | { tag: 'type-record'; val: TypeField[] }
  | { tag: 'type-tuple'; val: TypeRef[] }
  | { tag: 'type-flags'; val: string[] }
  | { tag: 'type-enum'; val: string[] }
  | { tag: 'type-variant'; val: TypeCase[] }
  | { tag: 'type-option'; val: TypeRef }
  | { tag: 'type-result'; val: [TypeRef | undefined, TypeRef | undefined] }
  | { tag: 'type-list'; val: TypeRef };

export type TypeRef =
  | { tag: 'primitive'; val: string }
  | { tag: 'defined'; val: number };

export interface TypeField {
  name: string;
  ty: TypeRef;
}

export interface TypeCase {
  name: string;
  ty?: TypeRef;
}

// WitAst resource interface
export interface WitAst {
  types(): TypeDef[];
  findType(name: string): number | undefined;
}

// Module interfaces
interface ParserModule {
  parseWit(definition: string): WitAst;
}

interface LifterModule {
  lift(ast: WitAst, typeName: string, data: BinaryExport): ValueTree;
}

interface FormatterModule {
  valueTreeToWave(ast: WitAst, typeName: string, value: ValueTree): string;
  waveToValueTree(ast: WitAst, typeName: string, waveText: string): ValueTree;
}

/**
 * Service for working with wit-ast WASM component
 */
export class WitAstService {
  private parser: ParserModule | null = null;
  private lifter: LifterModule | null = null;
  private formatter: FormatterModule | null = null;
  private loaded = false;
  private loadPromise: Promise<void> | null = null;

  // Cache parsed ASTs by WIT definition
  private astCache = new Map<string, WitAst>();

  async load(): Promise<void> {
    if (this.loaded) return;
    if (this.loadPromise) return this.loadPromise;

    this.loadPromise = this.doLoad();
    return this.loadPromise;
  }

  private async doLoad(): Promise<void> {
    try {
      // Dynamic import for WASM module from src/lib
      const module = await import('../lib/witast/witast.js');
      this.parser = module.parser as ParserModule;
      this.lifter = module.lifter as LifterModule;
      this.formatter = module.formatter as FormatterModule;
      this.loaded = true;
    } catch (err) {
      console.error('Failed to load wit-ast module:', err);
      throw new Error('Failed to load wit-ast WASM module. Make sure witast bindings are available.');
    }
  }

  isLoaded(): boolean {
    return this.loaded;
  }

  /**
   * Parse a WIT definition into an AST (cached)
   */
  parseWit(definition: string): WitAst {
    if (!this.parser) {
      throw new Error('wit-ast not loaded');
    }

    // Check cache
    const cached = this.astCache.get(definition);
    if (cached) return cached;

    // Parse and cache
    const ast = this.parser.parseWit(definition);
    this.astCache.set(definition, ast);
    return ast;
  }

  /**
   * Lift binary data to a value tree
   */
  lift(ast: WitAst, typeName: string, data: BinaryExport): ValueTree {
    if (!this.lifter) {
      throw new Error('wit-ast not loaded');
    }
    return this.lifter.lift(ast, typeName, data);
  }

  /**
   * Convert a value tree to WAVE text format
   */
  valueTreeToWave(ast: WitAst, typeName: string, value: ValueTree): string {
    if (!this.formatter) {
      throw new Error('wit-ast not loaded');
    }
    return this.formatter.valueTreeToWave(ast, typeName, value);
  }

  /**
   * Parse WAVE text to a value tree
   */
  waveToValueTree(ast: WitAst, typeName: string, waveText: string): ValueTree {
    if (!this.formatter) {
      throw new Error('wit-ast not loaded');
    }
    return this.formatter.waveToValueTree(ast, typeName, waveText);
  }

  /**
   * Get type definitions from an AST
   */
  getTypes(ast: WitAst): TypeDef[] {
    return ast.types();
  }

  /**
   * Clear the AST cache
   */
  clearCache(): void {
    this.astCache.clear();
  }
}

// Singleton instance
export const witAstService = new WitAstService();
