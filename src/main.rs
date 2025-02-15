/*
  Copyright 2022 Tejas Ravishankar

  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing, software
  distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions and
  limitations under the License.
*/

use indicatif::ProgressBar;
use jwalk::DirEntry;
use owo_colors::{AnsiColors, OwoColorize};
use rayon::iter::{IntoParallelRefIterator, ParallelBridge, ParallelIterator};
use rusty_pool::ThreadPool;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Instant,
};

// change a file to be writable
pub fn set_writable(path: &Path) {
    let mut perms = std::fs::metadata(path).unwrap().permissions();
    perms.set_readonly(false);
    std::fs::set_permissions(path, perms).unwrap();
}

pub fn set_folder_writable(path: &Path) {
    // get complete list of folders and files
    let entries: Vec<DirEntry<((), ())>> = jwalk::WalkDir::new(&path)
        .follow_links(true)
        .skip_hidden(false)
        .into_iter()
        .filter(|v| v.as_ref().map(|e| e.path().exists()).unwrap_or(false))
        .map(|v| {
            v.unwrap_or_else(|err| {
                eprintln!(
                    "{} {}",
                    " ERROR ".on_color(AnsiColors::BrightRed).black(),
                    err
                );
                std::process::exit(1);
            })
        })
        .collect();

    entries.par_iter().for_each(|entry| {
        set_writable(&entry.path());
    });
}

fn delete_entry(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

fn main() {
    let start = Instant::now();

    let args = std::env::args().collect::<Vec<String>>();

    let mut target_path: String = args
        .get(1)
        .unwrap_or_else(|| {
            eprintln!(
                "{} {}\n\n{}:\n{} {}\n{} {}",
                " ERROR ".on_color(AnsiColors::BrightRed).black(),
                "Please provide a path.".bright_yellow(),
                "Examples".underline(),
                "turbodelete".bright_cyan(),
                "./node_modules/".bright_black(),
                "turbodelete".bright_cyan(),
                "./file.txt".bright_black(),
            );
            std::process::exit(1);
        })
        .to_string();

    if target_path.ends_with('"') {
        target_path.pop();
    }

    let path = PathBuf::from(&target_path);

    if !path.exists() {
        eprintln!(
            "{} {}",
            " ERROR ".on_color(AnsiColors::BrightRed).black(),
            "Path does not exist.".bright_yellow()
        );
        std::process::exit(1);
    }

    if path.is_file() {
        // Handle single file deletion
        println!(
            "Deleting file: {}",
            path.display().to_string().bright_green()
        );
        set_writable(&path);
        if let Err(err) = delete_entry(&path) {
            eprintln!(
                "{} {}",
                " ERROR ".on_color(AnsiColors::BrightRed).black(),
                err
            );
            std::process::exit(1);
        }
        println!(
            "File deleted successfully in {} seconds",
            start.elapsed().as_secs_f32().to_string().bright_red()
        );
        return;
    }

    // Handle directory deletion
    let mut tree: BTreeMap<u64, Vec<PathBuf>> = BTreeMap::new();

    // get complete list of entries (both files and folders)
    let entries: Vec<DirEntry<((), ())>> = jwalk::WalkDir::new(&path)
        .follow_links(true)
        .skip_hidden(false)
        .into_iter()
        .par_bridge()
        .map(|v| v.unwrap())
        .collect();

    let bar = ProgressBar::new(entries.len() as u64);

    for entry in entries {
        tree.entry(entry.depth as u64)
            .or_insert_with(Vec::new)
            .push(entry.path());
    }

    let pool = ThreadPool::default();
    let mut handles = vec![];

    // Delete files first, then directories (in reverse depth order)
    for (_, entries) in tree.iter().rev() {
        let entries = entries.clone();
        let bar = bar.clone();

        handles.push(pool.evaluate(move || {
            entries.par_iter().for_each(|entry| {
                let _ = delete_entry(entry);
                bar.inc(1);
            });
        }));
    }

    for handle in handles {
        handle.await_complete();
    }

    if path.exists() {
        // Try to fix permission issues and delete again
        set_folder_writable(&path);
        if let Err(err) = delete_entry(&path) {
            eprintln!(
                "{} {}",
                " ERROR ".on_color(AnsiColors::BrightRed).black(),
                err
            );
            std::process::exit(1);
        }
    }

    println!(
        "Deletion completed in {} seconds",
        start.elapsed().as_secs_f32().to_string().bright_green()
    );
}
