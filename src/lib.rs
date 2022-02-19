#![feature(c_unwind)]

use std::{path::{PathBuf, Component, Path}, cell::RefCell};
use futures::{task::{LocalSpawnExt}, future::Either};
use futures::executor::{LocalPool};

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

struct DialogOptions {
    title: String,
    path: PathBuf,
    filter: Vec<(String, Vec<String>)>,
    is_folder: bool,
    allow_multiple: bool, // only works with files
}

unsafe fn get_dialog_opts(lua: gmod::lua::State) -> DialogOptions {
    lua.check_table(1);
    lua.check_function(2);

    //lua.from_reference(1);

    lua.get_field(1, lua_string!("is_folder"));
    let is_folder = lua.get_boolean(-1);

    lua.get_field(1, lua_string!("allow_multiple"));
    let allow_multiple = lua.get_boolean(-1) && !is_folder;

    lua.get_field(1, lua_string!("title"));
    let title = match lua.get_string(-1) {
        Some(s) => s.into_owned(),
        None => String::from(if is_folder { "Select Folder" } else { if allow_multiple { "Select Files" } else { "Select File" }}),
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

    lua.get_field(1, lua_string!("filters"));
    if lua.get_type(-1) == "table" {
        //while lua.next(-1)
        //s.split(";").map(|s| s.to_string()).collect()
    }

    DialogOptions {
        title,
        path,
        filter: Vec::new(),
        is_folder,
        allow_multiple,
    }
}

fn is_bad_path(path: &PathBuf) -> bool {
    let path_str = path.to_str().unwrap();
    let game_dir = get_game_dir();
    let game_dir_str = game_dir.as_str();
    if path_str.starts_with(game_dir_str) && path_str.contains("..") {
        return false;
    }

    true
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

#[lua_function]
unsafe fn fs_dialog(lua: gmod::lua::State) -> i32 {
    let lua = lua;
    let opts = get_dialog_opts(lua);
    let dialog = rfd::AsyncFileDialog::new()
        .set_title(opts.title.as_str())
        .add_filter("lua", &["txt", "lua"])
        //.add_filter("rust", &["rs", "toml"])
        .set_directory(&opts.path);

    let spawner = POOL.with(|pool| pool.borrow().spawner());
    let res = match opts {
        opts if opts.allow_multiple => {
            let task = dialog.pick_files();
            spawner.spawn_local(async move {
                let res = task.await;
                match res {
                    Some (handles) => {

                    },
                    None => (),
                }
            })
        },
        _ => {
            let task = if opts.is_folder {
                Either::Left(dialog.pick_folder())
            } else {
                Either::Right(dialog.pick_file())
            };

            spawner.spawn_local(async move {
                let res = task.await;
                match res {
                    Some (handle) => {
                        let path = handle.path().to_path_buf();
                        if is_bad_path(&path) {
                            lua.push_boolean(false)
                        } else {
                            lua.push_string(path.to_str().unwrap());
                        }
                    },
                    None => lua.push_boolean(false),
                }
            })
        },
    };

    match res {
        Ok(_) => (),
        Err(e) => lua.error(e.to_string()),
    }

    0
}

unsafe fn initialize_polling(lua: gmod::lua::State) {
    lua.get_global(lua_string!("timer"));
    lua.get_field(-1, lua_string!("Create"));
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