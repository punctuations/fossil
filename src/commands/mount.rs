use std::collections::{BTreeMap, HashMap, VecDeque};
use std::ffi::{CString, OsStr};
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use fuser::{
    FileAttr, FileType, Filesystem, MountOption, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory,
    ReplyEmpty, ReplyEntry, ReplyOpen, ReplyWrite, Request, TimeOrNow,
};

use fossil::core::container::{self, BlockRef};
use fossil::core::{crc, dir};

use crate::error;
use crate::utils::color::{Color, paint};

type EventLog = Arc<Mutex<VecDeque<String>>>;

enum Sink {
    None,
    Ring(EventLog),
    Lines(Instant),
}

const TTL: Duration = Duration::from_secs(1);
const ROOT: u64 = 1;

const RENAME_NOREPLACE: u32 = 1;
const RENAME_EXCHANGE: u32 = 2;

const MAX_FILE: u64 = 1 << 32;

fn resolve_time(t: TimeOrNow) -> SystemTime {
    match t {
        TimeOrNow::SpecificTime(t) => t,
        TimeOrNow::Now => SystemTime::now(),
    }
}

enum Body {
    Stored {
        offset: usize,
        len: usize,
        crc: Option<u32>,
    },
    Loaded(Vec<u8>),
}

enum Kind {
    Dir(BTreeMap<String, u64>),
    File(Body),
}

struct Node {
    parent: u64,
    name: String,
    kind: Kind,
    perm: u16,
    uid: u32,
    gid: u32,
    atime: SystemTime,
    mtime: SystemTime,
    ctime: SystemTime,
    crtime: SystemTime,
}

impl Node {
    fn new(parent: u64, name: String, kind: Kind, uid: u32, gid: u32) -> Self {
        let now = SystemTime::now();
        let perm = if matches!(kind, Kind::Dir(_)) { 0o755 } else { 0o644 };

        Node {
            parent,
            name,
            kind,
            perm,
            uid,
            gid,
            atime: now,
            mtime: now,
            ctime: now,
            crtime: now,
        }
    }
}

struct FossilFs {
    path: PathBuf,
    data: Vec<u8>,
    blocks: Vec<BlockRef>,
    seg_blocks: usize,
    cache: BTreeMap<usize, Vec<u8>>,
    nodes: HashMap<u64, Node>,
    next: u64,
    dirty: bool,
    uid: u32,
    gid: u32,
    sink: Sink,
}

