#![feature(c_unwind)]

use std::{path::{PathBuf, Component, Path}, cell::RefCell};
use futures::{task::{LocalSpawnExt}, future::Either};
use futures::executor::{LocalPool};
use gmod::lua::LuaReference;
use rfd::FileHandle;

#[macro_use]
extern crate gmod;

static GMOD_PATH_FOLDER: &str = "steamapps/common/GarrysMod";
fn get_game_dir() -> String {
    let exe_path = std::env::current_exe().unwrap();
    let str_path = String::from(exe_path.as_os_str().to_str().unwrap()).replace("\\", "/");
    let index = str_path.find(GMOD_PATH_FOLDER).unwrap();

    let mut path_str = String::from(&str_path[0..index + &GMOD_PATH_FOLDER.len()]);
    path_str.push_str("/garrysmod/");
    path_str.replace("\\", "/")
}

fn is_path_transversal(path: &PathBuf) -> bool {
    path.components().into_iter().any(|c| c == Component::ParentDir)
}

fn is_bad_path(path: &PathBuf) -> bool {
    let path_str = path.to_str().unwrap();
    let game_dir = get_game_dir();
    let game_dir_str = game_dir.as_str();
    if path_str.replace("\\", "/").starts_with(game_dir_str) && !is_path_transversal(path) {
        return false;
    }

    true
}

struct DialogOptions {
    title: String,
    path: PathBuf,
    filters: Vec<(String, Vec<String>)>,
    is_folder: bool,
    allow_multiple: bool, // only works with files
    is_save: bool, // only works with files, doesnt allow multiple
    default_save_name: Option<String>, // for saving
    on_completed: Option<i32>,
}

impl DialogOptions {
    unsafe fn validate(&self, lua: gmod::lua::State) {
        if self.is_folder && self.is_save {
            lua.error("cannot save folders!");
        }

        if self.allow_multiple && self.is_save {
            lua.error("cannot save multiple files!");
        }

        if self.on_completed.is_none() {
            lua.error("no on_completed callback!");
        }
    }

    unsafe fn from_lua_stack(lua: gmod::lua::State) -> DialogOptions {
        lua.check_table(1);

        lua.get_field(1, lua_string!("is_folder"));
        let is_folder = lua.get_boolean(-1);

        lua.get_field(1, lua_string!("save"));
        let is_save = lua.get_boolean(-1);

        lua.get_field(1, lua_string!("default_save_name"));
        let default_save_name = match lua.get_string(-1) {
            Some(s) => Some(String::from(s)),
            None => None,
        };

        lua.get_field(1, lua_string!("allow_multiple"));
        let allow_multiple = lua.get_boolean(-1) && !is_folder && !is_save;

        lua.get_field(1, lua_string!("title"));
        let title = match lua.get_string(-1) {
            Some(s) => s.into_owned(),
            None if is_save => String::from("Save File"),
            _ => String::from(if is_folder { "Select Folder" } else { if allow_multiple { "Select Files" } else { "Select File" }}),
        };

        lua.get_field(1, lua_string!("path"));
        let input_path = lua.get_string(-1);
        let root_path = get_game_dir();
        let path = {
            let base_path = Path::new(&root_path);
            match input_path {
                Some(s) => {
                    let local_path = PathBuf::from(s.as_ref());
                    if !is_path_transversal(&local_path) {
                        base_path.join(local_path)
                    } else {
                        base_path.to_path_buf()
                    }
                },
                None => base_path.to_path_buf(),
            }
        };

        let on_completed = {
            lua.get_field(1, lua_string!("on_completed"));
            if lua.is_function(-1) {
                Some(lua.reference())
            } else {
                None
            }
        };

        let mut filters: Vec<(String, Vec<String>)> = Vec::new();
        lua.get_field(1, lua_string!("filters"));
        if lua.get_type(-1) == "table" {
            lua.push_nil();
            while lua.next(-2) != 0 {
                let filter_name = lua.get_string(-2);
                let filter_exts = lua.get_string(-1);

                filters.push(
                    (String::from(filter_name.unwrap_or_default().as_ref()),
                    filter_exts.unwrap_or_default()
                        .split(';')
                        .map(|s| s.trim().to_owned())
                        .collect()
                    )
                );

                lua.pop();
            }
        }

        DialogOptions {
            title,
            path,
            filters,
            is_folder,
            allow_multiple,
            on_completed,
            is_save,
            default_save_name,
        }
    }
}

