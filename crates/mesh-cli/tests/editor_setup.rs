//! The editor setup guide's Neovim configuration is exactly
//! `editors/neovim/init.lua`, the file `editors/neovim/verify.sh` checks
//! in a real Neovim, so the guide can't drift from what was verified.

use std::fs;
use std::path::Path;

fn read(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("should read {}: {err}", path.display()))
}

#[test]
fn the_guides_neovim_configuration_is_the_verified_one() {
    let guide = read("docs/guides/editor-setup.md");
    let blocks: Vec<&str> = guide
        .split("```lua\n")
        .skip(1)
        .map(|rest| rest.split("```").next().expect("a closing fence"))
        .collect();
    assert_eq!(blocks.len(), 1, "the guide has one Lua block");
    assert_eq!(blocks[0], read("editors/neovim/init.lua"));
}
