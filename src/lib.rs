#![feature(c_unwind)]

use std::{path::{PathBuf, Component, Path}};

#[macro_use] extern crate gmod;

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
    if lua.get_type(-1) != "table" { // if nil or anything else return default
        return DialogOptions {
            title: "Open File".to_string(),
            path: PathBuf::new(),
            filter: Vec::new(),
            is_folder: false,
            allow_multiple: false,
        };
    }

    lua.get_table(-1);
    lua.get_field(-1, lua_string!("is_folder"));
    let is_folder = lua.get_boolean(-1);

    lua.get_table(-2);
    lua.get_field(-1, lua_string!("allow_multiple"));
    let allow_multiple = lua.get_boolean(-1) && !is_folder;

    lua.get_table(-2);
    lua.get_field(-1, lua_string!("title"));
    let title = match lua.get_string(-1) {
        Some(s) => s.into_owned(),
        None => String::from(if is_folder { "Select Folder" } else { if allow_multiple { "Select Files" } else { "Select File" }}),
    };

    lua.get_table(-2);
    lua.get_field(-1, lua_string!("path"));
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

    lua.get_table(-2);
    lua.get_field(-1, lua_string!("filters"));
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

#[lua_function]
unsafe fn file_dialog_sync(lua: gmod::lua::State) -> i32 {
    let opts = get_dialog_opts(lua);
    let dialog = rfd::FileDialog::new()
        .set_title(opts.title.as_str())
        .add_filter("lua", &["txt", "lua"])
        //.add_filter("rust", &["rs", "toml"])
        .set_directory(&opts.path);

    if opts.allow_multiple {
        match dialog.pick_files() {
            Some(files) => {
                lua.new_table();
                for (i, file) in files.iter().enumerate() {
                    lua.push_string(file.to_str().unwrap());
                    lua.push_integer(i as isize);
                }
            },
            None => lua.push_boolean(false),
        }
    } else {
        let res = if opts.is_folder { dialog.pick_folder() } else { dialog.pick_file() };
        match res {
            Some(path) => lua.push_string(path.to_str().unwrap()),
            None => lua.push_boolean(false),
        }
    }

    1
}

#[gmod13_open]
unsafe fn gmod13_open(lua: gmod::lua::State) -> i32 {
    lua.get_global(lua_string!("file"));

    lua.push_function(file_dialog_sync);
    lua.set_field(-2, lua_string!("OpenDialog"));

    0
}

#[gmod13_close]
unsafe fn gmod13_close(_: gmod::lua::State) -> i32 {
    0
}