impl FossilFs {
    fn open(path: &str, sink: Sink) -> io::Result<Self> {
        let data = fs::read(path)?;
        let parts = container::read_lazy(&data)?.into_parts();

        if parts.ext != "/" {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a directory fossil (nothing to mount)",
            ));
        }

        if parts.meta.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "directory fossil has no manifest",
            ));
        }

        let entries = dir::read(&parts.meta)?;
        let dirs = dir::read_dirs(&parts.meta)?;

        let mut me = FossilFs {
            path: PathBuf::from(path),
            blocks: parts.blocks,
            seg_blocks: parts.seg_blocks,
            data,
            cache: BTreeMap::new(),
            nodes: HashMap::new(),
            next: ROOT + 1,
            dirty: false,
            uid: unsafe { libc::getuid() },
            gid: unsafe { libc::getgid() },
            sink,
        };

        let (uid, gid) = (me.uid, me.gid);
        me.nodes.insert(
            ROOT,
            Node::new(ROOT, "/".into(), Kind::Dir(BTreeMap::new()), uid, gid),
        );

        for e in &entries {
            me.insert_file(
                &e.path,
                Body::Stored {
                    offset: e.offset,
                    len: e.len,
                    crc: e.crc,
                },
            );
        }

        for d in &dirs {
            me.insert_dir(d);
        }

        Ok(me)
    }

    fn insert_dir(&mut self, path: &str) {
        let mut cur = ROOT;
        for comp in path.split('/').filter(|s| !s.is_empty()) {
            cur = self.ensure_dir(cur, comp);
        }
    }

    fn ensure_dir(&mut self, parent: u64, name: &str) -> u64 {
        if let Some(Node {
            kind: Kind::Dir(children),
            ..
        }) = self.nodes.get(&parent)
        {
            if let Some(&ino) = children.get(name) {
                return ino;
            }
        }

        let ino = self.next;
        self.next += 1;
        self.nodes.insert(
            ino,
            Node::new(
                parent,
                name.to_string(),
                Kind::Dir(BTreeMap::new()),
                self.uid,
                self.gid,
            ),
        );
        if let Some(Node {
            kind: Kind::Dir(children),
            ..
        }) = self.nodes.get_mut(&parent)
        {
            children.insert(name.to_string(), ino);
        }
        ino
    }

    fn insert_file(&mut self, path: &str, body: Body) {
        let comps: Vec<String> = path
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        if comps.is_empty() {
            return;
        }

        let mut cur = ROOT;
        for comp in &comps[..comps.len() - 1] {
            cur = self.ensure_dir(cur, comp);
        }

        let name = comps.last().unwrap().clone();
        let ino = self.next;
        self.next += 1;
        self.nodes.insert(
            ino,
            Node::new(cur, name.clone(), Kind::File(body), self.uid, self.gid),
        );
        if let Some(Node {
            kind: Kind::Dir(children),
            ..
        }) = self.nodes.get_mut(&cur)
        {
            children.insert(name, ino);
        }
    }

    fn path_of(&self, ino: u64) -> String {
        let mut parts = Vec::new();
        let mut cur = ino;
        while cur != ROOT {
            match self.nodes.get(&cur) {
                Some(node) => {
                    parts.push(node.name.clone());
                    cur = node.parent;
                }
                None => break,
            }
        }
        parts.reverse();
        parts.join("/")
    }

    fn note(&self, event: String) {
        match &self.sink {
            Sink::None => {}
            Sink::Ring(log) => {
                let mut log = log.lock().unwrap();
                log.push_back(event);
                while log.len() > 5 {
                    log.pop_front();
                }
            }
            Sink::Lines(start) => {
                eprintln!("  +{:.3}s {}", start.elapsed().as_secs_f64(), event);
            }
        }
    }

    fn child(&self, parent: u64, name: &str) -> Option<u64> {
        match self.nodes.get(&parent) {
            Some(Node {
                kind: Kind::Dir(children),
                ..
            }) => children.get(name).copied(),
            _ => None,
        }
    }

    fn is_dir(&self, ino: u64) -> bool {
        matches!(
            self.nodes.get(&ino),
            Some(Node {
                kind: Kind::Dir(_),
                ..
            })
        )
    }

    fn is_ancestor(&self, ino: u64, of: u64) -> bool {
        let mut cur = of;
        loop {
            if cur == ino {
                return true;
            }
            if cur == ROOT {
                return false;
            }
            match self.nodes.get(&cur) {
                Some(node) => cur = node.parent,
                None => return false,
            }
        }
    }

    fn materialize(&mut self, ino: u64) -> io::Result<()> {
        let (offset, len, want) = match self.nodes.get(&ino) {
            Some(Node {
                kind: Kind::File(Body::Stored { offset, len, crc }),
                ..
            }) => (*offset, *len, *crc),
            _ => return Ok(()),
        };

        let bytes = container::decode_range(
            &self.data,
            &self.blocks,
            self.seg_blocks,
            &mut self.cache,
            offset,
            len,
        )?;

        if let Some(want) = want {
            if crc::crc32(&bytes) != want {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "file checksum mismatch in archive",
                ));
            }
        }

        self.note(format!("decode {} · {} B", self.path_of(ino), bytes.len()));

        if let Some(node) = self.nodes.get_mut(&ino) {
            node.kind = Kind::File(Body::Loaded(bytes));
        }
        Ok(())
    }

    fn size_of(&self, ino: u64) -> u64 {
        match self.nodes.get(&ino) {
            Some(Node {
                kind: Kind::File(Body::Stored { len, .. }),
                ..
            }) => *len as u64,
            Some(Node {
                kind: Kind::File(Body::Loaded(b)),
                ..
            }) => b.len() as u64,
            _ => 0,
        }
    }

    fn attr(&self, ino: u64) -> FileAttr {
        let node = &self.nodes[&ino];
        let is_dir = matches!(node.kind, Kind::Dir(_));
        let size = self.size_of(ino);

        FileAttr {
            ino,
            size,
            blocks: size.div_ceil(512),
            atime: node.atime,
            mtime: node.mtime,
            ctime: node.ctime,
            crtime: node.crtime,
            kind: if is_dir {
                FileType::Directory
            } else {
                FileType::RegularFile
            },
            perm: node.perm,
            nlink: if is_dir { 2 } else { 1 },
            uid: node.uid,
            gid: node.gid,
            rdev: 0,
            blksize: 512,
            flags: 0,
        }
    }

    fn collect(
        &mut self,
        ino: u64,
        prefix: &str,
        out: &mut Vec<(String, Vec<u8>)>,
        dirs: &mut Vec<String>,
    ) -> io::Result<()> {
        let children = match self.nodes.get(&ino) {
            Some(Node {
                kind: Kind::Dir(c), ..
            }) => c.clone(),
            _ => return Ok(()),
        };

        for (name, child) in children {
            let path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };

            match self.nodes.get(&child) {
                Some(Node {
                    kind: Kind::File(_),
                    ..
                }) => {
                    self.materialize(child)?;
                    if let Some(Node {
                        kind: Kind::File(Body::Loaded(b)),
                        ..
                    }) = self.nodes.get(&child)
                    {
                        out.push((path, b.clone()));
                    }
                }
                Some(Node {
                    kind: Kind::Dir(c), ..
                }) if c.is_empty() => dirs.push(path),
                Some(Node {
                    kind: Kind::Dir(_), ..
                }) => self.collect(child, &path, out, dirs)?,
                None => {}
            }
        }

        Ok(())
    }

    fn repack(&mut self) -> io::Result<()> {
        let mut files: Vec<(String, Vec<u8>)> = Vec::new();
        let mut dirs: Vec<String> = Vec::new();
        self.collect(ROOT, "", &mut files, &mut dirs)?;
        files.sort_by(|a, b| a.0.cmp(&b.0));
        dirs.sort();

        let (meta, payload) = dir::pack_tree(&files, &dirs);
        let bytes = container::write_progress_meta(&payload, "/", &meta, None, false);

        let mut tmp = self.path.clone().into_os_string();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);

        fs::write(&tmp, &bytes)?;
        fs::rename(&tmp, &self.path)?;
        self.dirty = false;
        Ok(())
    }
}

