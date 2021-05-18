mod join;
mod split;

use std::{
    borrow::Cow,
    env,
    ffi::OsString,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

pub fn get_file_name(path: &Path) -> Result<&str, Error> {
    path.file_name()
        .unwrap()
        .to_str()
        .ok_or_else(|| Error("Invalid UTF-8".into()))
}

#[derive(Debug)]
pub struct Error(Cow<'static, str>);

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        use io::ErrorKind::*;

        let msg = match err.kind() {
            PermissionDenied => "Permission denied.",
            NotFound => "File not found.",
            _ => "Unknown error.",
        };

        Error(msg.into())
    }
}

impl From<Cow<'static, str>> for Error {
    fn from(err: Cow<'static, str>) -> Self {
        Error(err)
    }
}

fn get_paths(entries: fs::ReadDir) -> Result<Vec<PathBuf>, Error> {
    let mut paths_vec = Vec::<PathBuf>::new();

    for entry in entries {
        match entry {
            Ok(entry) => {
                paths_vec.push(entry.path());
            }
            Err(err) => return Err(err.into()),
        }
    }

    Ok(paths_vec)
}

fn handle_arg(
    stdin: &mut io::StdinLock,
    stdout: &mut io::StdoutLock,
    stderr: &mut io::StderrLock,
    arg: OsString,
) -> Result<Cow<'static, str>, Error> {
    let path = Path::new(&arg);
    if path.is_dir() {
        match path.read_dir() {
            Ok(entries) => match get_paths(entries) {
                Ok(vec) => join::join(vec),
                Err(err) => Err(err),
            },
            Err(_) => Err(Error("Unknown error".into())),
        }
    } else if path.is_file() {
        split::split(stdin, stdout, stderr, path.to_path_buf())
    } else {
        Err(Error(
            format!("File or directory not found: {}", path.to_string_lossy()).into(),
        ))
    }
}

fn main() {
    let message_dialog = match run() {
        Ok(message) => rfd::MessageDialog::new()
            .set_description(&message)
            .set_title("splitter")
            .set_level(rfd::MessageLevel::Info),
        Err(Error(message)) => rfd::MessageDialog::new()
            .set_description(&message)
            .set_title("splitter")
            .set_level(rfd::MessageLevel::Error),
    };

    message_dialog.show();
}

fn run() -> Result<Cow<'static, str>, Error> {
    let mut args = env::args_os();

    // NOTE: I want optimal performance, control and I don't want to unlock on every write.
    //       This is about the best way I found to do that.
    //       So I'm locking all standard streams at the start and then pass it around throughout the program.
    //       There might be something better.
    let (stdin_handle, stdout_handle, stderr_handle) = (io::stdin(), io::stdout(), io::stderr());
    let (mut stdin, mut stdout, mut stderr) = (
        stdin_handle.lock(),
        stdout_handle.lock(),
        stderr_handle.lock(),
    );

    args.next(); // This is probably the program name

    if let Some(arg) = args.next() {
        handle_arg(&mut stdin, &mut stdout, &mut stderr, arg)
    } else {
        writeln!(
            stdout,
            "Please select one file to split or multiple files to join."
        )?;

        if let Some(paths) = rfd::FileDialog::new().pick_files() {
            if paths.len() > 1 {
                join::join(paths)
            } else if let Some(path) = paths.get(0) {
                if path.is_file() {
                    split::split(&mut stdin, &mut stdout, &mut stderr, path.clone())
                } else {
                    Err(Error(
                        "Given entry is not a file and cannot be split.".into(),
                    ))
                }
            } else {
                unreachable!()
            }
        } else {
            Err(Error("No files were given.".into()))
        }
    }
}