thread_local! {
    static POOL: RefCell<LocalPool> = RefCell::new(LocalPool::new());
}

#[lua_function]
unsafe fn poll_dialog_events(_: gmod::lua::State) -> i32 {
    POOL.with(|pool| {
        pool.borrow_mut().run_until_stalled();
    });

    0
}

unsafe fn process_dialog_result(lua: gmod::lua::State,  on_completed: LuaReference, res: Option<FileHandle>) {
    lua.from_reference(on_completed);
    match res {
        Some (handle) => {
            let path = handle.path().to_path_buf();
            if is_bad_path(&path) {
                lua.push_boolean(false);
                lua.push_string("Path out of game directory");
                lua.call(2, 0);
            } else {
                lua.push_string(path.to_str().unwrap());
                lua.call(1, 0);
            }
        },
        None => {
            lua.push_boolean(false);
            lua.push_string("Invalid path");
            lua.call(2, 0);
        },
    }
    lua.dereference(on_completed);
}

#[lua_function]
unsafe fn fs_dialog(lua: gmod::lua::State) -> i32 {
    let lua = lua;
    let opts = DialogOptions::from_lua_stack(lua);
    opts.validate(lua);

    let on_completed = opts.on_completed.unwrap();
    let mut dialog = rfd::AsyncFileDialog::new()
        .set_title(opts.title.as_str())
        .set_directory(&opts.path);

    if opts.is_save && opts.default_save_name.is_some() {
        dialog = dialog.set_file_name(&opts.default_save_name.as_ref().unwrap().as_str());
    }

    for filter in &opts.filters {
        let fiters: Vec<&str> = filter.1.iter().map(|s| s.as_str()).collect();
        dialog = dialog.add_filter(filter.0.as_str(), fiters.as_slice());
    }

    let spawner = POOL.with(|pool| pool.borrow().spawner());
    let res = match opts {
        opts if opts.allow_multiple && !opts.is_save => {
            let task = dialog.pick_files();
            spawner.spawn_local(async move {
                let res = task.await;
                lua.from_reference(on_completed);
                lua.new_table();
                match res {
                    Some (handles) => {
                        for (i, handle) in handles.iter().enumerate() {
                            let path = handle.path().to_path_buf();
                            if !is_bad_path(&path) {
                                lua.push_string(path.to_str().unwrap());
                                lua.push_integer(i as isize);
                                lua.set_table(-3);
                            }
                        }
                    },
                    None => (),
                }
                lua.call(1, 0);
                lua.dereference(on_completed);
            })
        },
        opts if opts.is_save && !opts.is_folder => {
            let task = dialog.save_file();

            spawner.spawn_local(async move {
                let res = task.await;
                process_dialog_result(lua, on_completed, res);
            })
        },
        _ => {
            let task = {
                if opts.is_folder {
                    Either::Left(dialog.pick_folder())
                } else {
                    Either::Right(dialog.pick_file())
                }
            };

            spawner.spawn_local(async move {
                let res = task.await;
                process_dialog_result(lua, on_completed, res);
            })
        },
    };

    if let Err(e) = res {
        lua.from_reference(on_completed);
        lua.push_boolean(false);
        lua.push_string(e.to_string().as_str());
        lua.call(2, 0);
        lua.dereference(on_completed);
    }

    0
}

unsafe fn initialize_polling(lua: gmod::lua::State) {
    lua.get_global(lua_string!("timer"));
    lua.get_field(-1, lua_string!("Create"));
    lua.push_string("FSDialogPolling");
    lua.push_number(0.1);
    lua.push_integer(0);
    lua.push_function(poll_dialog_events);
    lua.pcall(4, 0, 0);
    lua.pop_n(2);
}

#[gmod13_open]
unsafe fn gmod13_open(lua: gmod::lua::State) -> i32 {
    lua.get_global(lua_string!("file"));
    lua.push_function(fs_dialog);
    lua.set_field(-2, lua_string!("OpenDialog"));
    lua.pop_n(2);

    initialize_polling(lua);

    0
}

#[gmod13_close]
unsafe fn gmod13_close(_: gmod::lua::State) -> i32 {
    0
}