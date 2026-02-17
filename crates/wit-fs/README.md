# wit-fs

A FUSE filesystem for WIT-typed files. Mount a directory where every file is validated against a WIT type schema, with human-readable WAVE text and raw binary views of the same data.

## Requirements

- **macOS**: Install [macFUSE](https://osxfuse.github.io/)
- **Linux**: Install `libfuse3-dev` (or equivalent)

## Installation

```bash
# From the workspace root
cargo build --release -p wit-fs
```

The binary is at `target/release/wit-fs`.

## Quick Start

```bash
# Create backing and mount directories
mkdir -p /tmp/wit-fs-backing /tmp/wit-fs-mount

# Mount the filesystem
wit-fs mount /tmp/wit-fs-backing /tmp/wit-fs-mount --foreground &

# Create a typed directory by writing a schema
mkdir /tmp/wit-fs-mount/points
cat > /tmp/wit-fs-mount/points/.type.wit << 'EOF'
package example:points;
interface types {
    record point {
        x: s32,
        y: s32,
    }
}
EOF

# Write a value as WAVE text
echo '{x: 10, y: 20}' > /tmp/wit-fs-mount/points/origin.wave

# Read it back
cat /tmp/wit-fs-mount/points/origin.wave
# Output: {x: 10, y: 20}

# Read the same value as binary
xxd /tmp/wit-fs-mount/points/origin.witb

# Unmount
umount /tmp/wit-fs-mount
```

## Usage

### Mount Command

```
wit-fs mount <backing-dir> <mountpoint> [OPTIONS]

Arguments:
  <backing-dir>   Path to the backing directory (stores binary data)
  <mountpoint>    Path to the mount point

Options:
  --read-only     Mount as read-only
  --foreground     Run in foreground (don't daemonize)
```

### File Extensions

Each value `<name>` in a typed directory appears as multiple files:

| File | Format | Access | Description |
|------|--------|--------|-------------|
| `<name>.wave` | WAVE text | R/W | Human-readable value |
| `<name>.witb` | Binary | R/W | Canonical ABI binary |
| `<name>.witerr` | WAVE text | Read-only | Validation error (if any) |
| `<name>.witerrb` | Binary | Read-only | Validation error binary |

All extensions are views of the same underlying value. Writing to `.wave` or `.witb` updates the same data.

### Schema Files

Each directory contains:

- `.type.wit` — The WIT type definition (writable). Defines the schema for all values in the directory.
- `.type.error.wit` — Auto-generated error schema (read-only). Defines the `validation-error` type used by `.witerr`/`.witerrb` files.

### Error Handling

When a write fails validation:

1. The `write()` system call returns `EINVAL`
2. The file reverts to its last valid content
3. Sibling `.witerr` and `.witerrb` files appear with the error details

```bash
# Write invalid data
$ echo '{x: "hello"}' > /tmp/wit-fs-mount/points/origin.wave
# bash: echo: write error: Invalid argument

# The value is unchanged
$ cat /tmp/wit-fs-mount/points/origin.wave
{x: 10, y: 20}

# Read the error
$ cat /tmp/wit-fs-mount/points/origin.witerr
{message: "...", timestamp: "2026-02-17T10:30:00Z", input: "{x: \"hello\"}", error-kind: wave-parse}

# Fix the error by writing valid data
$ echo '{x: 5, y: 15}' > /tmp/wit-fs-mount/points/origin.wave
# .witerr and .witerrb are automatically removed

# Or dismiss the error manually
$ rm /tmp/wit-fs-mount/points/origin.witerr
```

### Extended Attributes

Quick status check without reading files:

```bash
# macOS
xattr -p user.wit-fs.valid /tmp/wit-fs-mount/points/origin.wave
xattr -p user.wit-fs.error /tmp/wit-fs-mount/points/origin.wave

# Linux
getfattr -n user.wit-fs.valid /tmp/wit-fs-mount/points/origin.wave
```

### Using with wit-file

`wit-file` auto-discovers `.type.wit` in the same directory as the target file:

```bash
# Read a binary file using auto-discovered schema
wit-file read /tmp/wit-fs-mount/points/origin.witb

# Write via wit-file
wit-file write --value '{x: 42, y: 0}' -o /tmp/wit-fs-mount/points/origin.witb

# Validate without writing
wit-file validate --value '{x: 10, y: 20}' /tmp/wit-fs-mount/points/.type.wit
```

## Backing Store

Data is persisted as canonical ABI binary in the backing directory:

```
backing/
  points/
    .type.wit          # WIT schema
    origin.bin         # Binary value data
    origin.err.bin     # Error binary (only on validation failure)
    origin.err.wave    # Error WAVE text (only on validation failure)
```

The backing store survives unmount/remount cycles. WAVE text is generated on-the-fly from stored binary data.

## Deleting Values

- Delete `<name>.wave` or `<name>.witb` to remove the value entirely (both views + any error files)
- Delete `<name>.witerr` to dismiss the error (also removes `.witerrb`)
- Delete `.type.wit` to unset the directory's schema
- `.type.error.wit` cannot be deleted (auto-generated)
