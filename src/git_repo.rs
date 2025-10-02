use configparser::ini::Ini;
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use indexmap::IndexMap;
use sha1::{Digest, Sha1};
use std::{
    collections::HashSet,
    io::{Read, Write},
    path::PathBuf,
};

#[derive(Debug)]
pub struct GitRepository {
    pub(super) worktree: PathBuf,
    pub(super) git_dir: PathBuf,
    pub(super) conf: Ini,
}

impl GitRepository {
    pub(crate) fn repo_file(&self, file: PathBuf) -> PathBuf {
        self.git_dir.join(file)
    }

    pub(crate) fn repo_dir(&self, dir: String) -> Result<PathBuf, String> {
        let dir = self.git_dir.join(dir);
        if dir.is_dir() {
            Ok(dir)
        } else {
            Err("No Dir".to_string())
        }
    }
    pub(crate) fn new(path: PathBuf, no_check: bool) -> Result<Self, String> {
        let worktree = path;
        let git_dir = worktree.join(".git");
        let mut conf = Ini::new();

        if !no_check && !git_dir.is_dir() {
            return Err("Not a Git repo".to_string());
        }
        let config_file = git_dir.join("config");
        if !no_check && config_file.is_file() {
            conf.load(config_file)?;
            let ver = conf.getuint("core", "repositoryformatversion")?;
            if ver.is_none() || ver.unwrap() != 0 {
                return Err("Unsupported repository format version".to_string());
            }
        }

        Ok(GitRepository {
            worktree,
            git_dir,
            conf,
        })
    }

    pub(crate) fn create_repo_dir(&self, path: PathBuf) -> Result<(), String> {
        std::fs::create_dir_all(self.git_dir.join(&path))
            .map_err(|_| format!("Unable to create dir: {path:?}"))
    }

    pub(crate) fn default_config() -> Ini {
        let mut x = Ini::new();
        x.set("core", "repositoryformatversion", Some("0".into()));
        x.set("core", "filemode", Some("false".into()));
        x.set("core", "bare", Some("false".into()));
        x
    }

    pub(crate) fn object_read(&self, sha: &str) -> Option<GitObject> {
        let path = self.repo_file(["objects", &sha[0..2], &sha[2..]].iter().collect());
        if !path.is_file() {
            return None;
        }
        // Open file in binary read mode and decompress it using zlib
        let file = std::fs::File::open(&path)
            .map_err(|_| format!("Unable to open file: {path:?}"))
            .expect("Unable to open file");
        let mut zlib = ZlibDecoder::new(file);
        let mut buffer = Vec::new();
        zlib.read_to_end(&mut buffer).expect("Unable to decompress");

        // Find the first space in the buffer
        // And split the buffer on that first space
        let mut space_pos = 0;
        let mut null_pos = 0;
        for (i, &ch) in buffer.iter().enumerate() {
            if space_pos == 0 {
                if ch == b' ' {
                    space_pos = i;
                }
            } else if null_pos == 0 {
                if ch == 0x0 {
                    null_pos = i;
                }
            } else {
                break;
            }
        }
        let obj_type = String::from_utf8(buffer[..space_pos].into()).expect("Malformed object");
        let obj_size: usize = String::from_utf8(buffer[space_pos + 1..null_pos].into())
            .expect("Malformed object")
            .parse()
            .expect("Malformed object: Size is not a number");

        // Verify size
        assert_eq!(buffer.len() - null_pos - 1, obj_size);

        Some(match obj_type.as_str() {
            "commit" => GitObject::Commit(GitCommit::from(buffer[null_pos + 1..].into())),
            "tree" => GitObject::Tree(GitTree::from(buffer[null_pos + 1..].into())),
            "tag" => GitObject::Tag(GitTag::from(buffer[null_pos + 1..].into())),
            "blob" => GitObject::Blob(GitBlob::from(buffer[null_pos + 1..].into())),
            _ => panic!("Unknown object type: {}", obj_type),
        })
    }