impl Drop for FossilFs {
    fn drop(&mut self) {
        if self.dirty {
            let _ = self.repack();
        }
    }
}

impl Filesystem for FossilFs {
    fn destroy(&mut self) {
        if self.dirty {
            if let Err(e) = self.repack() {
                error!("failed to write archive on unmount: {}", e);
            }
        }
    }

    fn lookup(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEntry) {
        match self.child(parent, &name.to_string_lossy()) {
            Some(ino) => reply.entry(&TTL, &self.attr(ino), 0),
            None => reply.error(libc::ENOENT),
        }
    }

    fn getattr(&mut self, _req: &Request<'_>, ino: u64, _fh: Option<u64>, reply: ReplyAttr) {
        if self.nodes.contains_key(&ino) {
            reply.attr(&TTL, &self.attr(ino));
        } else {
            reply.error(libc::ENOENT);
        }
    }

    fn setattr(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
        size: Option<u64>,
        atime: Option<TimeOrNow>,
        mtime: Option<TimeOrNow>,
        ctime: Option<SystemTime>,
        _fh: Option<u64>,
        crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        if !self.nodes.contains_key(&ino) {
            reply.error(libc::ENOENT);
            return;
        }

        if let Some(new_len) = size {
            let Ok(new_len) = usize::try_from(new_len).map_err(|_| ()) else {
                reply.error(libc::EFBIG);
                return;
            };
            if new_len as u64 > MAX_FILE {
                reply.error(libc::EFBIG);
                return;
            }
            if self.materialize(ino).is_err() {
                reply.error(libc::EIO);
                return;
            }
            if let Some(Node {
                kind: Kind::File(Body::Loaded(buf)),
                ..
            }) = self.nodes.get_mut(&ino)
            {
                buf.resize(new_len, 0);
                self.dirty = true;
            }
        }

        let node = self.nodes.get_mut(&ino).unwrap();

        if let Some(mode) = mode {
            node.perm = (mode & 0o7777) as u16;
        }
        if let Some(uid) = uid {
            node.uid = uid;
        }
        if let Some(gid) = gid {
            node.gid = gid;
        }
        if let Some(atime) = atime {
            node.atime = resolve_time(atime);
        }
        if let Some(mtime) = mtime {
            node.mtime = resolve_time(mtime);
        }
        if let Some(ctime) = ctime {
            node.ctime = ctime;
        }
        if let Some(crtime) = crtime {
            node.crtime = crtime;
        }

        reply.attr(&TTL, &self.attr(ino));
    }

