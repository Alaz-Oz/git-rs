mod commands;
mod git_repo;

use std::{collections::HashSet, hash::Hash};

use crate::{
    commands::{cat_file, hash_object, repo_create, repo_find},
    git_repo::{GitRepository, log_graphviz},
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

    todo!()
}
