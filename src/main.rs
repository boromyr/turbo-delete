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

    // Verifica se ci sono argomenti
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

    // Ignora arg[0] (nome del programma) e processa tutti gli altri argomenti
    let paths = &args[1..];
    let mut success_count = 0;
    let mut error_count = 0;

    for target_path in paths {
        let mut path_str = target_path.to_string();

        // Rimuovi le virgolette se presenti
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

        println!("Deleting: {}", path.display().to_string().bright_green());

        if path.is_file() {
            // Gestione cancellazione singolo file
            set_writable(&path);
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
            // Gestione cancellazione directory
            let mut tree: BTreeMap<u64, Vec<PathBuf>> = BTreeMap::new();

            // Ottieni lista completa di entries (file e cartelle)
            let entries: Vec<DirEntry<((), ())>> = match jwalk::WalkDir::new(&path)
                .follow_links(true)
                .skip_hidden(false)
                .into_iter()
                .par_bridge()
                .map(|v| v.ok())
                .filter(Option::is_some)
                .collect::<Option<Vec<_>>>()
            {
                Some(entries) => entries,
                None => {
                    eprintln!(
                        "{} {} {}",
                        " ERROR ".on_color(AnsiColors::BrightRed).black(),
                        "Failed to read directory:".bright_yellow(),
                        path_str
                    );
                    error_count += 1;
                    continue;
                }
            };

            let bar = ProgressBar::new(entries.len() as u64);

            for entry in entries {
                tree.entry(entry.depth as u64)
                    .or_insert_with(Vec::new)
                    .push(entry.path());
            }

            let pool = ThreadPool::default();
            let mut handles = vec![];

            // Cancella prima i file, poi le directory (in ordine inverso di profondità)
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
                // Tenta di risolvere problemi di permessi e cancella di nuovo
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

    // Riassunto finale
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
}