    pub(crate) fn object_write(
        repo: Option<GitRepository>,
        object: GitObject,
    ) -> Result<String, String> {
        let (obj_type, data) = match object {
            GitObject::Blob(d) => ("blob", d.serialize()),
            GitObject::Commit(d) => ("commit", d.serialize()),
            GitObject::Tag(d) => ("tag", d.serialize()),
            GitObject::Tree(d) => ("tree", d.serialize()),
        };
        let mut buffer: Vec<u8> = Vec::new();
        obj_type.as_bytes().iter().for_each(|&ch| buffer.push(ch));

        buffer.push(b' ');

        // push length of data
        data.len()
            .to_string()
            .as_bytes()
            .iter()
            .for_each(|&ch| buffer.push(ch));

        buffer.push(0x0);

        // push data
        data.iter().for_each(|&ch| buffer.push(ch));

        // Compute Hash
        let digest: String = Sha1::digest(&buffer)
            .iter()
            .map(|ch| format!("{:02x}", ch))
            .collect();

        if let Some(repo) = repo {
            let path = repo.repo_file(["objects", &digest[..2], &digest[2..]].iter().collect());
            if !path.exists() {
                // Create path and write the content
                let file = std::fs::File::create_new(path).unwrap();

                let mut compressor = ZlibEncoder::new(file, Compression::default());
                let x = compressor.write(&buffer).unwrap();
                println!("Written bytes: {x}");
            } else {
                println!("Path: {path:#?} Already exists");
            }
        }
        return Ok(digest);
    }

    pub(crate) fn object_find(&self, name: String, fmt: String) -> String {
        name
    }
}

// Note: Could be optimized.
pub(crate) fn kv_parser(data: Vec<u8>) -> Result<IndexMap<String, Vec<String>>, String> {
    let data: String = String::from_utf8(data).map_err(|err| err.to_string())?;

    let mut map: IndexMap<String, Vec<String>> = IndexMap::new();

    let (header, msg) = data.split_once("\n\n").ok_or("No message section")?;
    let null = "\x00";
    let header = header.replace("\n ", null); // Escape the \n followed by space

    for line in header.split("\n") {
        assert!(!line.is_empty(), "There can't be an empty line");

        let (key, value) = line.split_once(" ").ok_or("Malformed Header section")?;
        let value = value.replace(null, "\n"); // Remove the placeholder to reveal original data

        if map.get(key).is_none() {
            // There is no value
            map.insert(key.into(), Vec::new());
        }
        let list = map.get_mut(key).unwrap();
        list.push(value);
    }
    let mut value = Vec::new();
    value.push(msg.into());

    map.insert(null.into(), value);
    Ok(map)
}

pub(crate) fn kv_serialize(map: IndexMap<String, Vec<String>>) -> Vec<u8> {
    let null = "\x00";
    let mut val = String::new();
    for (key, array) in &map {
        if key == null {
            continue; // The commit msg will be the last one
        }
        for value in array {
            val.push_str(key);
            val.push(' ');

            // The value with multiple line
            let value = value.replace("\n", "\n ");

            val.push_str(&value);
            val.push('\n');
        }
    }
    val.push('\n');
    if let Some(msg) = map.get(null) {
        val.push_str(&msg[0]);
    }

    val.into()
}

pub(crate) trait Serializable {
    fn serialize(self) -> Vec<u8>;
    fn deserialize(&mut self, data: Vec<u8>);
}

#[derive(Debug)]
pub(crate) enum GitObject {
    Blob(GitBlob),
    Commit(GitCommit),
    Tag(GitTag),
    Tree(GitTree),
}

impl Serializable for GitObject {
    fn serialize(self) -> Vec<u8> {
        match self {
            GitObject::Blob(data) => data.serialize(),
            GitObject::Commit(data) => data.serialize(),
            GitObject::Tag(data) => data.serialize(),
            GitObject::Tree(data) => data.serialize(),
        }
    }

    fn deserialize(&mut self, data: Vec<u8>) {
        todo!()
    }
}

