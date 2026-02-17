use std::collections::HashMap;
use std::ffi::OsStr;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use fuser::{
    Errno, FileAttr, FileHandle, FileType, Filesystem, FopenFlags, Generation, INodeNo,
    KernelConfig, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry,
    ReplyOpen, ReplyWrite, ReplyXattr, Request,
};

use super::error::{ErrorKind, ValidationError};
use super::inode::{FilenameParts, InodeKind, InodeTable, ValueExt, ROOT_INO, parse_filename};
use super::schema::SchemaCache;
use super::store::Store;
use super::validate::{validate_binary, validate_wave};

const TTL: Duration = Duration::from_secs(1);
const BLOCK_SIZE: u32 = 512;
const GEN: Generation = Generation(0);

const ENOENT: Errno = Errno::ENOENT;
const EACCES: Errno = Errno::EACCES;
const EINVAL: Errno = Errno::EINVAL;
const EIO: Errno = Errno::EIO;
const ENOTDIR: Errno = Errno::ENOTDIR;
const ENOTEMPTY: Errno = Errno::ENOTEMPTY;
const EEXIST: Errno = Errno::EEXIST;
const ERANGE: Errno = Errno::ERANGE;

/// Interior state protected by a mutex.
struct Inner {
    store: Store,
    inodes: InodeTable,
    schemas: SchemaCache,
    write_buffers: HashMap<u64, WriteBuffer>,
    next_fh: u64,
    read_only: bool,
    creation_time: SystemTime,
    uid: u32,
    gid: u32,
}

struct WriteBuffer {
    #[allow(dead_code)]
    ino: u64,
    data: Vec<u8>,
}

/// The WIT-FS FUSE filesystem.
pub struct WitFs {
    inner: Mutex<Inner>,
}

impl Inner {
    fn alloc_fh(&mut self) -> FileHandle {
        let fh = self.next_fh;
        self.next_fh += 1;
        FileHandle(fh)
    }

    fn make_dir_attr(&self, ino: u64) -> FileAttr {
        FileAttr {
            ino: INodeNo(ino),
            size: 0,
            blocks: 0,
            atime: self.creation_time,
            mtime: self.creation_time,
            ctime: self.creation_time,
            crtime: self.creation_time,
            kind: FileType::Directory,
            perm: if self.read_only { 0o555 } else { 0o755 },
            nlink: 2,
            uid: self.uid,
            gid: self.gid,
            rdev: 0,
            blksize: BLOCK_SIZE,
            flags: 0,
        }
    }

    fn make_file_attr(&self, ino: u64, size: u64, read_only: bool) -> FileAttr {
        FileAttr {
            ino: INodeNo(ino),
            size,
            blocks: size.div_ceil(u64::from(BLOCK_SIZE)),
            atime: self.creation_time,
            mtime: self.creation_time,
            ctime: self.creation_time,
            crtime: self.creation_time,
            kind: FileType::RegularFile,
            perm: if self.read_only || read_only {
                0o444
            } else {
                0o644
            },
            nlink: 1,
            uid: self.uid,
            gid: self.gid,
            rdev: 0,
            blksize: BLOCK_SIZE,
            flags: 0,
        }
    }

    fn file_content(&self, kind: &InodeKind) -> Option<Vec<u8>> {
        match kind {
            InodeKind::SchemaFile { dir_name } => self
                .schemas
                .get_schema(dir_name)
                .map(|s| s.wit_content.as_bytes().to_vec()),
            InodeKind::ErrorSchemaFile { .. } => {
                Some(self.schemas.error_wit_content().as_bytes().to_vec())
            }
            InodeKind::WaveFile {
                dir_name,
                value_name,
            } => {
                let binary = self.store.read_value(dir_name, value_name).ok()??;
                let schema = self.schemas.get_schema(dir_name)?;
                let wave_text = wit_core::binary_to_wave(&binary, &schema.resolved).ok()?;
                Some(wave_text.into_bytes())
            }
            InodeKind::WitbFile {
                dir_name,
                value_name,
            } => self.store.read_value(dir_name, value_name).ok()?,
            InodeKind::WiterrFile {
                dir_name,
                value_name,
            } => self
                .store
                .read_error_wave(dir_name, value_name)
                .ok()?
                .map(|s| s.into_bytes()),
            InodeKind::WiterrbFile {
                dir_name,
                value_name,
            } => self.store.read_error(dir_name, value_name).ok()?,
            _ => None,
        }
    }

