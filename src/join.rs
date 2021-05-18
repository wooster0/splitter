use crate::Error;
use std::{
    borrow::Cow,
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    usize,
};

fn get_trailing_number(path: &Path) -> Result<usize, Error> {
    if let Some(path) = path.to_str() {
        if let Some((_, trailing_number)) = path.rsplit_once('-') {
            Ok(trailing_number
                .parse::<usize>()
                .map_err(|_| Error("invalid trailing number".into()))?)
        } else {
            Err(Error("no trailing number found".into()))
        }
    } else {
        Err(Error("path is not UTF-8".into()))
    }
}

/// Splits off the "split" suffix from filenames.
///
/// # Examples
///
/// ```
/// assert_eq!(split_file_name("Cargo.toml-split-0"), "Cargo.toml");
/// ```
fn split_file_name(filename: &str) -> Option<&str> {
    let mut split = filename.rsplitn(3, '-');
    split.next()?;
    split.next()?;
    split.next()
}

struct File {
    file: fs::File,
    trailing_number: usize,
}

pub fn join(path_bufs: Vec<PathBuf>) -> Result<Cow<'static, str>, Error> {
    let first_path = &path_bufs[0];
    let file_name = if first_path.is_file() {
        let file_name = crate::get_file_name(first_path)?;

        split_file_name(file_name)
            .ok_or_else(|| Error(format!("Invalid filename: {}", file_name).into()))
    } else {
        Err(Error(
            format!("{} is not a file", first_path.to_string_lossy()).into(),
        ))
    }?;

    let mut files = Vec::<File>::new();
    let mut total_len = 0;

    for path in &path_bufs {
        let fs_file = fs::File::open(path)?;
        let trailing_number = get_trailing_number(&path)?;

        total_len += fs_file.metadata()?.len();

        let file = File {
            file: fs_file,
            trailing_number,
        };

        files.push(file)
    }

    // We make no assumptions about the order of `files` and sort it by trailing number.
    // We can use an unstable sort because our input is guaranteed to have no duplicates.
    files.sort_unstable_by(|file1, file2| file1.trailing_number.cmp(&file2.trailing_number));

    for (index, file) in files.iter().enumerate() {
        if index + 1 != file.trailing_number {
            return Err(Error(
                "Trailing number mismatch. Make sure you provided all split files.".into(),
            ));
        }
    }

    let mut open_options = fs::OpenOptions::new();
    open_options.write(true).create_new(true);
    let output_file_name = String::from("joined-") + file_name;
    let mut output = open_options
        .open(&output_file_name)
        .map_err(|err| match err.kind() {
            io::ErrorKind::AlreadyExists => Error(
                format!(
                    "Failed to create output file. {} already exists.",
                    output_file_name
                )
                .into(),
            ),
            _ => Error("Failed to create output file.".into()),
        })?;

    let mut buf = Vec::<u8>::with_capacity(total_len as usize);

    // NOTE: This could be more efficient
    for file in &mut files {
        file.file.read_to_end(&mut buf)?;
    }
    // This panics:
    // assert_eq!(buf.capacity(), buf.len());

    output
        .write_all(&buf)
        .map_err(|_| Error("Failed to write output".into()))?;

    Ok(format!("Successful join. Joined file: {}", output_file_name).into())
}