    fn open(&mut self, _req: &Request<'_>, _ino: u64, _flags: i32, reply: ReplyOpen) {
        reply.opened(0, 0);
    }

    fn read(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        let Ok(offset) = usize::try_from(offset) else {
            reply.error(libc::EINVAL);
            return;
        };

        if self.materialize(ino).is_err() {
            reply.error(libc::EIO);
            return;
        }

        if let Some(Node {
            kind: Kind::File(Body::Loaded(b)),
            ..
        }) = self.nodes.get(&ino)
        {
            let start = offset.min(b.len());
            let end = start.saturating_add(size as usize).min(b.len());
            let chunk = end - start;
            reply.data(&b[start..end]);
            self.note(format!("read {} · {} B", self.path_of(ino), chunk));
        } else {
            reply.error(libc::EISDIR);
        }
    }

    fn write(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        data: &[u8],
        _write_flags: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyWrite,
    ) {
        let Ok(off) = usize::try_from(offset) else {
            reply.error(libc::EINVAL);
            return;
        };

        let Some(end) = off.checked_add(data.len()) else {
            reply.error(libc::EFBIG);
            return;
        };

        if end as u64 > MAX_FILE {
            reply.error(libc::EFBIG);
            return;
        }

        if self.materialize(ino).is_err() {
            reply.error(libc::EIO);
            return;
        }

        let wrote = if let Some(Node {
            kind: Kind::File(Body::Loaded(buf)),
            ..
        }) = self.nodes.get_mut(&ino)
        {
            if buf.len() < end {
                buf.resize(end, 0);
            }
            buf[off..end].copy_from_slice(data);
            true
        } else {
            false
        };

        if wrote {
            self.dirty = true;
            self.note(format!("write {} · {} B", self.path_of(ino), data.len()));
            reply.written(data.len() as u32);
        } else {
            reply.error(libc::EISDIR);
        }
    }

    fn create(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
        _flags: i32,
        reply: ReplyCreate,
    ) {
        let name = name.to_string_lossy().into_owned();

        if !matches!(
            self.nodes.get(&parent),
            Some(Node {
                kind: Kind::Dir(_),
                ..
            })
        ) {
            reply.error(libc::ENOTDIR);
            return;
        }

        let ino = match self.child(parent, &name) {
            Some(existing) => {
                if let Some(node) = self.nodes.get_mut(&existing) {
                    node.kind = Kind::File(Body::Loaded(Vec::new()));
                }
                existing
            }
            None => {
                let ino = self.next;
                self.next += 1;
                self.nodes.insert(
                    ino,
                    Node::new(
                        parent,
                        name.clone(),
                        Kind::File(Body::Loaded(Vec::new())),
                        self.uid,
                        self.gid,
                    ),
                );
                if let Some(Node {
                    kind: Kind::Dir(children),
                    ..
                }) = self.nodes.get_mut(&parent)
                {
                    children.insert(name, ino);
                }
                ino
            }
        };

        self.dirty = true;
        self.note(format!("create {}", self.path_of(ino)));
        reply.created(&TTL, &self.attr(ino), 0, 0, 0);
    }

