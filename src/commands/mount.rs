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

        me.nodes.insert(
            ROOT,
            Node {
                parent: ROOT,
                name: "/".into(),
                kind: Kind::Dir(BTreeMap::new()),
            },
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

        Ok(me)
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
            Node {
                parent,
                name: name.to_string(),
                kind: Kind::Dir(BTreeMap::new()),
            },
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
            Node {
                parent: cur,
                name: name.clone(),
                kind: Kind::File(body),
            },
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
        let is_dir = matches!(
            self.nodes.get(&ino),
            Some(Node {
                kind: Kind::Dir(_),
                ..
            })
        );

        let size = self.size_of(ino);
        let now = SystemTime::now();

        FileAttr {
            ino,
            size,
            blocks: size.div_ceil(512),
            atime: now,
            mtime: now,
            ctime: now,
            crtime: now,
            kind: if is_dir {
                FileType::Directory
            } else {
                FileType::RegularFile
            },
            perm: if is_dir { 0o755 } else { 0o644 },
            nlink: if is_dir { 2 } else { 1 },
            uid: self.uid,
            gid: self.gid,
            rdev: 0,
            blksize: 512,
            flags: 0,
        }
    }

    fn collect(&mut self, ino: u64, prefix: &str, out: &mut Vec<(String, Vec<u8>)>) {
        let children = match self.nodes.get(&ino) {
            Some(Node {
                kind: Kind::Dir(c), ..
            }) => c.clone(),
            _ => return,
        };

        for (name, child) in children {
            let path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };

            let is_file = matches!(
                self.nodes.get(&child),
                Some(Node {
                    kind: Kind::File(_),
                    ..
                })
            );

            if is_file {
                if self.materialize(child).is_err() {
                    continue;
                }
                if let Some(Node {
                    kind: Kind::File(Body::Loaded(b)),
                    ..
                }) = self.nodes.get(&child)
                {
                    out.push((path, b.clone()));
                }
            } else {
                self.collect(child, &path, out);
            }
        }
    }

    fn repack(&mut self) -> io::Result<()> {
        let mut files: Vec<(String, Vec<u8>)> = Vec::new();
        self.collect(ROOT, "", &mut files);
        files.sort_by(|a, b| a.0.cmp(&b.0));

        let (meta, payload) = dir::pack(&files);
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
        _mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        size: Option<u64>,
        _atime: Option<TimeOrNow>,
        _mtime: Option<TimeOrNow>,
        _ctime: Option<SystemTime>,
        _fh: Option<u64>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        if let Some(new_len) = size {
            if self.materialize(ino).is_err() {
                reply.error(libc::EIO);
                return;
            }
            if let Some(Node {
                kind: Kind::File(Body::Loaded(buf)),
                ..
            }) = self.nodes.get_mut(&ino)
            {
                buf.resize(new_len as usize, 0);
                self.dirty = true;
            }
        }

        if self.nodes.contains_key(&ino) {
            reply.attr(&TTL, &self.attr(ino));
        } else {
            reply.error(libc::ENOENT);
        }
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
        if self.materialize(ino).is_err() {
            reply.error(libc::EIO);
            return;
        }

        if let Some(Node {
            kind: Kind::File(Body::Loaded(b)),
            ..
        }) = self.nodes.get(&ino)
        {
            let start = (offset as usize).min(b.len());
            let end = (start + size as usize).min(b.len());
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
        if self.materialize(ino).is_err() {
            reply.error(libc::EIO);
            return;
        }

        let off = offset as usize;
        let wrote = if let Some(Node {
            kind: Kind::File(Body::Loaded(buf)),
            ..
        }) = self.nodes.get_mut(&ino)
        {
            let end = off + data.len();
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
                    Node {
                        parent,
                        name: name.clone(),
                        kind: Kind::File(Body::Loaded(Vec::new())),
                    },
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

        let ino = self.ensure_dir(parent, &name);
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
        _flags: u32,
        reply: ReplyEmpty,
    ) {
        let name = name.to_string_lossy().into_owned();
        let newname = newname.to_string_lossy().into_owned();

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

        if let Some(old) = self.child(newparent, &newname) {
            self.nodes.remove(&old);
            if let Some(Node {
                kind: Kind::Dir(children),
                ..
            }) = self.nodes.get_mut(&newparent)
            {
                children.remove(&newname);
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
        reply.ok();
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

    #[cfg(target_os = "macos")]
    unsafe {
        libc::unmount(c.as_ptr(), libc::MNT_FORCE);
    }

    #[cfg(not(target_os = "macos"))]
    unsafe {
        libc::umount2(c.as_ptr(), libc::MNT_DETACH);
    }
}

pub fn run(archive: &str, mountpoint: &str, verbose: bool, log_lines: bool) {
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

    #[cfg_attr(not(target_os = "macos"), allow(unused_mut))]
    let mut options = vec![
        MountOption::FSName("fossil".into()),
        MountOption::Subtype("fossil".into()),
        MountOption::DefaultPermissions,
        MountOption::AutoUnmount,
    ];

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
