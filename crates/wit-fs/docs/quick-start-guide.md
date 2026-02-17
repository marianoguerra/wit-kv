# wit-fs in Five Minutes

This is a live walkthrough of wit-fs, a filesystem where every file has a type.

You define a type with WIT. You write values as plain text. The filesystem
validates everything on the spot, stores it as compact binary, and gives you
multiple views of the same data — all through ordinary Unix commands.

No database. No server. Just `mkdir`, `echo`, and `cat`.

## Setup

Two directories: one where the data actually lives on disk (the backing store),
and one where we mount the filesystem.

```bash
mkdir -p /tmp/demo-data /tmp/demo
```

Mount it:

```bash
wit-fs mount /tmp/demo-data /tmp/demo &
```

```
Mounting wit-fs at /tmp/demo
Backing store: /tmp/demo-data
Press Ctrl+C to unmount
```

That's it. We have a running filesystem. Let's use it.

## Define a Type

Every directory in wit-fs has a schema. You define it by writing a `.type.wit`
file. Let's create a keyspace for points:

```bash
mkdir /tmp/demo/points
```

Now give it a type — a record with two signed 32-bit integers:

```bash
cat > /tmp/demo/points/.type.wit << 'EOF'
package demo:geometry;
interface types {
    record point {
        x: s32,
        y: s32,
    }
}
EOF
```

From this moment on, everything written into `points/` must be a valid point.

## Write a Value

Write a point using WAVE text — the human-readable format for WIT values:

```bash
echo '{x: 10, y: 20}' > /tmp/demo/points/origin.wave
```

That's a validated, typed, binary-encoded value — created with `echo`.

## Read It Back

```bash
cat /tmp/demo/points/origin.wave
```

```
{x: 10, y: 20}
```

The filesystem decoded the stored binary back to text for you. What's actually
on disk? Let's look at the same value through the binary view:

```bash
xxd /tmp/demo/points/origin.witb
```

```
00000000: 0a00 0000 1400 0000                      ........
```

Eight bytes. Two 32-bit little-endian integers, `10` and `20`, packed in the
WebAssembly Canonical ABI format. Same value, two views — pick whichever you
need.

## Add More Values

```bash
echo '{x: 0, y: 0}' > /tmp/demo/points/center.wave
echo '{x: -5, y: 100}' > /tmp/demo/points/far.wave
```

List them:

```bash
ls -a /tmp/demo/points/
```

```
.type.wit       .type.error.wit
center.wave     center.witb
far.wave        far.witb
origin.wave     origin.witb
```

Every value shows up twice — `.wave` and `.witb`. The `.type.error.wit` was
auto-generated; it defines the shape of error diagnostics. We'll see that next.

## Validation: Try Writing Bad Data

Here's what makes this interesting. What if we write something that doesn't
match the schema?

```bash
echo '{x: "hello"}' > /tmp/demo/points/bad.wave
```

```
bash: echo: write error: Invalid argument
```

Rejected. The filesystem refused the write at the kernel level. But that error
message isn't very helpful. Let's check the error file:

```bash
cat /tmp/demo/points/bad.witerr
```

```
{message: "...", timestamp: "2026-02-17T15:30:00Z", input: "{x: \"hello\"}", error-kind: wave-parse}
```

The error itself is a WIT-typed value. It tells you exactly what went wrong,
when, and what input caused it. The error schema is right there too:

```bash
cat /tmp/demo/points/.type.error.wit
```

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

Errors are first-class typed data, not log messages.

## Fix the Error

Write a valid value to the same key and the error disappears:

```bash
echo '{x: 42, y: 0}' > /tmp/demo/points/bad.wave
ls /tmp/demo/points/bad.*
```

```
bad.wave    bad.witb
```

No more `.witerr`. Or if you just want to dismiss the error without fixing it:

```bash
rm /tmp/demo/points/bad.witerr
```

## Delete a Value

```bash
rm /tmp/demo/points/far.wave
ls /tmp/demo/points/far.*
```

```
ls: /tmp/demo/points/far.*: No such file or directory
```

Deleting either view (`.wave` or `.witb`) removes the value entirely.

## Extended Attributes: Quick Status Check

You can check whether a value is valid without reading files at all:

```bash
xattr -p user.wit-fs.valid /tmp/demo/points/origin.wave
```

```
true
```

Scriptable. No parsing required.

## Multiple Types: Not Just Records

Create a new directory with an enum type:

```bash
mkdir /tmp/demo/colors

cat > /tmp/demo/colors/.type.wit << 'EOF'
package demo:palette;
interface types {
    enum color {
        red,
        green,
        blue,
    }
}
EOF

echo 'red' > /tmp/demo/colors/primary.wave
cat /tmp/demo/colors/primary.wave
```

```
red
```

Or a variant type:

```bash
mkdir /tmp/demo/shapes

cat > /tmp/demo/shapes/.type.wit << 'EOF'
package demo:drawing;
interface types {
    variant shape {
        circle(u32),
        square(u32),
        empty,
    }
}
EOF

echo 'circle(42)' > /tmp/demo/shapes/logo.wave
cat /tmp/demo/shapes/logo.wave
```

```
circle(42)
```

Records, enums, variants, strings, nested types — anything you can express
in WIT, you can store and validate here.

## Using wit-file Alongside

The companion tool `wit-file` auto-discovers schemas in wit-fs directories:

```bash
# Read a binary file — wit-file finds the .type.wit automatically
wit-file read /tmp/demo/points/origin.witb
```

```
{x: 10, y: 20}
```

```bash
# Validate without writing
wit-file validate --value '{x: 1, y: 2}' --wit /tmp/demo/points/.type.wit
```

```
Valid
```

```bash
# Generate a binary file from WAVE text
wit-file write --value '{x: 99, y: -1}' -o /tmp/demo/points/new.witb
```

## Persistence

Everything survives unmount. Let's look at what's on the backing store:

```bash
ls /tmp/demo-data/points/
```

```
.type.wit    center.bin    origin.bin    new.bin
```

Plain files. The backing store holds the WIT schema and one `.bin` file per
value containing canonical ABI bytes. Mount the same backing dir again and
all your data is still there.

## Unmount

```bash
umount /tmp/demo
# or on macOS:
diskutil unmount /tmp/demo
```

## What Just Happened

In five minutes, we:

1. Mounted a typed filesystem with one command
2. Defined schemas using WIT — the WebAssembly Interface Type language
3. Wrote and read values as plain text using `echo` and `cat`
4. Got automatic binary encoding in the WebAssembly Canonical ABI
5. Had invalid writes rejected at the kernel level with typed error diagnostics
6. Saw error auto-cleanup, manual dismissal, and extended attributes
7. Used multiple WIT types: records, enums, and variants
8. Got full persistence across mount cycles

No client libraries. No serialization code. No migration scripts.
Just a filesystem that understands your types.