    fn mkdir(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        let name = name.to_string_lossy().into_owned();

        if self.child(parent, &name).is_some() {
            reply.error(libc::EEXIST);
            return;
        }

        if !self.is_dir(parent) {
            reply.error(libc::ENOTDIR);
            return;
        }

        let ino = self.ensure_dir(parent, &name);
        self.dirty = true;
        self.note(format!("mkdir {}", self.path_of(ino)));
        reply.entry(&TTL, &self.attr(ino), 0);
    }

    fn unlink(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let name = name.to_string_lossy().into_owned();

        match self.child(parent, &name) {
            Some(ino)
                if matches!(
                    self.nodes.get(&ino),
                    Some(Node {
                        kind: Kind::File(_),
                        ..
                    })
                ) =>
            {
                self.nodes.remove(&ino);
                if let Some(Node {
                    kind: Kind::Dir(children),
                    ..
                }) = self.nodes.get_mut(&parent)
                {
                    children.remove(&name);
                }
                self.dirty = true;
                self.note(format!("rm {}", name));
                reply.ok();
            }
            Some(_) => reply.error(libc::EISDIR),
            None => reply.error(libc::ENOENT),
        }
    }

    fn rmdir(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let name = name.to_string_lossy().into_owned();

        match self.child(parent, &name) {
            Some(ino) => match self.nodes.get(&ino) {
                Some(Node {
                    kind: Kind::Dir(children),
                    ..
                }) if children.is_empty() => {
                    self.nodes.remove(&ino);
                    if let Some(Node {
                        kind: Kind::Dir(children),
                        ..
                    }) = self.nodes.get_mut(&parent)
                    {
                        children.remove(&name);
                    }
                    self.dirty = true;
                    reply.ok();
                }
                Some(Node {
                    kind: Kind::Dir(_), ..
                }) => reply.error(libc::ENOTEMPTY),
                _ => reply.error(libc::ENOTDIR),
            },
            None => reply.error(libc::ENOENT),
        }
    }

    fn rename(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        newparent: u64,
        newname: &OsStr,
        flags: u32,
        reply: ReplyEmpty,
    ) {
        let name = name.to_string_lossy().into_owned();
        let newname = newname.to_string_lossy().into_owned();

        if flags & RENAME_EXCHANGE != 0 {
            reply.error(libc::EOPNOTSUPP);
            return;
        }

        let ino = match self.child(parent, &name) {
            Some(ino) => ino,
            None => {
                reply.error(libc::ENOENT);
                return;
            }
        };

        if !matches!(
            self.nodes.get(&newparent),
            Some(Node {
                kind: Kind::Dir(_),
                ..
            })
        ) {
            reply.error(libc::ENOTDIR);
            return;
        }

        if self.is_ancestor(ino, newparent) {
            reply.error(libc::EINVAL);
            return;
        }

        if let Some(old) = self.child(newparent, &newname) {
            if flags & RENAME_NOREPLACE != 0 {
                reply.error(libc::EEXIST);
                return;
            }

            if old != ino {
                let src_is_dir = self.is_dir(ino);
                match self.nodes.get(&old) {
                    Some(Node {
                        kind: Kind::Dir(children),
                        ..
                    }) => {
                        if !src_is_dir {
                            reply.error(libc::EISDIR);
                            return;
                        }
                        if !children.is_empty() {
                            reply.error(libc::ENOTEMPTY);
                            return;
                        }
                    }
                    Some(Node {
                        kind: Kind::File(_),
                        ..
                    }) => {
                        if src_is_dir {
                            reply.error(libc::ENOTDIR);
                            return;
                        }
                    }
                    None => {}
                }

                self.nodes.remove(&old);
                if let Some(Node {
                    kind: Kind::Dir(children),
                    ..
                }) = self.nodes.get_mut(&newparent)
                {
                    children.remove(&newname);
                }
            }
        }

        if let Some(Node {
            kind: Kind::Dir(children),
            ..
        }) = self.nodes.get_mut(&parent)
        {
            children.remove(&name);
        }
        if let Some(Node {
            kind: Kind::Dir(children),
            ..
        }) = self.nodes.get_mut(&newparent)
        {
            children.insert(newname.clone(), ino);
        }
        if let Some(node) = self.nodes.get_mut(&ino) {
            node.parent = newparent;
            node.name = newname;
        }

        self.dirty = true;
        reply.ok();
    }

