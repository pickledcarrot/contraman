use clap::{Parser, Subcommand};
use keyring::Entry;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

const SERVICE_NAME: &str = "contraman";
const INDEX_FILE_NAME: &str = "entries.txt";

#[derive(Parser, Debug)]
#[command(
    name = "contraman",
    version,
    about = "A minimal CLI password manager backed by your OS keychain"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Shorthand for `get <name>`
    name: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Store or update a password in the OS keychain
    Set {
        name: String,
        #[arg(short, long)]
        password: Option<String>,
    },
    /// Fetch a password, copy it to the clipboard, and print it
    Get { name: String },
    /// List known entry names
    List {
        /// Include password values in the output
        #[arg(long)]
        with_pass: bool,
    },
    /// Remove an entry from the keychain and local index
    Remove { name: String },
}

pub fn run() -> Result<(), String> {
    let cli = Cli::parse();

    match (cli.command, cli.name) {
        (Some(Commands::Set { name, password }), None) => set_password(&name, password),
        (Some(Commands::Get { name }), None) => get_password(&name),
        (Some(Commands::List { with_pass }), None) => list_entries(with_pass),
        (Some(Commands::Remove { name }), None) => remove_password(&name),
        (None, Some(name)) => get_password(&name),
        (None, None) => {
            let program = env::args()
                .next()
                .and_then(|arg| {
                    PathBuf::from(arg)
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| "contraman".to_string());

            println!("Usage:");
            println!("  {program} set <name>");
            println!("  {program} get <name>");
            println!("  {program} <name>");
            println!("  {program} list");
            println!("  {program} list --with-pass");
            println!("  {program} remove <name>");
            Ok(())
        }
        _ => Err("Provide either a subcommand or a single entry name.".to_string()),
    }
}

fn set_password(name: &str, password: Option<String>) -> Result<(), String> {
    let password = match password {
        Some(password) => password,
        None => prompt_password(name)?,
    };

    entry(name)?
        .set_password(&password)
        .map_err(|err| format!("failed to save password to keychain: {err}"))?;
    upsert_index(name)?;

    println!("Stored password for `{name}`.");
    Ok(())
}

fn get_password(name: &str) -> Result<(), String> {
    let password = entry(name)?
        .get_password()
        .map_err(|err| format!("failed to read password from keychain: {err}"))?;

    copy_to_clipboard(&password)?;
    println!("{password}");
    eprintln!("Copied `{name}` to the clipboard.");
    Ok(())
}

fn list_entries(with_pass: bool) -> Result<(), String> {
    let entries = read_index()?;
    if entries.is_empty() {
        println!("No entries stored yet.");
        return Ok(());
    }

    for name in entries {
        if with_pass {
            let password = entry(&name)?
                .get_password()
                .map_err(|err| format!("failed to read password for `{name}`: {err}"))?;
            println!("{name}\t{password}");
        } else {
            println!("{name}");
        }
    }

    Ok(())
}

fn remove_password(name: &str) -> Result<(), String> {
    entry(name)?
        .delete_credential()
        .map_err(|err| format!("failed to remove password from keychain: {err}"))?;
    remove_from_index(name)?;

    println!("Removed `{name}`.");
    Ok(())
}

fn prompt_password(name: &str) -> Result<String, String> {
    print!("Enter password for `{name}`: ");
    io::stdout()
        .flush()
        .map_err(|err| format!("failed to flush stdout: {err}"))?;

    let password =
        rpassword::read_password().map_err(|err| format!("failed to read password: {err}"))?;

    if password.is_empty() {
        return Err("password cannot be empty".to_string());
    }

    Ok(password)
}

fn entry(name: &str) -> Result<Entry, String> {
    Entry::new(SERVICE_NAME, name).map_err(|err| format!("failed to create keychain entry: {err}"))
}

fn copy_to_clipboard(contents: &str) -> Result<(), String> {
    if cfg!(target_os = "macos") {
        return pipe_to_command("pbcopy", &[], contents);
    }

    if cfg!(target_os = "windows") {
        return pipe_to_command("clip", &[], contents);
    }

    if command_exists("wl-copy") {
        return pipe_to_command("wl-copy", &[], contents);
    }

    if command_exists("xclip") {
        return pipe_to_command("xclip", &["-selection", "clipboard"], contents);
    }

    Err("no supported clipboard command found (tried pbcopy, clip, wl-copy, xclip)".to_string())
}

fn pipe_to_command(program: &str, args: &[&str], input: &str) -> Result<(), String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to launch `{program}`: {err}"))?;

    let stdin = child
        .stdin
        .as_mut()
        .ok_or_else(|| format!("failed to open stdin for `{program}`"))?;
    stdin
        .write_all(input.as_bytes())
        .map_err(|err| format!("failed to write to `{program}` stdin: {err}"))?;

    let status = child
        .wait()
        .map_err(|err| format!("failed to wait for `{program}`: {err}"))?;
    if !status.success() {
        return Err(format!("`{program}` exited with status {status}"));
    }

    Ok(())
}

fn command_exists(program: &str) -> bool {
    Command::new("which")
        .arg(program)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn index_file_path() -> Result<PathBuf, String> {
    let mut candidates = Vec::new();

    if let Some(path) = env::var_os("CONTRAMAN_DATA_DIR") {
        candidates.push(PathBuf::from(path));
    }

    if let Some(path) = dirs::data_local_dir() {
        candidates.push(path.join(SERVICE_NAME));
    }

    let current_dir =
        env::current_dir().map_err(|err| format!("failed to resolve current directory: {err}"))?;
    candidates.push(current_dir.join(".contraman"));

    for dir in candidates {
        if fs::create_dir_all(&dir).is_ok() {
            return Ok(dir.join(INDEX_FILE_NAME));
        }
    }

    Err(
        "failed to create a writable data directory; set CONTRAMAN_DATA_DIR to a writable path"
            .to_string(),
    )
}

fn read_index() -> Result<BTreeSet<String>, String> {
    let path = index_file_path()?;
    if !path.exists() {
        return Ok(BTreeSet::new());
    }

    let contents =
        fs::read_to_string(&path).map_err(|err| format!("failed to read index file: {err}"))?;
    Ok(contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

fn write_index(entries: &BTreeSet<String>) -> Result<(), String> {
    let path = index_file_path()?;
    let mut data = String::new();
    for entry in entries {
        data.push_str(entry);
        data.push('\n');
    }

    fs::write(path, data).map_err(|err| format!("failed to write index file: {err}"))
}

fn upsert_index(name: &str) -> Result<(), String> {
    let mut entries = read_index()?;
    entries.insert(name.to_string());
    write_index(&entries)
}

fn remove_from_index(name: &str) -> Result<(), String> {
    let mut entries = read_index()?;
    entries.remove(name);
    write_index(&entries)
}
