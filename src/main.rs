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
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Instant,
};

// change a file to be writable
pub fn set_writable(path: &Path) -> std::io::Result<()> {
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_readonly(false);
    std::fs::set_permissions(path, perms)
}

pub fn set_folder_writable(path: &Path) {
    let entries: Vec<DirEntry<((), ())>> = jwalk::WalkDir::new(path)
        .follow_links(true)
        .skip_hidden(false)
        .into_iter()
        .filter_map(|v| v.ok())
        .filter(|e| e.path().exists())
        .collect();

    entries.par_iter().for_each(|entry| {
        if let Err(err) = set_writable(&entry.path()) {
            eprintln!(
                "{} {} {}",
                " ERROR ".on_color(AnsiColors::BrightRed).black(),
                "Failed to set writable:".bright_yellow(),
                err
            );
        }
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

    if args.len() <= 1 {
        eprintln!(
            "{} {}\n\n{}:\n{} {}\n{} {}\n{} {}",
            " ERROR ".on_color(AnsiColors::BrightRed).black(),
            "Please provide at least one path.".bright_yellow(),
            "Examples".underline(),
            "turbodelete".bright_cyan(),
            "./node_modules/".bright_black(),
            "turbodelete".bright_cyan(),
            "./file1.txt ./file2.txt".bright_black(),
            "turbodelete".bright_cyan(),
            "\"path with spaces\" another_path".bright_black(),
        );
        std::process::exit(1);
    }

    let paths = &args[1..];
    let mut success_count = 0;
    let mut error_count = 0;

    for target_path in paths {
        let mut path_str = target_path.to_string();

        if path_str.starts_with('"') && path_str.ends_with('"') {
            path_str = path_str[1..path_str.len() - 1].to_string();
        }

        let path = PathBuf::from(&path_str);

        if !path.exists() {
            eprintln!(
                "{} {} {}",
                " ERROR ".on_color(AnsiColors::BrightRed).black(),
                "Path does not exist:".bright_yellow(),
                path_str
            );
            error_count += 1;
            continue;
        }

        println!("Deleting: {}", path.display().bright_green());

        if path.is_file() {
            if let Err(err) = set_writable(&path) {
                eprintln!(
                    "{} {} {}",
                    " ERROR ".on_color(AnsiColors::BrightRed).black(),
                    "Failed to set writable:".bright_yellow(),
                    err
                );
            }
            if let Err(err) = delete_entry(&path) {
                eprintln!(
                    "{} {} {}",
                    " ERROR ".on_color(AnsiColors::BrightRed).black(),
                    err,
                    path_str
                );
                error_count += 1;
                continue;
            }
            success_count += 1;
        } else {
            let mut tree: BTreeMap<u64, Vec<PathBuf>> = BTreeMap::new();

            let entries: Vec<DirEntry<((), ())>> = jwalk::WalkDir::new(&path)
                .follow_links(true)
                .skip_hidden(false)
                .into_iter()
                .filter_map(|v| v.ok())
                .collect();

            let bar = ProgressBar::new(entries.len() as u64);

            for entry in entries {
                tree.entry(entry.depth as u64)
                    .or_default()
                    .push(entry.path());
            }

            // Cancella per livello in ordine inverso (foglie prima delle radici).
            // Ogni livello viene completato prima di passare al successivo,
            // eliminando la race condition della versione originale.
            for (_, level_entries) in tree.iter().rev() {
                level_entries.par_iter().for_each(|entry| {
                    let _ = delete_entry(entry);
                    bar.inc(1);
                });
            }

            bar.finish_and_clear();

            if path.exists() {
                set_folder_writable(&path);
                if let Err(err) = delete_entry(&path) {
                    eprintln!(
                        "{} {} {}",
                        " ERROR ".on_color(AnsiColors::BrightRed).black(),
                        err,
                        path_str
                    );
                    error_count += 1;
                    continue;
                }
            }
            success_count += 1;
        }
    }

    if success_count > 0 && error_count == 0 {
        println!(
            "Deletion completed successfully for {} items in {} seconds",
            success_count.to_string().bright_green(),
            start.elapsed().as_secs_f32().to_string().bright_yellow()
        );
    } else {
        println!(
            "Deletion completed with {} successes and {} errors in {} seconds",
            success_count.to_string().bright_green(),
            error_count.to_string().bright_red(),
            start.elapsed().as_secs_f32().to_string().bright_yellow()
        );
    }

    println!("\nPress ENTER to exit...");
    let mut _input = String::new();
    std::io::stdin().read_line(&mut _input).ok();
}