    fn readdir(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        let (children, parent) = match self.nodes.get(&ino) {
            Some(Node {
                kind: Kind::Dir(c),
                parent,
                ..
            }) => (c.clone(), *parent),
            _ => {
                reply.error(libc::ENOTDIR);
                return;
            }
        };

        let mut rows: Vec<(u64, FileType, String)> = Vec::new();
        rows.push((ino, FileType::Directory, ".".into()));
        rows.push((parent, FileType::Directory, "..".into()));
        for (cname, cino) in &children {
            let ft = match self.nodes.get(cino) {
                Some(Node {
                    kind: Kind::Dir(_), ..
                }) => FileType::Directory,
                _ => FileType::RegularFile,
            };
            rows.push((*cino, ft, cname.clone()));
        }

        for (i, (cino, ft, cname)) in rows.into_iter().enumerate().skip(offset as usize) {
            if reply.add(cino, (i + 1) as i64, ft, &cname) {
                break;
            }
        }
        reply.ok();
    }

    fn flush(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        _fh: u64,
        _lock_owner: u64,
        reply: ReplyEmpty,
    ) {
        reply.ok();
    }

    fn fsync(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        _fh: u64,
        _datasync: bool,
        reply: ReplyEmpty,
    ) {
        if !self.dirty {
            reply.ok();
            return;
        }

        match self.repack() {
            Ok(()) => reply.ok(),
            Err(e) => {
                self.note(format!("fsync failed · {}", e));
                reply.error(libc::EIO);
            }
        }
    }
}

static STOP: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_: libc::c_int) {
    STOP.store(true, Ordering::SeqCst);
}

const DOTS: [&str; 3] = ["  ·", " ··", "···"];

