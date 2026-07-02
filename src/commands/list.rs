use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use fossil::core::{container, dir};

use crate::utils::color::Color;
use crate::utils::spinner::Spinner;
use crate::{error, n};

pub fn run(input: &str) {
    let sp = Spinner::start("sifting…");
    let result = list(input);
    sp.stop();

    match result {
        Ok(r) => {
            n!();

            println!("  {}", r.input.display().to_string().accent());
            println!(
                "  {} blocks → {} bytes ({} files)",
                r.blocks.to_string().bold(),
                r.size.to_string().bold(),
                r.files.len().to_string().bold()
            );

            n!();
            print_tree(&r.files);
            n!();
        }
        Err(err) => error!("{}", err),
    }
}

struct ListReport {
    input: PathBuf,
    blocks: usize,
    size: usize,
    files: Vec<ListEntry>,
}

#[derive(Debug, Clone)]
struct ListEntry {
    path: String,
    size: usize,
}

fn list(input: &str) -> io::Result<ListReport> {
    let from_stdin = input == "-";

    let data = if from_stdin {
        let mut buf = Vec::new();
        io::stdin().read_to_end(&mut buf)?;
        buf
    } else {
        let input_path = Path::new(input);

        if !input_path.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("input path is not a file: {}", input_path.display()),
            ));
        }

        fs::read(input_path)?
    };

    let archive = container::read(&data)?;

    if archive.ext != "/" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a directory fossil",
        ));
    }

    if archive.meta.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "directory fossil has no manifest",
        ));
    }

    let mut files = dir::read(&archive.meta)?
        .into_iter()
        .map(|entry| ListEntry {
            path: entry.path,
            size: entry.len,
        })
        .collect::<Vec<_>>();

    files.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(ListReport {
        input: if from_stdin {
            PathBuf::from("stdin")
        } else {
            Path::new(input).to_path_buf()
        },
        blocks: archive.blocks.len(),
        size: archive.orig_size,
        files,
    })
}

#[derive(Default)]
struct TreeNode {
    size: Option<usize>,
    children: BTreeMap<String, TreeNode>,
}

fn print_tree(files: &[ListEntry]) {
    let mut root = TreeNode::default();

    for file in files {
        let parts = file
            .path
            .split('/')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();

        let mut node = &mut root;

        for (i, part) in parts.iter().enumerate() {
            node = node.children.entry((*part).to_string()).or_default();

            if i == parts.len() - 1 {
                node.size = Some(file.size);
            }
        }
    }

    print_tree_node(&root, "");
}

fn print_tree_node(node: &TreeNode, prefix: &str) {
    let len = node.children.len();

    let name_width = node
        .children
        .iter()
        .map(|(name, child)| {
            if child.size.is_some() {
                name.chars().count()
            } else {
                name.chars().count() + 1
            }
        })
        .max()
        .unwrap_or(0);

    for (i, (name, child)) in node.children.iter().enumerate() {
        let last = i + 1 == len;

        let branch = if last { "└── " } else { "├── " };
        let next_prefix = if last { "    " } else { "│   " };

        if let Some(size) = child.size {
            let display = format!("{name:<name_width$}");
            let meta = human_size(size).dim();

            println!("  {}{}{}  {}", prefix, branch, display, meta);
        } else {
            let display = format!("{}/", name);
            let display = format!("{display:<name_width$}").header();

            let (size, files) = tree_stats(child);
            let file_word = if files == 1 { "file" } else { "files" };
            let meta = format!("{} · {} {}", human_size(size), files, file_word).dark();

            println!("  {}{}{}  {}", prefix, branch, display, meta);
        }

        let next = format!("{}{}", prefix, next_prefix);
        print_tree_node(child, &next);
    }
}

fn tree_stats(node: &TreeNode) -> (usize, usize) {
    if let Some(size) = node.size {
        return (size, 1);
    }

    let mut total_size = 0;
    let mut total_files = 0;

    for child in node.children.values() {
        let (size, files) = tree_stats(child);
        total_size += size;
        total_files += files;
    }

    (total_size, total_files)
}

fn human_size(bytes: usize) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;

    let b = bytes as f64;

    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

pub fn help() -> Vec<String> {
    vec![
        "fossil list".header(),
        "show the files inside a directory fossil".bold(),
        "".into(),
        "usage".header(),
        "  fossil list <file.fossil>".into(),
        "".into(),
        "arguments".header(),
        "  <file.fossil>     directory archive to list".into(),
        "".into(),
        "examples".header(),
        "  fossil list archive.fossil".into(),
        "  cat archive.fossil | fossil list -".into(),
    ]
}
