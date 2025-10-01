use crate::git_repo::{
    GitBlob, GitCommit, GitObject, GitRepository, GitTag, GitTree, Serializable,
};
use std::path::PathBuf;

pub(crate) fn repo_create(path: PathBuf) -> Result<GitRepository, String> {
    let repo = GitRepository::new(path.into(), true).unwrap();
    if repo.worktree.exists() {
        if !repo.worktree.is_dir() {
            return Err("Not a directory".to_string());
        }
        if repo.git_dir.exists() {
            let x = std::fs::read_dir(&repo.git_dir); // Assuming it exist
            if x.is_err() || x.unwrap().next().is_some() {
                return Err("Not an empty repo".to_string());
            }
        }
    } else {
        std::fs::create_dir_all(&repo.worktree).expect("Unable to create the project root");
    }

    // Create Required Directories
    repo.create_repo_dir("branches".into())?;
    repo.create_repo_dir("objects".into())?;
    repo.create_repo_dir(["refs", "tags"].iter().collect())?;
    repo.create_repo_dir(["refs", "heads"].iter().collect())?;

    // .git/description
    std::fs::write(
        repo.repo_file("description".into()),
        "Unnamed repository; edit this file 'description' to name the repository.\n",
    )
    .map_err(|e| e.to_string())?;

    // .git/HEAD
    std::fs::write(repo.repo_file("HEAD".into()), "ref: refs/heads/master\n")
        .map_err(|e| e.to_string())?;

    // .git/config
    GitRepository::default_config()
        .write(repo.repo_file("config".into()))
        .map_err(|e| e.to_string())?;

    Ok(repo)
}

pub(crate) fn repo_find() -> Result<GitRepository, String> {
    // Finds the .git folder for the current dir
    let mut node: PathBuf = ".".into();
    node = node.canonicalize().map_err(|e| e.to_string())?;
    for path in node.ancestors() {
        if path.join(".git").exists() {
            // This is what we have been looking
            return Ok(GitRepository::new(path.into(), false)?);
        }
    }
    return Err("Not a .git repository (or any of the parent directories): .git".to_string());
}

pub(crate) fn cat_file(repo: GitRepository, sha: String, obj_type: String) -> Result<(), String> {
    let obj = GitRepository::object_read(&repo, &repo.object_find(sha, obj_type))
        .ok_or("Unable to read Object")?;

    let content = obj.serialize();
    content.iter().for_each(|&ch| print!("{}", ch as char));
    Ok(())
}

pub(crate) fn hash_object(
    repo: Option<GitRepository>,
    obj_type: String,
    data: Vec<u8>,
) -> Result<String, String> {
    let x = match obj_type.as_str() {
        "blob" => GitObject::Blob(GitBlob::from(data)),
        "tree" => GitObject::Tree(GitTree::from(data)),
        "commit" => GitObject::Commit(GitCommit::from(data)),
        "tag" => GitObject::Tag(GitTag::from(data)),
        _ => Err("Unknown object type encountered")?,
    };

    Ok(GitRepository::object_write(repo, x).unwrap())
}