struct Serve {
    done: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Serve {
    fn start(msg: String, log: Option<EventLog>) -> Self {
        let done = Arc::new(AtomicBool::new(false));

        if !io::stderr().is_terminal() {
            return Serve { done, handle: None };
        }

        let flag = done.clone();
        let handle = thread::spawn(move || {
            let mut i = 0;
            let mut drawn = 1usize;
            while !flag.load(Ordering::Relaxed) {
                if drawn > 1 {
                    eprint!("\x1b[{}A", drawn - 1);
                }
                eprint!("\r\x1b[J");
                eprint!(
                    "  {} {}",
                    paint(DOTS[i % DOTS.len()], "38;5;173"),
                    paint(&msg, "38;5;173")
                );

                if let Some(log) = &log {
                    let events: Vec<String> = log.lock().unwrap().iter().cloned().collect();
                    for e in &events {
                        eprint!("\n\x1b[2K    {}", e.clone().dim());
                    }
                    for _ in events.len()..5 {
                        eprint!("\n\x1b[2K");
                    }
                    drawn = 6;
                } else {
                    drawn = 1;
                }

                let _ = io::stderr().flush();
                i += 1;
                thread::sleep(Duration::from_millis(400));
            }

            if drawn > 1 {
                eprint!("\x1b[{}A", drawn - 1);
            }
            eprint!("\r\x1b[J");
            let _ = io::stderr().flush();
        });

        Serve {
            done,
            handle: Some(handle),
        }
    }

    fn stop(self) {
        self.done.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle {
            let end = Instant::now() + Duration::from_millis(400);
            while !h.is_finished() && Instant::now() < end {
                thread::sleep(Duration::from_millis(20));
            }
            if h.is_finished() {
                let _ = h.join();
            }
        }
    }
}

fn force_unmount(mountpoint: &Path) {
    let Ok(c) = CString::new(mountpoint.as_os_str().as_bytes()) else {
        return;
    };

    #[cfg(any(target_os = "linux", target_os = "android"))]
    unsafe {
        libc::umount2(c.as_ptr(), libc::MNT_DETACH);
    }

    #[cfg(any(
        target_os = "macos",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    unsafe {
        libc::unmount(c.as_ptr(), libc::MNT_FORCE);
    }

    #[cfg(not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd"
    )))]
    let _ = c;
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn allow_auto_unmount() -> bool {
    const CONF: &str = "/etc/fuse.conf";

    let already_set = fs::read_to_string(CONF).is_ok_and(|conf| {
        conf.lines()
            .any(|line| line.split('#').next().unwrap_or("").trim() == "user_allow_other")
    });

    if already_set || unsafe { libc::geteuid() } == 0 {
        return true;
    }

    let declined = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map(|base| base.join("fossil").join("no-fuse-conf"));

    if declined.as_ref().is_some_and(|path| path.exists()) || !io::stdin().is_terminal() {
        return false;
    }

    println!(
        "  auto-unmount needs {} in {}",
        "user_allow_other".accent(),
        CONF.accent()
    );
    println!("  without it, a crash can leave the mount point stale");
    print!("  add it now (uses sudo)? [y/N] ");
    io::stdout().flush().ok();

    let mut answer = String::new();
    if io::stdin().read_line(&mut answer).is_err() {
        return false;
    }

    if !matches!(answer.trim(), "y" | "Y" | "yes") {
        if let Some(path) = declined {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&path, "");
        }
        return false;
    }

    let added = std::process::Command::new("sudo")
        .args(["sh", "-c", "echo user_allow_other >> /etc/fuse.conf"])
        .status()
        .is_ok_and(|s| s.success());

    if !added {
        error!("couldn't write /etc/fuse.conf, mounting without auto-unmount");
    }
    added
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn allow_auto_unmount() -> bool {
    true
}

#[cfg(target_os = "macos")]
fn macfuse_present() -> bool {
    Path::new("/usr/local/lib/libfuse.2.dylib").exists()
}

#[cfg(target_os = "macos")]
fn ensure_macfuse() -> bool {
    if macfuse_present() {
        return true;
    }

    let marker = std::env::var("HOME")
        .map(|h| PathBuf::from(h).join("Library/Application Support/fossil/macfuse-prompted"));
    let already_asked = marker.as_ref().map(|m| m.exists()).unwrap_or(true);

    if already_asked || !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        error!("mount needs macFUSE (brew install --cask macfuse)");
        return false;
    }

    if let Ok(m) = &marker {
        if let Some(dir) = m.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let _ = fs::write(m, b"");
    }

    eprintln!(
        "  {}",
        "fossil mount needs macFUSE, which is not installed".bold()
    );
    eprint!("  install it now with homebrew? {} ", "[y/N]".dim());
    let _ = io::stderr().flush();

    let mut line = String::new();
    if io::stdin().read_line(&mut line).is_err()
        || !matches!(line.trim(), "y" | "Y" | "yes" | "Yes")
    {
        error!("mount needs macFUSE (brew install --cask macfuse)");
        return false;
    }

    let status = std::process::Command::new("brew")
        .args(["install", "--cask", "macfuse"])
        .status();

    match status {
        Ok(s) if s.success() => {
            eprintln!(
                "  {}",
                "macFUSE installed · approve the system extension in".bold()
            );
            eprintln!(
                "  {}",
                "System Settings › Privacy & Security, then run fossil mount again".bold()
            );
        }
        _ => error!("install failed; get macFUSE from https://macfuse.github.io"),
    }
    false
}

