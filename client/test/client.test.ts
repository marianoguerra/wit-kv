import { describe, it, before, after } from 'node:test';
import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { WitKvClient, WitKvError } from '../src/index.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Path to example WASM modules (relative to project root)
const PROJECT_ROOT = path.resolve(__dirname, '../..');
const POINT_FILTER_WASM = path.join(
  PROJECT_ROOT,
  'examples/point-filter/target/wasm32-unknown-unknown/release/point_filter.wasm'
);
const SUM_SCORES_WASM = path.join(
  PROJECT_ROOT,
  'examples/sum-scores/target/wasm32-unknown-unknown/release/sum_scores.wasm'
);

const BASE_URL = process.env.WIT_KV_URL ?? 'http://localhost:8080';

describe('WitKvClient', () => {
  const client = new WitKvClient(BASE_URL);

  describe('health', () => {
    it('should return ok', async () => {
      const status = await client.health();
      assert.strictEqual(status, 'ok');
    });
  });

  describe('types', () => {
    const testKeyspace = 'test_points';
    const witDefinition = `package test:types;

interface types {
  record point {
    x: s32,
    y: s32,
  }
}
`;

    after(async () => {
      // Clean up
      try {
        await client.deleteType(testKeyspace, { deleteData: true });
      } catch {
        // Ignore errors
      }
    });

    it('should register a type', async () => {
      const metadata = await client.setType(testKeyspace, witDefinition, {
        typeName: 'point',
      });

      assert.strictEqual(metadata.name, testKeyspace);
      assert.strictEqual(metadata.type_name, 'point');
    });

    it('should get type metadata', async () => {
      const metadata = await client.getType(testKeyspace);

      assert.strictEqual(metadata.name, testKeyspace);
      assert.strictEqual(metadata.type_name, 'point');
    });

    it('should list types', async () => {
      const types = await client.listTypes();

      const found = types.find((t) => t.name === testKeyspace);
      assert.ok(found, 'Test keyspace should be in type list');
    });
  });

  describe('kv operations', () => {
    const testKeyspace = 'test_kv_points';
    const witDefinition = `package test:kv;

interface types {
  record point {
    x: s32,
    y: s32,
  }
}
`;

    before(async () => {
      // Set up type
      try {
        await client.setType(testKeyspace, witDefinition, {
          typeName: 'point',
          force: true,
        });
      } catch {
        // Ignore if exists
      }
    });

    after(async () => {
      // Clean up
      try {
        await client.deleteType(testKeyspace, { deleteData: true });
      } catch {
        // Ignore errors
      }
    });

    it('should set and get a value', async () => {
      const value = '{x: 10, y: 20}';
      await client.set(testKeyspace, 'p1', value);

      const retrieved = await client.get(testKeyspace, 'p1');
      assert.strictEqual(retrieved, value);
    });

    it('should list keys', async () => {
      await client.set(testKeyspace, 'p2', '{x: 30, y: 40}');
      await client.set(testKeyspace, 'p3', '{x: 50, y: 60}');

      const keys = await client.list(testKeyspace);
      assert.ok(keys.includes('p1'));
      assert.ok(keys.includes('p2'));
      assert.ok(keys.includes('p3'));
    });

    it('should list keys with prefix', async () => {
      const keys = await client.list(testKeyspace, { prefix: 'p' });
      assert.ok(keys.length >= 3);
    });

    it('should list keys with limit', async () => {
      const keys = await client.list(testKeyspace, { limit: 2 });
      assert.strictEqual(keys.length, 2);
    });

    it('should delete a value', async () => {
      await client.delete(testKeyspace, 'p1');

      try {
        await client.get(testKeyspace, 'p1');
        assert.fail('Should have thrown');
      } catch (err) {
        assert.ok(err instanceof WitKvError);
        assert.ok(err.isNotFound());
      }
    });

    it('should get binary format', async () => {
      await client.set(testKeyspace, 'binary_test', '{x: 100, y: 200}');

      const binary = await client.get(testKeyspace, 'binary_test', {
        format: 'binary',
      });

      assert.ok(binary instanceof ArrayBuffer);
      assert.ok(binary.byteLength > 0);
    });
  });

  describe('error handling', () => {
    it('should throw for non-existent keyspace', async () => {
      try {
        await client.get('nonexistent_keyspace', 'key');
        assert.fail('Should have thrown');
      } catch (err) {
        assert.ok(err instanceof WitKvError);
        assert.strictEqual(err.code, 'KEYSPACE_NOT_FOUND');
      }
    });

    it('should throw for non-existent key', async () => {
      // First create a keyspace
      const keyspace = 'error_test_keyspace';
      const witDef = `package test:err;
interface types {
  record dummy { value: u32 }
}
`;
      await client.setType(keyspace, witDef, { typeName: 'dummy', force: true });

      try {
        await client.get(keyspace, 'nonexistent_key');
        assert.fail('Should have thrown');
      } catch (err) {
        assert.ok(err instanceof WitKvError);
        assert.strictEqual(err.code, 'KEY_NOT_FOUND');
      } finally {
        await client.deleteType(keyspace, { deleteData: true });
      }
    });
  });

  describe('map operations', () => {
    const testKeyspace = 'test_map_points';
    const witDefinition = `package wit-kv:typed-map@0.1.0;

interface types {
  record point {
    x: s32,
    y: s32,
  }
}
`;

    before(async function () {
      // Skip if WASM module not built
      if (!fs.existsSync(POINT_FILTER_WASM)) {
        console.log('Skipping map tests: WASM module not built. Run "just build-examples" first.');
        this.skip();
        return;
      }

      // Set up type and data
      await client.setType(testKeyspace, witDefinition, {
        typeName: 'point',
        force: true,
      });

      // Add test data: points at various distances from origin
      // point-filter keeps points within radius 100 and doubles coordinates
      const points = [
        ['p1', '{x: 10, y: 20}'], // distance ~22, will be kept and doubled
        ['p2', '{x: 50, y: 50}'], // distance ~70, will be kept and doubled
        ['p3', '{x: 150, y: 0}'], // distance 150, will be filtered out
        ['p4', '{x: 3, y: 4}'], // distance 5, will be kept and doubled
      ];

      for (const [key, value] of points) {
        await client.set(testKeyspace, key, value);
      }
    });

    after(async () => {
      try {
        await client.deleteType(testKeyspace, { deleteData: true });
      } catch {
        // Ignore errors
      }
    });

    it('should execute map operation', async function () {
      if (!fs.existsSync(POINT_FILTER_WASM)) {
        this.skip();
        return;
      }

      const wasmBytes = fs.readFileSync(POINT_FILTER_WASM);

      const result = await client.map(testKeyspace, wasmBytes, witDefinition, 'point');

      // Should process all 4 points
      assert.strictEqual(result.processed, 4);

      // p1, p2, p4 are within radius 100 - should be transformed
      assert.strictEqual(result.transformed, 3);

      // p3 is at distance 150 - should be filtered
      assert.strictEqual(result.filtered, 1);

      // Check that results contain transformed points
      assert.ok(result.results.length > 0);

      // Find p4 result (was {x: 3, y: 4}, should be doubled to {x: 6, y: 8})
      const p4Result = result.results.find(([key]) => key === 'p4');
      assert.ok(p4Result, 'Should have result for p4');
      assert.strictEqual(p4Result[1], '{x: 6, y: 8}');
    });

    it('should execute map operation with filter', async function () {
      if (!fs.existsSync(POINT_FILTER_WASM)) {
        this.skip();
        return;
      }

      const wasmBytes = fs.readFileSync(POINT_FILTER_WASM);

      // Only process keys starting with 'p1' or 'p2'
      const result = await client.map(testKeyspace, wasmBytes, witDefinition, 'point', {
        filter: { prefix: 'p1' },
      });

      // Should only process p1
      assert.strictEqual(result.processed, 1);
      assert.strictEqual(result.transformed, 1);
    });
  });

  describe('reduce operations', () => {
    const testKeyspace = 'test_reduce_people';
    const witDefinition = `package wit-kv:typed-sum-scores@0.1.0;

interface types {
  record person {
    age: u8,
    score: u32,
  }

  record total {
    sum: u64,
    count: u32,
  }
}
`;

    before(async function () {
      // Skip if WASM module not built
      if (!fs.existsSync(SUM_SCORES_WASM)) {
        console.log(
          'Skipping reduce tests: WASM module not built. Run "just build-examples" first.'
        );
        this.skip();
        return;
      }

      // Set up type - use 'person' as the type since that's what we store
      await client.setType(testKeyspace, witDefinition, {
        typeName: 'person',
        force: true,
      });

      // Add test data
      const people = [
        ['alice', '{age: 25, score: 100}'],
        ['bob', '{age: 30, score: 200}'],
        ['charlie', '{age: 35, score: 150}'],
      ];

      for (const [key, value] of people) {
        await client.set(testKeyspace, key, value);
      }
    });

    after(async () => {
      try {
        await client.deleteType(testKeyspace, { deleteData: true });
      } catch {
        // Ignore errors
      }
    });

    it('should execute reduce operation', async function () {
      if (!fs.existsSync(SUM_SCORES_WASM)) {
        this.skip();
        return;
      }

      const wasmBytes = fs.readFileSync(SUM_SCORES_WASM);

      const result = await client.reduce(
        testKeyspace,
        wasmBytes,
        witDefinition,
        'person',
        'total'
      );

      // Should process all 3 people
      assert.strictEqual(result.processed, 3);
      assert.strictEqual(result.errorCount, 0);

      // Final state should be sum=450, count=3
      assert.strictEqual(result.state, '{sum: 450, count: 3}');
    });

    it('should execute reduce operation with filter', async function () {
      if (!fs.existsSync(SUM_SCORES_WASM)) {
        this.skip();
        return;
      }

      const wasmBytes = fs.readFileSync(SUM_SCORES_WASM);

      // Only process keys starting with 'a' (just alice)
      const result = await client.reduce(
        testKeyspace,
        wasmBytes,
        witDefinition,
        'person',
        'total',
        { filter: { prefix: 'a' } }
      );

      // Should only process alice
      assert.strictEqual(result.processed, 1);
      assert.strictEqual(result.state, '{sum: 100, count: 1}');
    });
  });
});
