mod commands;
mod git_repo;

use std::{
    collections::HashSet,
    fs,
    hash::Hash,
    path::{Path, PathBuf},
};

use crate::{
    commands::{cat_file, hash_object, repo_create, repo_find},
    git_repo::{GitObject, GitRepository, GitTree, log_graphviz},
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

pub fn cmd_checkout(commit: String, path: String) -> Result<(), String> {
    let repo = repo_find()?;
    let sha = repo.object_find(commit, "".to_string());
    let obj = repo
        .object_read(&sha)
        .ok_or("Unable to read the git object")?;
    let tree = match obj {
        GitObject::Tree(tree) => tree,
        GitObject::Commit(commit) => {
            let hash = commit
                .data
                .get("tree")
                .ok_or("Malformed Commit")?
                .first()
                .ok_or("Malformed Commit")?;
            let obj = repo.object_read(hash).ok_or("Unable to read the tree")?;
            if let GitObject::Tree(tree) = obj {
                tree
            } else {
                Err("Expected tree")?
            }
        }
        _ => Err("Expected commit or tree")?,
    };

    let path: PathBuf = path.into();
    if path.exists() {
        // Exist
        if !path.is_dir() {
            Err("Not a directory")?;
        }
        if path.read_dir().map_err(|_| "Unable to read dir")?.count() > 0 {
            Err("The directory is not empty")?;
        }
    } else {
        // Need to create one
        fs::create_dir_all(&path).map_err(|_| "Failed to create the directory")?;
    }

    checkout_tree(&repo, &tree, &path)?;
    Ok(())
}

fn checkout_tree(repo: &GitRepository, tree: &GitTree, path: &Path) -> Result<(), String> {
    for items in &tree.items {
        let obj = repo.object_read(&items.2).ok_or("Can't read object")?;
        let path = path.join(&items.1);
        match obj {
            GitObject::Blob(blob) => {
                fs::write(path, blob.buffer).map_err(|_| "Can't write into the file")?;
            }
            GitObject::Tree(tree) => {
                fs::create_dir(&path).map_err(|_| "Failed to create the directory")?;
                checkout_tree(repo, &tree, &path)?;
            }
            _ => Err("Malformed tree")?,
        }
    }
    Ok(())
}
