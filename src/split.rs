use crate::Error;
use parse_size::parse_size;
use std::{
    borrow::Cow,
    fs,
    io::{self, BufRead, Read, Write},
    path::PathBuf,
    usize,
};

fn get_split_size(
    stdin: &mut io::StdinLock,
    stdout: &mut io::StdoutLock,
    stderr: &mut io::StderrLock,
) -> Result<u64, Error> {
    fn parse_split_size(
        stdin: &mut io::StdinLock,
        input: &mut String,
    ) -> Result<u64, &'static str> {
        stdin.read_line(input).map_err(|_| "Failed to read input")?;

        match parse_size(input.trim()) {
            Ok(size) => Ok(size),
            Err(err) => {
                use parse_size::Error::*;

                let err = match err {
                    PosOverflow => "Size too big",
                    Empty => "No input",
                    _ => "Invalid input",
                };
                Err(err)
            }
        }
    }

    let mut input = String::new();

    loop {
        write!(stdout, "Split size:  ")?;
        stdout.flush()?;

        match parse_split_size(stdin, &mut input) {
            Ok(split_size) => break Ok(split_size),
            Err(err) => writeln!(stderr, "{}. Please try again.", err)?,
        };

        input.clear();
    }
}

pub fn split(
    stdin: &mut io::StdinLock,
    stdout: &mut io::StdoutLock,
    stderr: &mut io::StderrLock,
    mut path_buf: PathBuf,
) -> Result<Cow<'static, str>, Error> {
    // NOTE: this is one of those cases where I would like to let the error run through the
    //       logic of `From<io::Error>::from` first to be able to provide more information
    //       if for instance the permission is denied. I haven't yet found a very idiomatic way to do that
    //       so for now all errors will result in only this error message
    let mut file = fs::File::open(&path_buf).map_err(|_| Error("Failed to open file.".into()))?;

    let file_len = file.metadata()?.len();

    writeln!(stdout, "File length: {}", file_len)?;

    let split_size = get_split_size(stdin, stdout, stderr)?;

    if file_len < split_size {
        return Err(Error(
            "File length is below split length. Nothing to split.".into(),
        ));
    }

    let mut buffers = get_buffers(file_len, split_size);

    let buffers = &mut buffers
        .iter_mut()
        .map(|buffer| io::IoSliceMut::new(buffer))
        .collect::<Vec<io::IoSliceMut>>();

    file.read_vectored(buffers)
        .map_err(|_| Error("Failed reading file.".into()))?;

    let mut path_os_string = path_buf.clone().into_os_string();
    path_os_string.push("-split");

    path_buf.set_file_name(path_os_string);

    fs::create_dir(&path_buf).map_err(|_| {
        Error(
            format!(
                "Folder {} already exists. Please remove the previous split folder.",
                path_buf.to_string_lossy()
            )
            .into(),
        )
    })?;

    let mut open_options = fs::OpenOptions::new();
    open_options.write(true).create_new(true);

    for (index, buffer) in buffers.iter().enumerate() {
        let path_os_string = crate::get_file_name(&path_buf)?;
        let file_name = format!("{}-{}", path_os_string, index + 1);
        let mut file = open_options
            .open(path_buf.join(file_name))
            .map_err(|_| Error("Failed to create output file.".into()))?;
        file.write_all(buffer)
            .map_err(|_| Error("Failed to write output.".into()))?;
    }

    Ok(format!("Successful split. Split folder: {}\n\nNote that altering the trailing numbers of the filenames may result in corruption when the files are joined.", path_buf.to_string_lossy()).into())
}

/// Splits `parts` until all elements are below `split_size`.
///
/// # Examples
//
/// ```
/// let parts = split::split_parts(10, 3);
///
/// assert_eq!(parts, [2, 2, 1, 1, 2, 2]);
/// ```  
pub fn split_parts(initial_part: u64, split_size: u64) -> Vec<u64> {
    // NOTE: the algorithm could be more efficient

    let mut parts = vec![initial_part];

    while !parts.iter().all(|part| *part < split_size) {
        // NOTE: maybe there is a better way to both half the element and add a new one
        for index in 0..parts.len() {
            let part = parts[index];

            if part >= split_size {
                let half = part / 2;

                parts[index] = half;

                parts.push(half + part % 2);
            }
        }
    }

    debug_assert_eq!(initial_part, parts.iter().sum::<u64>());

    parts
}

fn get_buffers(file_len: u64, split_size: u64) -> Vec<Vec<u8>> {
    let parts = split_parts(file_len, split_size);

    let mut buffers = Vec::<Vec<u8>>::new();
    for part in &parts {
        buffers.push(vec![0_u8; *part as usize]);
    }

    debug_assert_eq!(buffers.len(), parts.len());

    buffers
}
