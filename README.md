# gm_fsdialog
**Open your operating system's file browser from Garry's Mod.**

## Examples
Opening a file:
```lua
require("fsdialog")

-- prompts the user to open a lua file
file.OpenDialog({
	path = "lua",
	filters = {
		["scripts"] = "lua" -- 'scripts' is the name of the filter, "lua" is the extension that it allows
	},
	on_completed = function(path, err)
		if not path then error(err) end

		print("User picked file: " .. path)
	end,
})
```

Opening multiple files:
```lua
require("fsdialog")

-- prompts the user to open his pac3 files in the data folder
file.OpenDialog({
	path = "data/pac3/",
	filters = {
		["pac3"] = "txt"
	},
	on_completed = function(paths)
		print("User selected the following files:")
		PrintTable(paths)
	end,
})
```

Opening a folder:
```lua
require("fsdialog")

-- prompts the user to open a lua file
file.OpenDialog({
	path = "data",
	is_folder = true,
	on_completed = function(path, err)
		if not path then error(err) end

		print("User selected folder: " .. path)
	end,
})
```

Saving a file:
```lua
require("fsdialog")

file.OpenDialog({
	path = "lua",
	is_save = true,
	default_save_name = "my_script.lua",
	filters = {
		["scripts"] = "lua"
	},
	on_completed = function(path, err)
		if not path then error(err) end

		print("User wants to save the file at: " .. path)
	end,
})
```

## Building / Compiling
- Open a terminal
- Install **cargo** if you dont have it (on Windows => https://win.rustup.rs) (on Linux/Macos => curl https://sh.rustup.rs -sSf | sh)
- Get [git](https://git-scm.com/downloads) or download the archive for the repository directly
- `git clone https://github.com/Earu/gm_fsdialog` (ignore this if you've downloaded the archive)
- Run `cd gm_fsdialog`
- `cargo build`
- Go in `target/debug` and rename the binary according to your branch and realm (gmsv_fsdialog_win64.dll, gmcl_fsdialog_win64.dll, gmsv_fsdialog_linux.dll, gmcl_fsdialog_linux.dll, gmcl_fsdialog_osx64.dll)
- Put the binary in your gmod `lua/bin` directory

*Note: Even on other platforms than Windows the extension of your modules **need** to be **.dll***