#[derive(Debug)]
pub(crate) struct GitBlob {
    pub(crate) buffer: Vec<u8>,
}
#[derive(Debug)]
pub(crate) struct GitTree {
    pub(crate) items: Vec<([u8; 6], String, String)>,
}
#[derive(Debug)]
pub(crate) struct GitCommit {
    pub(crate) data: IndexMap<String, Vec<String>>,
}
#[derive(Debug)]
pub(crate) struct GitTag {
    buffer: Vec<u8>,
}

impl GitBlob {
    pub(crate) fn from(buffer: Vec<u8>) -> Self {
        GitBlob { buffer }
    }
}

impl GitTree {
    pub(crate) fn from(buffer: Vec<u8>) -> Self {
        GitTree {
            items: tree_parse(&buffer),
        }
    }
    pub(crate) fn new() -> Self {
        GitTree { items: Vec::new() }
    }
}
impl GitCommit {
    pub(crate) fn from(buffer: Vec<u8>) -> Self {
        GitCommit {
            data: kv_parser(buffer).unwrap(),
        }
    }
}
impl GitTag {
    pub(crate) fn from(buffer: Vec<u8>) -> Self {
        GitTag { buffer }
    }
}

impl Serializable for GitBlob {
    fn serialize(self) -> Vec<u8> {
        self.buffer
    }

    fn deserialize(&mut self, data: Vec<u8>) {
        self.buffer = data;
    }
}

// Todo implement these
impl Serializable for GitCommit {
    fn serialize(self) -> Vec<u8> {
        kv_serialize(self.data)
    }

    fn deserialize(&mut self, data: Vec<u8>) {
        self.data = kv_parser(data).unwrap();
    }
}
impl Serializable for GitTree {
    fn serialize(self) -> Vec<u8> {
        let mut items = self.items.clone();
        tree_serialize(&mut items)
    }

    fn deserialize(&mut self, data: Vec<u8>) {
        self.items = tree_parse(&data);
    }
}
impl Serializable for GitTag {
    fn serialize(self) -> Vec<u8> {
        self.buffer
    }

    fn deserialize(&mut self, data: Vec<u8>) {
        todo!()
    }
}

/// **Recursive algorithm to log history of the commit provided**
///
/// It prints directly to stdout.
pub fn log_graphviz(
    repo: &GitRepository,
    sha: &String,
    seen: &mut HashSet<String>,
) -> Result<(), String> {
    if seen.contains(sha) {
        return Ok(());
    }
    seen.insert(sha.to_owned());

    if let GitObject::Commit(commit) = repo.object_read(sha).ok_or("The object can't be read")? {
        let msg = commit
            .data
            .get("\x00")
            .expect("No commit section")
            .first()
            .expect("No commit msg found");

        let msg = msg.replace("\\", "\\\\");
        let msg = msg.replace("\"", "\\\"");

        // Only 1st line when there are multiple lines in the commit
        let msg = msg.split_once("\n").unwrap_or(("default", "")).0;

        println!("  c_{} [label=\"{} : {}\"]", sha, &sha[..7], &msg);

        // Lets print parent now
        if let Some(parents) = commit.data.get("parent") {
            for parent in parents {
                println!("  c_{sha} -> c_{parent};");
                log_graphviz(repo, parent, seen)?;
            }
        } else {
            // First commit, no parent
            // return
        }
    } else {
        panic!("Inconsistent");
    }

    Ok(())
}

pub(crate) fn tree_parse(data: &Vec<u8>) -> Vec<([u8; 6], String, String)> {
    let mut list = Vec::new();
    let mut i = 0;
    loop {
        if i >= data.len() {
            break;
        }
        let frame = &data[i..];

        let null_pos = frame
            .iter()
            .position(|&ch| ch == 0)
            .expect("Malformed tree");
        let sha = format!(
            "{:0>40}",
            frame[null_pos + 1..=null_pos + 20]
                .iter()
                .map(|ch| format!("{:02x}", ch))
                .collect::<String>()
        );

        let (m, path) = frame[..null_pos].split_at(
            frame
                .iter()
                .position(|&ch| ch == b' ')
                .expect("Malformed tree"),
        );
        let mut file_mode: [u8; 6] = [0; 6];
        file_mode[if m.len() == 6 { 0 } else { 1 }..].copy_from_slice(m);
        let path = String::from_utf8(path[1..].into()).expect("Malformed tree");
        list.push((file_mode, path, sha));
        i += null_pos + 21;
    }
    list
}

