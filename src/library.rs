#![allow(dead_code)]

use colored::Colorize;
use hashbrown::HashMap;
use piglog::prelude::*;
use std::io;
use std::process::Command;
use users::get_current_username;

use crate::convert::*;
use crate::generation::Generation;

#[derive(PartialEq)]
pub enum HistoryMode {
    Remove,
    Add,
}

pub struct History {
    pub mode: HistoryMode,
    pub line: String,
}

pub fn abort() {
    // Try not to use this function!
    std::process::exit(1);
}

pub fn run_command(command: &str) -> bool {
    match Command::new("bash").args(["-c", command]).status() {
        Ok(o) => o,
        Err(_e) => return false,
    }
    .success()
}

pub fn run_command_with_output(command: &str) -> Option<String> {
    match Command::new("bash").args(["-c", command]).output() {
        Ok(output) => {
            if !output.status.success() {
                return None;
            }
            String::from_utf8(output.stdout).ok()
        }
        Err(_e) => None,
    }
}

pub fn cut(full: &str, fword: u32, dword: char) -> String {
    let vecced = str_to_string_vec(full, dword.to_string().as_str());

    let fspot = fword - 1;

    if vecced.len() < fword as usize {
        return String::from("");
    }

    vecced[fspot as usize].to_string()
}

pub fn sed(full: &str, replace: &str, with: &str) -> String {
    let mut phrase_vec: Vec<String> = Vec::new();

    for i in full.split(replace) {
        phrase_vec.push(i.to_string());
    }

    let mut phrase = String::new();

    for i in 0..phrase_vec.len() {
        phrase.push_str(phrase_vec[i].as_str());

        if i < phrase_vec.len() - 1 {
            phrase.push_str(with);
        }
    }

    phrase
}

pub fn name_from_path(path: &str) -> String {
    let converted = str_to_string_vec(path, "/");

    converted[converted.len() - 1].to_string()
}

pub fn custom_error(error: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, error)
}

pub fn is_root_user() -> bool {
    if username() == "root" {
        true
    } else {
        false
    }
}

pub fn username() -> String {
    let username: String = match get_current_username() {
        Some(uname) => uname.to_str().unwrap().to_string(),
        None => panic!("Unable to get username!"),
    };

    username
}

pub fn remove_array_duplicates<T: Clone + PartialEq>(dup_vec: &[T]) -> Vec<T> {
    let mut new_vec: Vec<T> = Vec::new();

    for i in dup_vec.iter() {
        if new_vec.contains(i) == false {
            new_vec.push(i.clone());
        }
    }

    new_vec
}

pub fn history_gen(gen_1: &Generation, gen_2: &Generation) -> HashMap<String, Vec<History>> {
    let mut history_map: HashMap<String, Vec<History>> = HashMap::new();

    for i in gen_2.managers.keys() {
        let items_2 = gen_2.managers.get(i).unwrap();

        match gen_1.managers.get(i) {
            Some(items_1) => {
                history_map.insert(i.to_string(), history(&items_1.items, &items_2.items))
            }
            None => history_map.insert(
                i.to_string(),
                items_2
                    .items
                    .iter()
                    .map(|x| History {
                        mode: HistoryMode::Add,
                        line: x.to_string(),
                    })
                    .collect(),
            ),
        };
    }

    for i in gen_1.managers.keys() {
        let items_1 = gen_1.managers.get(i).unwrap();

        match gen_2.managers.get(i) {
            Some(_) => (),
            None => {
                history_map.insert(
                    i.to_string(),
                    items_1
                        .items
                        .iter()
                        .map(|x| History {
                            mode: HistoryMode::Remove,
                            line: x.to_string(),
                        })
                        .collect(),
                );
            }
        };
    }

    history_map
}

pub fn print_history_gen(history: &HashMap<String, Vec<History>>) {
    for i in history.keys() {
        piglog::info!("{}:", i);

        print_history(history.get(i).unwrap());

        println!("");
    }
}

pub fn print_history(diff_vec: &Vec<History>) {
    for i in diff_vec.iter() {
        match i.mode {
            HistoryMode::Add => println!("{}", format!("+ {}", i.line).bright_green().bold()),
            HistoryMode::Remove => println!("{}", format!("- {}", i.line).bright_red().bold()),
        };
    }
}

pub fn history(array_1: &[String], array_2: &[String]) -> Vec<History> {
    let lines_1 = remove_array_duplicates(array_1);
    let lines_2 = remove_array_duplicates(array_2);

    let mut history_vec: Vec<History> = Vec::new();

    for i in lines_1.iter() {
        if i.trim() != "" {
            if lines_2.contains(i) == false {
                history_vec.push(History {
                    mode: HistoryMode::Remove,
                    line: i.to_string(),
                });
            }
        }
    }

    for i in lines_2.iter() {
        if i.trim() != "" {
            if lines_1.contains(i) == false {
                history_vec.push(History {
                    mode: HistoryMode::Add,
                    line: i.to_string(),
                });
            }
        }
    }

    history_vec
}
