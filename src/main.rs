use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Init {
        #[arg(default_value = ".")]
        path: String,
    },
    CatFile {
        #[arg(name = "type")]
        obj_type: String,
        object: String,
    },
    HashObject {
        #[arg(short, help = "Write it to the disk")]
        write: bool,

        #[arg(help = "type of file", name = "type")]
        file_type: String,

        #[arg(help = "file path", name = "path")]
        file_path: String,
    },

    #[command(about = "Display history of a given commit.")]
    Log {
        #[arg(help = "Commit to start at.", default_value = "HEAD")]
        commit: String,
    },
    #[command(about = "Preety-print the tree object")]
    LsTree {
        #[arg(short, help = "Recurse into sub-trees")]
        recursive: bool,
        #[arg(help = "tree object to start from")]
        tree: String,
    },
    #[command(about = "Checkout the specific version from the git history to the given path")]
    Checkout {
        #[arg(help = "The commit or tree to checkout")]
        commit: String,
        #[arg(help = "The path where to store those files")]
        path: String,
    },
    // Add,
    // CheckIgnore,
    // Commit,
    // LsFiles,
    // RevParse,
    // Rm,
    // ShowRef,
    // Status,
    // Tag,
}

fn main() {
    let x = Cli::parse();
    let result = match x.command {
        Commands::Init { path } => oz::cmd_init(path),
        Commands::CatFile { obj_type, object } => oz::cmd_cat_file(obj_type, object),
        Commands::HashObject {
            write,
            file_type,
            file_path,
        } => oz::cmd_hash_object(write, file_type, file_path),
        Commands::Log { commit } => oz::cmd_log(commit),
        Commands::LsTree { recursive, tree } => oz::cmd_list_tree(recursive, tree),
        Commands::Checkout { commit, path } => oz::cmd_checkout(commit, path),
    };
    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
