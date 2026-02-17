# wit-fs Design Document

## Overview

`wit-fs` is a FUSE filesystem that exposes WIT-typed values as files. Each logical value is accessible through multiple file extensions — human-readable WAVE text (`.wave`) and raw canonical ABI binary (`.witb`) — with sibling error files generated on validation failure.

The filesystem validates all writes against WIT type schemas, rejecting invalid data at the FUSE `flush()` boundary while providing detailed error diagnostics through sibling `.witerr` files.

## Architecture

### Crate Structure

```
crates/wit-fs/
  src/
    main.rs       CLI entry point (clap): mount command
    fs.rs         fuser::Filesystem trait implementation
    inode.rs      Inode table: maps inodes to entries
    store.rs      Backing storage: read/write .bin files + .type.wit
    schema.rs     WIT schema loading, parsing, caching
    validate.rs   WAVE/binary validation against WIT types
    error.rs      ValidationError type + WAVE/binary encoding
```

### Dependencies

- **fuser 0.17** — FUSE bindings (with `macfuse-4-compat` feature for macOS)
- **wit-core** — WIT type loading, WAVE encoding/decoding, canonical ABI
- **clap** — CLI argument parsing
- **thiserror** — Error type derivation
- **libc** — POSIX constants and uid/gid

### Reuse from Existing Crates

- `wit_core::load_wit_type_from_string()` — parse WIT definitions
- `wit_core::wave_from_str()` / `wave_to_string()` — WAVE encode/decode
- `wit_core::CanonicalAbi` — canonical ABI binary encoding
- `wit_core::LinearMemory` — memory management for variable-length types

## File Extension Convention

Each logical value `<name>` in a typed directory produces multiple virtual file views:

| Extension | Format | Access | Purpose |
|-----------|--------|--------|---------|
| `<name>.wave` | WAVE text | Read/Write | Human-readable value |
| `<name>.witb` | Canonical ABI binary | Read/Write | Binary value |
| `<name>.witerr` | WAVE text | Read-only | Validation error (exists only on error) |
| `<name>.witerrb` | Canonical ABI binary | Read-only | Validation error in binary |

All four extensions refer to the same underlying value. Writing to `origin.wave` or `origin.witb` updates the same stored data.

### Directory Layout

```
mountpoint/
  points/
    .type.wit              # schema: "record point { x: s32, y: s32 }"
    .type.error.wit        # auto-generated error schema (read-only)
    origin.wave            # "{x: 1, y: 2}"
    origin.witb            # <binary bytes>
    center.wave            # "{x: 0, y: 0}"
    center.witb            # <binary bytes>
```

## Schema Discovery

Each directory contains a `.type.wit` file defining the schema for all values in that directory. The type name defaults to the first named type in the WIT definition.

A companion `.type.error.wit` file is auto-generated (read-only) and defines the `validation-error` record type used by `.witerr`/`.witerrb` files:

```wit
package witfs:errors;

interface errors {
    record validation-error {
        message: string,
        timestamp: string,
        input: string,
        error-kind: error-kind,
    }

    enum error-kind {
        wave-parse,
        type-mismatch,
        schema-error,
        abi-error,
    }
}
```

## Error Reporting

### The Problem

FUSE `write()` can only return a byte count or a POSIX errno. When validation fails, the user sees only "Invalid argument".

### Solution: POSIX errno + Sibling Error Files

**Layer 1: POSIX errno from `flush()`**
- `EINVAL` — WAVE parse or type validation failure
- `EIO` — internal error (ABI encoding, schema loading)
- File content reverts to last valid state

**Layer 2: Sibling `.witerr` / `.witerrb` files**

On validation failure, the filesystem generates sibling error files containing a `validation-error` WIT-typed value. The error includes the full parser error message, a timestamp, the rejected input, and the error kind.

**Lifecycle:**
- Created when a write fails validation
- Removed automatically when a subsequent write succeeds
- Can be deleted manually to dismiss the error
- Always read-only (writes return `EACCES`)

**Layer 3: Extended attributes (xattrs)**

Quick status check without reading files:
- `user.wit-fs.valid` = `"true"` | `"false"`
- `user.wit-fs.error` = error message string

## FUSE Implementation

### Interior Mutability

fuser 0.17 requires `&self` (not `&mut self`) for all `Filesystem` trait methods except `init`. The implementation uses a `Mutex<Inner>` pattern where `WitFs` wraps a mutex-protected inner state.

### Write Buffering

```
open()     → allocate write buffer, set direct_io flag
write()    → append to buffer (always return full byte count)
flush()    → parse + validate complete content
               success → store data, remove errors, return Ok
               failure → generate error files, return EINVAL
release()  → free buffer
```

Writes are accepted optimistically during `write()` calls (WAVE text may arrive in multiple chunks). Validation happens on `flush()` when the complete document is available.

### Read Dispatch

```
read()     → .wave:    decode stored ABI binary → WAVE text
             .witb:    return raw stored ABI bytes
             .witerr:  return stored error as WAVE text
             .witerrb: return stored error as ABI bytes
```

### Inode Management

The inode table maps inode numbers to entries with extension awareness. File types:

- `RootDir` — the mount root
- `TypedDir` — a subdirectory with a WIT schema
- `SchemaFile` — `.type.wit` in a directory
- `ErrorSchemaFile` — `.type.error.wit` (auto-generated, read-only)
- `WaveFile` — `<name>.wave` view
- `WitbFile` — `<name>.witb` view
- `WiterrFile` — `<name>.witerr` error view
- `WiterrbFile` — `<name>.witerrb` error view

### `unlink()` Behavior

- Deleting `<name>.wave` or `<name>.witb` deletes the value (both views + any error files)
- Deleting `<name>.witerr` also deletes `<name>.witerrb` (paired error dismissal)
- Deleting `.type.wit` unsets the schema
- Deleting `.type.error.wit` returns `EACCES` (auto-generated)

## Backing Store

Canonical ABI binary is the source of truth, stored on a real backing directory:

```
backing/
  points/
    .type.wit          # WIT schema (persisted as-is)
    origin.bin         # Value bytes (buffer + memory concatenated)
    origin.err.bin     # Error binary (only if error exists)
    origin.err.wave    # Error WAVE text (only if error exists)
```

The backing store persists across unmount/remount cycles. WAVE text (`.wave`) and error text (`.witerr`) are generated on-the-fly from the stored binary data.

## Platform Notes

- **macOS**: Requires [macFUSE](https://osxfuse.github.io/). The `macfuse-4-compat` feature is enabled in the fuser dependency.
- **Linux**: Native FUSE support. No additional features needed.
- Extended attributes accessed via `xattr -p` (macOS) or `getfattr` (Linux).
