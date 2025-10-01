mod commands;
mod git_repo;

use std::{collections::HashSet, hash::Hash};

use crate::{
    commands::{cat_file, hash_object, repo_create, repo_find},
    git_repo::{GitObject, GitRepository, log_graphviz},
};

pub fn cmd_init(path: String) -> Result<(), String> {
    repo_create(path.into()).map(|_| ())
}

pub fn cmd_cat_file(obj_type: String, object: String) -> Result<(), String> {
    cat_file(repo_find()?, object, obj_type)
}

pub fn cmd_hash_object(write: bool, obj_type: String, path: String) -> Result<(), String> {
    let repo = if write { Some(repo_find()?) } else { None };

    let shasum = hash_object(
        repo,
        obj_type,
        std::fs::read(path).map_err(|e| e.to_string())?,
    )?;
    println!("{}", shasum);
    Ok(())
}

#[cfg(test)]
mod tests {
    // use super::*;

    #[test]
    fn parse_command() {}
}

pub fn cmd_log(commit: String) -> Result<(), String> {
    let repo = repo_find()?;

    println!("digraph ozlog{{");
    println!("node[shape=rect]");
    log_graphviz(
        &repo,
        &repo.object_find(commit, "commit".to_string()),
        &mut HashSet::new(),
    )?;
    println!("}}");

    Ok(())
}

pub fn cmd_list_tree(recursive: bool, tree: String) -> Result<(), String> {
    let repo = repo_find()?;
    ls_tree(&repo, tree, recursive, "");
    Ok(())
    // todo!()
}

pub(self) fn ls_tree(repo: &GitRepository, tree: String, recursive: bool, prefix: &str) {
    let sha = repo.object_find(tree, "tree".into());
    let obj = repo.object_read(&sha);

    if let Some(GitObject::Tree(obj)) = obj {
        for entry in obj.items {
            let obj_type = match &entry.0[0..2] {
                [b'0', b'4'] => "tree",
                [b'1', b'0'] => "blob", // Regular file
                [b'1', b'2'] => "blob", // symlink, pointed to a blob file
                [b'1', b'6'] => "commit",
                _ => panic!("Unknown type of file"),
            };

            let is_tree = obj_type == "tree";
            if !recursive || !is_tree {
                println!(
                    "{} {} {}\t{}",
                    String::from_utf8(entry.0.into()).expect("Something wrong with the header"),
                    obj_type,
                    entry.2,
                    prefix.to_string() + if prefix.is_empty() { "" } else { "/" } + &entry.1
                );
            } else {
                ls_tree(
                    repo,
                    entry.2,
                    recursive,
                    &(prefix.to_string() + "/" + &entry.1),
                );
            }
        }
    }
}