// pub(crate) fn tree_parse_line(data: &[u8]) -> ([u8; 6], String, String) {
//     let (file_info, sha) =
//         data.split_at(data.iter().position(|&x| x == 0).expect("Malformed tree"));
//     let (file_mode, file_path) = file_info.split_at(
//         file_info
//             .iter()
//             .position(|&x| x == b' ')
//             .expect("Malformed tree"),
//     );
//     assert!(file_mode.len() == 5 || file_mode.len() == 6);
//     let mut mode: [u8; 6] = [0; 6];

//     mode[if file_mode.len() == 6 { 0 } else { 1 }..].copy_from_slice(file_mode);

//     let file_path = String::from_utf8(file_path.into()).expect("Malformed tree");
//     let sha = format!(
//         "{:0>40}",
//         sha.iter()
//             .map(|ch| format!("{:02x}", ch))
//             .collect::<String>()
//     );

//     (mode, file_path, sha)
// }

pub(crate) fn tree_serialize(list: &mut Vec<([u8; 6], String, String)>) -> Vec<u8> {
    // Sort the list
    list.sort_by(|a, b| {
        let x = if a.0[0] == b'1' && a.0[1] == b'0' {
            a.1.clone()
        } else {
            a.1.clone() + "/"
        };
        let y = if b.0[0] == b'1' && b.0[1] == b'0' {
            b.1.clone()
        } else {
            b.1.clone() + "/"
        };

        (&a.0, x, &a.2).cmp(&(&b.0, y, &b.2))
    });

    // let mut list = list
    //     .iter()
    //     .map(|data| {
    //         if (data.0[0] == b'1' && data.0[1] == b'0') {
    //             // file
    //             data.clone()
    //         } else {
    //             // Directory
    //             (data.0, data.1.clone() + "/", data.2.clone())
    //         }
    //     })
    //     .collect::<Vec<_>>();
    // list.sort();

    // Serialize
    let mut result: Vec<u8> = Vec::new();

    for entry in list {
        for ele in entry.0 {
            result.push(ele);
        }
        // result.copy_from_slice(&entry.0);
        result.push(b' ');
        for ele in entry.1.as_bytes() {
            result.push(*ele);
        }
        // result.copy_from_slice(entry.1.as_bytes());
        result.push(0);
        let mut hex: Vec<u8> = (0..entry.2.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&entry.2[i..i + 2], 16).expect("Problem with hex string"))
            .collect();
        result.append(&mut hex);
    }
    result
}

#[test]
fn test_tree_serializer() {
    let mut data = Vec::new();
    data.push((
        [b'1', b'0', 3, 4, 5, 2],
        "AEADME".to_string(),
        "29c95630072cd48c6c227938e66681536613f9ad".to_string(),
    ));
    data.push((
        [b'1', b'0', 3, 4, 5, 2],
        "REAME".to_string(),
        "29c95630072cd48c6c227938e66681536613f9ad".to_string(),
    ));
    data.push((
        [b'0', b'0', 3, 4, 5, 2],
        "README".to_string(),
        "29c95630072cd48c6c227938e66681536613f9ad".to_string(),
    ));
    data.push((
        [b'0', b'0', 3, 4, 5, 2],
        "README".to_string(),
        "29c95630072cd48c6c227938e66681536613f9ad".to_string(),
    ));
    data.push((
        [b'0', b'0', 3, 4, 5, 2],
        "AEADME".to_string(),
        "29c95630072cd48c6c227938e66681536613f9ad".to_string(),
    ));

    println!("{data:#?}");
    println!("----------------------------------");
    let x = tree_serialize(&mut data);
    let y = tree_parse(&x);

    println!("{y:#?}");
    todo!()
}