pub fn run(archive: &str, mountpoint: &str, verbose: bool, log_lines: bool) {
    #[cfg(target_os = "macos")]
    if !ensure_macfuse() {
        return;
    }

    let ring: Option<EventLog> =
        (verbose && !log_lines).then(|| Arc::new(Mutex::new(VecDeque::new())));

    let sink = if log_lines {
        Sink::Lines(Instant::now())
    } else if let Some(r) = &ring {
        Sink::Ring(r.clone())
    } else {
        Sink::None
    };

    let fs = match FossilFs::open(archive, sink) {
        Ok(fs) => fs,
        Err(e) => {
            error!("{}", e);
            return;
        }
    };

    let mp = Path::new(mountpoint);
    if !mp.is_dir() {
        error!("mount point is not a directory: {}", mountpoint);
        return;
    }

    let mut options = vec![
        MountOption::FSName("fossil".into()),
        MountOption::Subtype("fossil".into()),
        MountOption::DefaultPermissions,
    ];

    if allow_auto_unmount() {
        options.push(MountOption::AutoUnmount);
    }

    #[cfg(target_os = "macos")]
    options.push(MountOption::CUSTOM("noappledouble".into()));

    unsafe {
        libc::signal(libc::SIGINT, on_signal as *const () as libc::sighandler_t);
        libc::signal(libc::SIGTERM, on_signal as *const () as libc::sighandler_t);
    }

    let session = match fuser::spawn_mount2(fs, mp, &options) {
        Ok(s) => s,
        Err(e) => {
            error!("mount failed: {}", e);
            return;
        }
    };

    println!(
        "  {} {} {}",
        archive.accent(),
        "→".bold(),
        mountpoint.accent()
    );

    let serve = if log_lines {
        println!("  serving {} · ctrl-c to commit", mountpoint);
        None
    } else {
        Some(Serve::start(
            format!("serving {} · ctrl-c to commit", mountpoint),
            ring,
        ))
    };

    while !STOP.load(Ordering::SeqCst) && !session.guard.is_finished() {
        thread::sleep(Duration::from_millis(200));
    }

    if let Some(serve) = serve {
        serve.stop();
    }
    println!("  unmounting…");

    let joiner = thread::spawn(move || session.join());
    let watch = mp.to_path_buf();
    let started = Instant::now();

    while !joiner.is_finished() {
        if started.elapsed() >= Duration::from_secs(2) {
            force_unmount(&watch);
            thread::sleep(Duration::from_millis(500));
        } else {
            thread::sleep(Duration::from_millis(100));
        }
    }

    let _ = joiner.join();
    println!("  unmounted");
}

pub fn help() -> Vec<String> {
    vec![
        "fossil mount".header(),
        "mount a directory fossil as a live filesystem".bold(),
        "".into(),
        "usage".header(),
        "  fossil mount [--verbose] <dir.fossil> <mountpoint>".into(),
        "".into(),
        "arguments".header(),
        "  <dir.fossil>      directory archive to mount".into(),
        "  <mountpoint>      empty directory to mount onto".into(),
        "".into(),
        "options".header(),
        "  --verbose, -v     rolling display of the last few filesystem events".into(),
        "  --log, -l         print every event line-by-line, for diagnosing".into(),
        "".into(),
        "notes".header(),
        "  reads decode on demand; writes are staged in memory".into(),
        "  changes are written back to the archive on unmount".into(),
        "  stop with ctrl-c (or unmount the volume)".into(),
        "  rename is unreliable on macFUSE; copy then delete instead".into(),
        "".into(),
        "examples".header(),
        "  fossil mount project.fossil mnt/".into(),
        "  fossil mount --verbose project.fossil mnt/".into(),
    ]
}
