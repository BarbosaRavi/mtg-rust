use anyhow::{Context, Result};
use std::fs::File;
use std::io::{ErrorKind, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub struct LogFollower {
    path: PathBuf,
    offset: u64,
    pending: Vec<u8>,
}

impl LogFollower {
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            offset: 0,
            pending: Vec::new(),
        }
    }

    pub fn read_new_lines(&mut self) -> Result<String> {
        let mut file = match File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(String::new()),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("não consegui abrir {}", self.path.display()));
            }
        };

        if file.metadata()?.len() < self.offset {
            self.offset = 0;
            self.pending.clear();
        }

        file.seek(SeekFrom::Start(self.offset))?;
        let read = file.read_to_end(&mut self.pending)?;
        self.offset += read as u64;

        let ready: Vec<u8> = self.pending.drain(..self.ready_len()).collect();
        Ok(String::from_utf8_lossy(&ready).into_owned())
    }

    fn ready_len(&self) -> usize {
        let Some(last_newline) = self.pending.iter().rposition(|&byte| byte == b'\n') else {
            return 0;
        };

        let last_line_start = self.pending[..last_newline]
            .iter()
            .rposition(|&byte| byte == b'\n')
            .map_or(0, |index| index + 1);

        if self.pending[last_line_start..].starts_with(b"<== ") {
            last_line_start
        } else {
            last_newline + 1
        }
    }
}
