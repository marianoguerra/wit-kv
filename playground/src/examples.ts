/**
 * Predefined examples for easy exploration
 */

export interface Example {
  name: string;
  description: string;
  keyspace: string;
  witDefinition: string;
  typeName: string;
  values: { key: string; value: string }[];
}

export interface ExampleCategory {
  name: string;
  examples: Example[];
}

export const exampleCategories: ExampleCategory[] = [
  {
    name: 'Basic Types',
    examples: [
      {
        name: 'Simple Record',
        description: 'Person with name and age',
        keyspace: 'people',
        witDefinition: `package example:types@0.1.0;

interface types {
  record person {
    name: string,
    age: u32,
    active: bool,
  }
}

world example {
  export types;
}`,
        typeName: 'person',
        values: [
          { key: 'alice', value: '{name: "Alice", age: 30, active: true}' },
          { key: 'bob', value: '{name: "Bob", age: 25, active: false}' },
          { key: 'charlie', value: '{name: "Charlie", age: 35, active: true}' },
        ],
      },
      {
        name: 'Point Coordinates',
        description: '2D point with float coordinates',
        keyspace: 'points',
        witDefinition: `package example:types@0.1.0;

interface types {
  record point {
    x: f64,
    y: f64,
  }
}

world example {
  export types;
}`,
        typeName: 'point',
        values: [
          { key: 'origin', value: '{x: 0.0, y: 0.0}' },
          { key: 'p1', value: '{x: 10.5, y: 20.3}' },
          { key: 'p2', value: '{x: -5.0, y: 15.7}' },
        ],
      },
      {
        name: 'Tuples',
        description: 'Various tuple types',
        keyspace: 'coordinates',
        witDefinition: `package example:types@0.1.0;

interface types {
  type coordinate = tuple<f64, f64, f64>;
}

world example {
  export types;
}`,
        typeName: 'coordinate',
        values: [
          { key: 'home', value: '(40.7128, -74.006, 10.0)' },
          { key: 'office', value: '(37.7749, -122.4194, 15.0)' },
        ],
      },
    ],
  },
  {
    name: 'Algebraic Types',
    examples: [
      {
        name: 'Enums',
        description: 'Color enumeration',
        keyspace: 'colors',
        witDefinition: `package example:types@0.1.0;

interface types {
  enum color {
    red,
    green,
    blue,
    yellow,
    cyan,
    magenta,
  }
}

world example {
  export types;
}`,
        typeName: 'color',
        values: [
          { key: 'primary1', value: 'red' },
          { key: 'primary2', value: 'green' },
          { key: 'primary3', value: 'blue' },
        ],
      },
      {
        name: 'Variants',
        description: 'Status with different payloads',
        keyspace: 'tasks',
        witDefinition: `package example:types@0.1.0;

interface types {
  variant status {
    pending,
    running(u32),
    complete(string),
    failed(string),
  }
}

world example {
  export types;
}`,
        typeName: 'status',
        values: [
          { key: 'task1', value: 'pending' },
          { key: 'task2', value: 'running(42)' },
          { key: 'task3', value: 'complete("All done!")' },
          { key: 'task4', value: 'failed("Connection timeout")' },
        ],
      },
      {
        name: 'Flags',
        description: 'Permission bitset',
        keyspace: 'permissions',
        witDefinition: `package example:types@0.1.0;

interface types {
  flags permissions {
    read,
    write,
    execute,
    delete,
  }
}

world example {
  export types;
}`,
        typeName: 'permissions',
        values: [
          { key: 'readonly', value: '{read}' },
          { key: 'readwrite', value: '{read, write}' },
          { key: 'admin', value: '{read, write, execute, delete}' },
          { key: 'none', value: '{}' },
        ],
      },
    ],
  },
  {
    name: 'Container Types',
    examples: [
      {
        name: 'Options',
        description: 'Optional values',
        keyspace: 'profiles',
        witDefinition: `package example:types@0.1.0;

interface types {
  record profile {
    username: string,
    email: option<string>,
    bio: option<string>,
    age: option<u32>,
  }
}

world example {
  export types;
}`,
        typeName: 'profile',
        values: [
          {
            key: 'complete',
            value:
              '{username: "alice", email: some("alice@example.com"), bio: some("Software developer"), age: some(30)}',
          },
          {
            key: 'minimal',
            value: '{username: "bob", email: none, bio: none, age: none}',
          },
          {
            key: 'partial',
            value:
              '{username: "charlie", email: some("charlie@example.com"), bio: none, age: some(25)}',
          },
        ],
      },
      {
        name: 'Results',
        description: 'Success or error values',
        keyspace: 'operations',
        witDefinition: `package example:types@0.1.0;

interface types {
  type operation-result = result<u32, string>;
}

world example {
  export types;
}`,
        typeName: 'operation-result',
        values: [
          { key: 'success1', value: 'ok(42)' },
          { key: 'success2', value: 'ok(100)' },
          { key: 'error1', value: 'err("Not found")' },
          { key: 'error2', value: 'err("Permission denied")' },
        ],
      },
      {
        name: 'Lists',
        description: 'Arrays of values',
        keyspace: 'collections',
        witDefinition: `package example:types@0.1.0;

interface types {
  type numbers = list<u32>;
}

world example {
  export types;
}`,
        typeName: 'numbers',
        values: [
          { key: 'fibonacci', value: '[1, 1, 2, 3, 5, 8, 13, 21]' },
          { key: 'primes', value: '[2, 3, 5, 7, 11, 13, 17, 19]' },
          { key: 'empty', value: '[]' },
        ],
      },
    ],
  },
  {
    name: 'Complex Types',
    examples: [
      {
        name: 'Nested Records',
        description: 'Records containing other records',
        keyspace: 'contacts',
        witDefinition: `package example:types@0.1.0;

interface types {
  record address {
    street: string,
    city: string,
    zip: string,
  }

  record contact-info {
    email: string,
    phone: option<string>,
  }

  record contact {
    name: string,
    address: address,
    contact: contact-info,
  }
}

world example {
  export types;
}`,
        typeName: 'contact',
        values: [
          {
            key: 'john',
            value:
              '{name: "John Doe", address: {street: "123 Main St", city: "Springfield", zip: "12345"}, contact: {email: "john@example.com", phone: some("555-1234")}}',
          },
          {
            key: 'jane',
            value:
              '{name: "Jane Smith", address: {street: "456 Oak Ave", city: "Portland", zip: "97201"}, contact: {email: "jane@example.com", phone: none}}',
          },
        ],
      },
      {
        name: 'User System',
        description: 'Comprehensive user type with multiple features',
        keyspace: 'users',
        witDefinition: `package example:types@0.1.0;

interface types {
  enum role {
    guest,
    user,
    moderator,
    admin,
  }

  flags capabilities {
    read,
    write,
    delete,
    manage-users,
  }

  variant auth-status {
    anonymous,
    authenticated(string),
    expired,
    banned(string),
  }

  record profile {
    display-name: string,
    bio: option<string>,
  }

  record user {
    id: u64,
    username: string,
    role: role,
    capabilities: capabilities,
    profile: profile,
    auth: auth-status,
    tags: list<string>,
  }
}

world example {
  export types;
}`,
        typeName: 'user',
        values: [
          {
            key: 'admin-user',
            value:
              '{id: 1, username: "admin", role: admin, capabilities: {read, write, delete, manage-users}, profile: {display-name: "Administrator", bio: some("System administrator")}, auth: authenticated("session_abc123"), tags: ["staff", "verified"]}',
          },
          {
            key: 'regular-user',
            value:
              '{id: 42, username: "alice", role: user, capabilities: {read, write}, profile: {display-name: "Alice", bio: none}, auth: authenticated("session_xyz789"), tags: ["verified"]}',
          },
          {
            key: 'guest',
            value:
              '{id: 0, username: "guest", role: guest, capabilities: {read}, profile: {display-name: "Guest User", bio: none}, auth: anonymous, tags: []}',
          },
        ],
      },
      {
        name: 'Shopping Cart',
        description: 'List of items with nested data',
        keyspace: 'carts',
        witDefinition: `package example:types@0.1.0;

interface types {
  record item {
    id: string,
    name: string,
    price: f64,
    quantity: u32,
  }

  record cart {
    user-id: string,
    items: list<item>,
    discount: option<f64>,
  }
}

world example {
  export types;
}`,
        typeName: 'cart',
        values: [
          {
            key: 'cart-alice',
            value:
              '{user-id: "alice", items: [{id: "SKU001", name: "Widget", price: 19.99, quantity: 2}, {id: "SKU002", name: "Gadget", price: 49.99, quantity: 1}], discount: some(10.0)}',
          },
          {
            key: 'cart-empty',
            value: '{user-id: "bob", items: [], discount: none}',
          },
        ],
      },
    ],
  },
];

/**
 * Get all examples as a flat list
 */
export function getAllExamples(): Example[] {
  return exampleCategories.flatMap((cat) => cat.examples);
}

/**
 * Find an example by keyspace name
 */
export function findExampleByKeyspace(keyspace: string): Example | undefined {
  return getAllExamples().find((ex) => ex.keyspace === keyspace);
}