    fn file_size(&self, kind: &InodeKind) -> u64 {
        self.file_content(kind)
            .map(|c| c.len() as u64)
            .unwrap_or(0)
    }

    fn handle_successful_write(
        &mut self,
        dir_name: &str,
        value_name: &str,
        binary: Vec<u8>,
    ) -> Result<(), Errno> {
        self.store
            .write_value(dir_name, value_name, &binary)
            .map_err(|_| EIO)?;

        let dir_ino = self.inodes.get_dir_ino(dir_name).ok_or(EIO)?;
        if self.inodes.has_error_files(dir_ino, value_name) {
            self.inodes.remove_error_files(dir_ino, value_name);
            let _ = self.store.remove_error(dir_name, value_name);
        }

        self.inodes.add_value(dir_name, dir_ino, value_name);
        Ok(())
    }

    fn handle_failed_write(
        &mut self,
        dir_name: &str,
        value_name: &str,
        error: ValidationError,
    ) -> Errno {
        tracing::warn!(
            dir = %dir_name,
            value = %value_name,
            kind = ?error.error_kind,
            "Validation failed: {}",
            error.message,
        );
        let dir_ino = match self.inodes.get_dir_ino(dir_name) {
            Some(ino) => ino,
            None => return EIO,
        };

        let wave_err = error.to_wave_string();
        let _ = self.store.write_error_wave(dir_name, value_name, &wave_err);

        if let Some(err_bin) = error.to_binary() {
            let _ = self.store.write_error(dir_name, value_name, &err_bin);
        }

        self.inodes
            .add_error_files(dir_name, dir_ino, value_name);

        EINVAL
    }
}

