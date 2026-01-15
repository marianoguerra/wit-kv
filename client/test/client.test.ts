import { describe, it, before, after } from 'node:test';
import assert from 'node:assert';
import { WitKvClient, WitKvError } from '../src/index.js';

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
});