impl WitFs {
    pub fn new(store: Store, read_only: bool, uid: u32, gid: u32) -> Self {
        let mut inner = Inner {
            store,
            inodes: InodeTable::new(),
            schemas: SchemaCache::new(),
            write_buffers: HashMap::new(),
            next_fh: 1,
            read_only,
            creation_time: SystemTime::now(),
            uid,
            gid,
        };

        if let Err(e) = load_from_store(&mut inner) {
            tracing::warn!("Failed to load backing store: {e}");
        }

        Self {
            inner: Mutex::new(inner),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn load_from_store(inner: &mut Inner) -> Result<(), super::error::Error> {
    let dirs = inner.store.list_dirs()?;
    tracing::info!(count = dirs.len(), "Loading directories from backing store");
    for dir_name in &dirs {
        let dir_ino = inner.inodes.add_dir(dir_name);

        if let Some(wit_content) = inner.store.read_schema(dir_name)?
            && inner.schemas.set_schema(dir_name, &wit_content).is_ok()
        {
            inner.inodes.add_schema_files(dir_name, dir_ino);
        }

        let values = inner.store.list_values(dir_name)?;
        for value_name in &values {
            inner.inodes.add_value(dir_name, dir_ino, value_name);
            if inner.store.has_error(dir_name, value_name) {
                inner
                    .inodes
                    .add_error_files(dir_name, dir_ino, value_name);
            }
        }
    }
    Ok(())
}

impl Filesystem for WitFs {
    fn init(
        &mut self,
        _req: &Request,
        _config: &mut KernelConfig,
    ) -> Result<(), std::io::Error> {
        Ok(())
    }

    fn lookup(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
        let s = self.lock();
        match s.inodes.lookup(parent.0, name) {
            Some(ino) => {
                let entry = match s.inodes.get(ino) {
                    Some(e) => e.clone(),
                    None => {
                        reply.error(ENOENT);
                        return;
                    }
                };
                let attr = if entry.kind.is_dir() {
                    s.make_dir_attr(ino)
                } else {
                    let size = s.file_size(&entry.kind);
                    s.make_file_attr(ino, size, entry.kind.is_read_only())
                };
                reply.entry(&TTL, &attr, GEN);
            }
            None => {
                reply.error(ENOENT);
            }
        }
    }

    fn getattr(&self, _req: &Request, ino: INodeNo, _fh: Option<FileHandle>, reply: ReplyAttr) {
        let s = self.lock();
        let entry = match s.inodes.get(ino.0) {
            Some(e) => e.clone(),
            None => {
                reply.error(ENOENT);
                return;
            }
        };

        let attr = if entry.kind.is_dir() {
            s.make_dir_attr(ino.0)
        } else {
            let size = s.file_size(&entry.kind);
            s.make_file_attr(ino.0, size, entry.kind.is_read_only())
        };
        reply.attr(&TTL, &attr);
    }

    fn setattr(
        &self,
        _req: &Request,
        ino: INodeNo,
        _mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        size: Option<u64>,
        _atime: Option<fuser::TimeOrNow>,
        _mtime: Option<fuser::TimeOrNow>,
        _ctime: Option<SystemTime>,
        _fh: Option<FileHandle>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<fuser::BsdFileFlags>,
        reply: ReplyAttr,
    ) {
        let s = self.lock();
        let ino = ino.0;
        let entry = match s.inodes.get(ino) {
            Some(e) => e.clone(),
            None => {
                reply.error(ENOENT);
                return;
            }
        };

        if let Some(0) = size
            && !entry.kind.is_read_only()
            && !entry.kind.is_dir()
        {
            let attr = s.make_file_attr(ino, 0, entry.kind.is_read_only());
            reply.attr(&TTL, &attr);
            return;
        }

        let attr = if entry.kind.is_dir() {
            s.make_dir_attr(ino)
        } else {
            let file_size = s.file_size(&entry.kind);
            s.make_file_attr(ino, file_size, entry.kind.is_read_only())
        };
        reply.attr(&TTL, &attr);
    }

    fn readdir(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        mut reply: ReplyDirectory,
    ) {
        let s = self.lock();
        let ino = ino.0;
        let entry = match s.inodes.get(ino) {
            Some(e) => e.clone(),
            None => {
                reply.error(ENOENT);
                return;
            }
        };

        if !entry.kind.is_dir() {
            reply.error(ENOTDIR);
            return;
        }

        let mut entries: Vec<(u64, FileType, String)> = vec![
            (ino, FileType::Directory, ".".to_string()),
            (entry.parent, FileType::Directory, "..".to_string()),
        ];

        let children = s.inodes.children(ino);
        for child in children {
            if let Some(name) = child.kind.file_name() {
                let ft = if child.kind.is_dir() {
                    FileType::Directory
                } else {
                    FileType::RegularFile
                };
                entries.push((child.ino, ft, name));
            }
        }

        for (i, (child_ino, ft, name)) in entries.iter().enumerate().skip(offset as usize) {
            if reply.add(INodeNo(*child_ino), (i + 1) as u64, *ft, name) {
                break;
            }
        }
        reply.ok();
    }

    fn open(&self, _req: &Request, ino: INodeNo, flags: fuser::OpenFlags, reply: ReplyOpen) {
        let mut s = self.lock();
        let ino = ino.0;
        let entry = match s.inodes.get(ino) {
            Some(e) => e.clone(),
            None => {
                reply.error(ENOENT);
                return;
            }
        };

        if entry.kind.is_dir() {
            reply.error(ENOENT);
            return;
        }

        let fh = s.alloc_fh();

        // Check if the open flags indicate writing
        let is_writing = (flags.0 & libc::O_ACCMODE) != libc::O_RDONLY;
        if is_writing && !entry.kind.is_read_only() && !s.read_only {
            s.write_buffers.insert(
                fh.0,
                WriteBuffer {
                    ino,
                    data: Vec::new(),
                },
            );
        }

        reply.opened(fh, FopenFlags::FOPEN_DIRECT_IO);
    }

    fn read(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        size: u32,
        _flags: fuser::OpenFlags,
        _lock_owner: Option<fuser::LockOwner>,
        reply: ReplyData,
    ) {
        let s = self.lock();
        let entry = match s.inodes.get(ino.0) {
            Some(e) => e.clone(),
            None => {
                reply.error(ENOENT);
                return;
            }
        };

        let content = match s.file_content(&entry.kind) {
            Some(c) => c,
            None => {
                reply.error(EIO);
                return;
            }
        };

        let offset = offset as usize;
        if offset >= content.len() {
            reply.data(&[]);
        } else {
            let end = std::cmp::min(offset + size as usize, content.len());
            reply.data(content.get(offset..end).unwrap_or_default());
        }
    }

    fn write(
        &self,
        _req: &Request,
        ino: INodeNo,
        fh: FileHandle,
        offset: u64,
        data: &[u8],
        _write_flags: fuser::WriteFlags,
        _flags: fuser::OpenFlags,
        _lock_owner: Option<fuser::LockOwner>,
        reply: ReplyWrite,
    ) {
        let mut s = self.lock();
        if s.read_only {
            reply.error(EACCES);
            return;
        }

        let entry = match s.inodes.get(ino.0) {
            Some(e) => e.clone(),
            None => {
                reply.error(ENOENT);
                return;
            }
        };

        if entry.kind.is_read_only() {
            reply.error(EACCES);
            return;
        }

        if let Some(buffer) = s.write_buffers.get_mut(&fh.0) {
            let offset = offset as usize;
            let end = offset + data.len();
            if end > buffer.data.len() {
                buffer.data.resize(end, 0);
            }
            if let Some(dest) = buffer.data.get_mut(offset..end) {
                dest.copy_from_slice(data);
            }
            reply.written(data.len() as u32);
        } else {
            reply.error(EIO);
        }
    }

    fn flush(
        &self,
        _req: &Request,
        ino: INodeNo,
        fh: FileHandle,
        _lock_owner: fuser::LockOwner,
        reply: ReplyEmpty,
    ) {
        let mut s = self.lock();

        let buffer = match s.write_buffers.get(&fh.0) {
            Some(b) => b.data.clone(),
            None => {
                reply.ok();
                return;
            }
        };

        if buffer.is_empty() {
            reply.ok();
            return;
        }

        let entry = match s.inodes.get(ino.0) {
            Some(e) => e.clone(),
            None => {
                reply.error(EIO);
                return;
            }
        };

        match &entry.kind {
            InodeKind::SchemaFile { dir_name } => {
                let content = match std::str::from_utf8(&buffer) {
                    Ok(val) => val.to_string(),
                    Err(_) => {
                        reply.error(EINVAL);
                        return;
                    }
                };

                let dir_name = dir_name.clone();

                if let Err(e) = s.schemas.set_schema(&dir_name, &content) {
                    tracing::warn!(dir = %dir_name, "Schema parse error: {e}");
                    reply.error(EINVAL);
                    return;
                }

                if s.store.write_schema(&dir_name, &content).is_err() {
                    reply.error(EIO);
                    return;
                }

                let dir_ino = match s.inodes.get_dir_ino(&dir_name) {
                    Some(ino) => ino,
                    None => {
                        reply.error(EIO);
                        return;
                    }
                };
                s.inodes.add_schema_files(&dir_name, dir_ino);
                tracing::info!(dir = %dir_name, "Schema updated");
                reply.ok();
            }
            InodeKind::WaveFile {
                dir_name,
                value_name,
            } => {
                let dir_name = dir_name.clone();
                let value_name = value_name.clone();

                let wave_text = match std::str::from_utf8(&buffer) {
                    Ok(val) => val.to_string(),
                    Err(_) => {
                        let err = ValidationError::new(
                            "Invalid UTF-8 in WAVE text".to_string(),
                            format!("<{} bytes of binary data>", buffer.len()),
                            ErrorKind::WaveParse,
                        );
                        let errno = s.handle_failed_write(&dir_name, &value_name, err);
                        reply.error(errno);
                        return;
                    }
                };

                let schema = match s.schemas.get_schema(&dir_name) {
                    Some(schema) => schema,
                    None => {
                        reply.error(EINVAL);
                        return;
                    }
                };

                match validate_wave(&wave_text, schema) {
                    Ok(binary) => {
                        match s.handle_successful_write(&dir_name, &value_name, binary) {
                            Ok(()) => reply.ok(),
                            Err(errno) => reply.error(errno),
                        }
                    }
                    Err(err) => {
                        let errno = s.handle_failed_write(&dir_name, &value_name, err);
                        reply.error(errno);
                    }
                }
            }
            InodeKind::WitbFile {
                dir_name,
                value_name,
            } => {
                let dir_name = dir_name.clone();
                let value_name = value_name.clone();

                let schema = match s.schemas.get_schema(&dir_name) {
                    Some(schema) => schema,
                    None => {
                        reply.error(EINVAL);
                        return;
                    }
                };

                match validate_binary(&buffer, schema) {
                    Ok(binary) => {
                        match s.handle_successful_write(&dir_name, &value_name, binary) {
                            Ok(()) => reply.ok(),
                            Err(errno) => reply.error(errno),
                        }
                    }
                    Err(err) => {
                        let errno = s.handle_failed_write(&dir_name, &value_name, err);
                        reply.error(errno);
                    }
                }
            }
            _ => {
                reply.error(EACCES);
            }
        }
    }

    fn release(
        &self,
        _req: &Request,
        _ino: INodeNo,
        fh: FileHandle,
        _flags: fuser::OpenFlags,
        _lock_owner: Option<fuser::LockOwner>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        self.lock().write_buffers.remove(&fh.0);
        reply.ok();
    }

    fn create(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
        _flags: i32,
        reply: ReplyCreate,
    ) {
        let mut s = self.lock();
        let parent = parent.0;
        if s.read_only {
            reply.error(EACCES);
            return;
        }

        let parent_entry = match s.inodes.get(parent) {
            Some(e) => e.clone(),
            None => {
                reply.error(ENOENT);
                return;
            }
        };

        let name_str = name.to_string_lossy().to_string();
        let parsed = parse_filename(&name_str);

        match &parent_entry.kind {
            InodeKind::RootDir => {
                reply.error(EACCES);
            }
            InodeKind::TypedDir { dir_name } => {
                let dir_name = dir_name.clone();
                match parsed {
                    FilenameParts::SchemaFile => {
                        s.inodes.add_schema_files(&dir_name, parent);
                        let ino = match s.inodes.lookup(parent, name) {
                            Some(ino) => ino,
                            None => {
                                reply.error(EIO);
                                return;
                            }
                        };
                        let fh = s.alloc_fh();
                        s.write_buffers.insert(
                            fh.0,
                            WriteBuffer {
                                ino,
                                data: Vec::new(),
                            },
                        );
                        let attr = s.make_file_attr(ino, 0, false);
                        reply.created(&TTL, &attr, GEN, fh, FopenFlags::FOPEN_DIRECT_IO);
                    }
                    FilenameParts::Value { stem, ext } => match ext {
                        ValueExt::Wave | ValueExt::Witb => {
                            if !s.schemas.has_schema(&dir_name) {
                                reply.error(EINVAL);
                                return;
                            }
                            s.inodes.add_value(&dir_name, parent, &stem);
                            let ino = match s.inodes.lookup(parent, name) {
                                Some(ino) => ino,
                                None => {
                                    reply.error(EIO);
                                    return;
                                }
                            };
                            let fh = s.alloc_fh();
                            s.write_buffers.insert(
                                fh.0,
                                WriteBuffer {
                                    ino,
                                    data: Vec::new(),
                                },
                            );
                            let attr = s.make_file_attr(ino, 0, false);
                            reply.created(&TTL, &attr, GEN, fh, FopenFlags::FOPEN_DIRECT_IO);
                        }
                        ValueExt::Witerr | ValueExt::Witerrb => {
                            reply.error(EACCES);
                        }
                    },
                    FilenameParts::ErrorSchemaFile => {
                        reply.error(EACCES);
                    }
                    FilenameParts::Unknown => {
                        reply.error(EINVAL);
                    }
                }
            }
            _ => {
                reply.error(ENOTDIR);
            }
        }
    }

    fn mkdir(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        let mut s = self.lock();
        let parent = parent.0;
        if s.read_only {
            reply.error(EACCES);
            return;
        }

        if parent != ROOT_INO {
            reply.error(EACCES);
            return;
        }

        let name_str = name.to_string_lossy().to_string();

        if s.inodes.lookup(parent, name).is_some() {
            reply.error(EEXIST);
            return;
        }

        if s.store.ensure_dir(&name_str).is_err() {
            reply.error(EIO);
            return;
        }

        let dir_ino = s.inodes.add_dir(&name_str);
        tracing::debug!(dir = %name_str, "Directory created");
        let attr = s.make_dir_attr(dir_ino);
        reply.entry(&TTL, &attr, GEN);
    }

    fn unlink(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        let mut s = self.lock();
        let parent = parent.0;
        if s.read_only {
            reply.error(EACCES);
            return;
        }

        let ino = match s.inodes.lookup(parent, name) {
            Some(ino) => ino,
            None => {
                reply.error(ENOENT);
                return;
            }
        };

        let entry = match s.inodes.get(ino) {
            Some(e) => e.clone(),
            None => {
                reply.error(ENOENT);
                return;
            }
        };

        match &entry.kind {
            InodeKind::ErrorSchemaFile { .. } => {
                reply.error(EACCES);
            }
            InodeKind::SchemaFile { dir_name } => {
                let dir_name = dir_name.clone();
                s.schemas.remove_schema(&dir_name);
                let _ = s.store.remove_schema(&dir_name);
                s.inodes.remove_schema_files(parent);
                reply.ok();
            }
            InodeKind::WaveFile {
                dir_name,
                value_name,
            }
            | InodeKind::WitbFile {
                dir_name,
                value_name,
            } => {
                let dir_name = dir_name.clone();
                let value_name = value_name.clone();
                let _ = s.store.remove_value(&dir_name, &value_name);
                let _ = s.store.remove_error(&dir_name, &value_name);
                s.inodes.remove_value(parent, &value_name);
                reply.ok();
            }
            InodeKind::WiterrFile {
                dir_name,
                value_name,
            }
            | InodeKind::WiterrbFile {
                dir_name,
                value_name,
            } => {
                let dir_name = dir_name.clone();
                let value_name = value_name.clone();
                let _ = s.store.remove_error(&dir_name, &value_name);
                s.inodes.remove_error_files(parent, &value_name);
                reply.ok();
            }
            _ => {
                reply.error(EACCES);
            }
        }
    }

    fn rmdir(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        let mut s = self.lock();
        let parent = parent.0;
        if s.read_only {
            reply.error(EACCES);
            return;
        }

        if parent != ROOT_INO {
            reply.error(EACCES);
            return;
        }

        let name_str = name.to_string_lossy().to_string();

        let dir_ino = match s.inodes.lookup(parent, name) {
            Some(ino) => ino,
            None => {
                reply.error(ENOENT);
                return;
            }
        };

        let children = s.inodes.children(dir_ino);
        if !children.is_empty() {
            reply.error(ENOTEMPTY);
            return;
        }

        s.schemas.remove_schema(&name_str);
        let _ = s.store.remove_dir(&name_str);
        s.inodes.remove_dir(&name_str);
        reply.ok();
    }

    fn getxattr(
        &self,
        _req: &Request,
        ino: INodeNo,
        name: &OsStr,
        size: u32,
        reply: ReplyXattr,
    ) {
        let s = self.lock();
        let entry = match s.inodes.get(ino.0) {
            Some(e) => e.clone(),
            None => {
                reply.error(ENOENT);
                return;
            }
        };

        let name_str = name.to_string_lossy();

        let value = match name_str.as_ref() {
            "user.wit-fs.valid" => match &entry.kind {
                InodeKind::WaveFile {
                    dir_name,
                    value_name,
                }
                | InodeKind::WitbFile {
                    dir_name,
                    value_name,
                } => {
                    if s.store.has_error(dir_name, value_name) {
                        b"false".to_vec()
                    } else {
                        b"true".to_vec()
                    }
                }
                _ => {
                    reply.error(ENOENT);
                    return;
                }
            },
            "user.wit-fs.error" => match &entry.kind {
                InodeKind::WaveFile {
                    dir_name,
                    value_name,
                }
                | InodeKind::WitbFile {
                    dir_name,
                    value_name,
                } => match s.store.read_error_wave(dir_name, value_name) {
                    Ok(Some(err_text)) => err_text.into_bytes(),
                    _ => {
                        reply.error(ENOENT);
                        return;
                    }
                },
                _ => {
                    reply.error(ENOENT);
                    return;
                }
            },
            _ => {
                reply.error(ENOENT);
                return;
            }
        };

        if size == 0 {
            reply.size(value.len() as u32);
        } else if size >= value.len() as u32 {
            reply.data(&value);
        } else {
            reply.error(ERANGE);
        }
    }

    fn listxattr(&self, _req: &Request, ino: INodeNo, size: u32, reply: ReplyXattr) {
        let s = self.lock();
        let entry = match s.inodes.get(ino.0) {
            Some(e) => e.clone(),
            None => {
                reply.error(ENOENT);
                return;
            }
        };

        let xattrs = match &entry.kind {
            InodeKind::WaveFile { .. } | InodeKind::WitbFile { .. } => {
                b"user.wit-fs.valid\0user.wit-fs.error\0".to_vec()
            }
            _ => Vec::new(),
        };

        if size == 0 {
            reply.size(xattrs.len() as u32);
        } else if size >= xattrs.len() as u32 {
            reply.data(&xattrs);
        } else {
            reply.error(ERANGE);
        }
    }

    fn statfs(&self, _req: &Request, _ino: INodeNo, reply: fuser::ReplyStatfs) {
        reply.statfs(0, 0, 0, 0, 0, BLOCK_SIZE, 255, 0);
    }
}
