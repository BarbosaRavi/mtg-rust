#[cfg(target_os = "linux")]
use anyhow::{Context, anyhow};
use anyhow::{Result, bail};

const STRING_CHUNK: usize = 64;
const MAX_STRING: usize = 1024;

pub struct Process {
    #[cfg(target_os = "linux")]
    pid: u32,
    #[cfg(target_os = "linux")]
    memory: std::fs::File,
}

impl Process {
    pub fn read(&self, address: u64, buffer: &mut [u8]) -> Result<()> {
        if self.read_some(address, buffer)? < buffer.len() {
            bail!("leitura incompleta da memória do MTGA em {address:#x}");
        }
        Ok(())
    }

    pub fn read_vec(&self, address: u64, len: usize) -> Result<Vec<u8>> {
        let mut buffer = vec![0; len];
        self.read(address, &mut buffer)?;
        Ok(buffer)
    }

    pub fn read_u8(&self, address: u64) -> Result<u8> {
        Ok(self.read_array::<1>(address)?[0])
    }

    pub fn read_u16(&self, address: u64) -> Result<u16> {
        Ok(u16::from_le_bytes(self.read_array(address)?))
    }

    pub fn read_i32(&self, address: u64) -> Result<i32> {
        Ok(i32::from_le_bytes(self.read_array(address)?))
    }

    pub fn read_u32(&self, address: u64) -> Result<u32> {
        Ok(u32::from_le_bytes(self.read_array(address)?))
    }

    pub fn read_u64(&self, address: u64) -> Result<u64> {
        Ok(u64::from_le_bytes(self.read_array(address)?))
    }

    pub fn read_c_string(&self, address: u64) -> Result<String> {
        let mut bytes = Vec::new();
        while bytes.len() < MAX_STRING {
            let mut chunk = [0; STRING_CHUNK];
            let read = self.read_some(address + bytes.len() as u64, &mut chunk)?;
            if read == 0 {
                bail!("texto cortado na memória do MTGA em {address:#x}");
            }
            match chunk[..read].iter().position(|&byte| byte == 0) {
                Some(end) => {
                    bytes.extend_from_slice(&chunk[..end]);
                    return Ok(String::from_utf8_lossy(&bytes).into_owned());
                }
                None => bytes.extend_from_slice(&chunk[..read]),
            }
        }
        bail!("texto longo demais na memória do MTGA em {address:#x}")
    }

    fn read_array<const N: usize>(&self, address: u64) -> Result<[u8; N]> {
        let mut bytes = [0; N];
        self.read(address, &mut bytes)?;
        Ok(bytes)
    }
}

#[cfg(target_os = "linux")]
impl Process {
    pub fn find(name: &str) -> Result<Option<Self>> {
        let Some(pid) = pid_named(name)? else {
            return Ok(None);
        };

        let path = format!("/proc/{pid}/mem");
        let memory = std::fs::File::open(&path).map_err(|error| match error.kind() {
            std::io::ErrorKind::PermissionDenied => anyhow!(
                "o sistema não deixou ler a memória do MTGA (pid {pid}); \
                 abra o jogo pela Steam (Proton) ou ajuste kernel.yama.ptrace_scope"
            ),
            _ => anyhow!(error).context(format!("não consegui abrir {path}")),
        })?;

        Ok(Some(Self { pid, memory }))
    }

    pub fn module_base(&self, file_name: &str) -> Result<Option<u64>> {
        let maps = std::fs::read_to_string(format!("/proc/{}/maps", self.pid))
            .context("não consegui ler os módulos do MTGA")?;
        let suffix = format!("/{file_name}");

        Ok(maps
            .lines()
            .filter(|line| line.ends_with(&suffix))
            .find_map(|line| {
                let mut columns = line.split_whitespace();
                let range = columns.next()?;
                let file_offset = columns.nth(1)?;
                if u64::from_str_radix(file_offset, 16).ok()? != 0 {
                    return None;
                }
                u64::from_str_radix(range.split('-').next()?, 16).ok()
            }))
    }

    fn read_some(&self, address: u64, buffer: &mut [u8]) -> Result<usize> {
        use std::os::unix::fs::FileExt;

        self.memory
            .read_at(buffer, address)
            .with_context(|| format!("não consegui ler a memória do MTGA em {address:#x}"))
    }
}

#[cfg(target_os = "linux")]
fn pid_named(name: &str) -> Result<Option<u32>> {
    for entry in std::fs::read_dir("/proc").context("não consegui listar /proc")? {
        let entry = entry?;
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|text| text.parse::<u32>().ok())
        else {
            continue;
        };

        let comm = std::fs::read_to_string(entry.path().join("comm")).unwrap_or_default();
        if comm.trim_end() == name {
            return Ok(Some(pid));
        }
    }
    Ok(None)
}

#[cfg(not(target_os = "linux"))]
impl Process {
    pub fn find(_name: &str) -> Result<Option<Self>> {
        bail!("ler a coleção direto do MTGA por enquanto só funciona no Linux")
    }

    pub fn module_base(&self, _file_name: &str) -> Result<Option<u64>> {
        Ok(None)
    }

    fn read_some(&self, address: u64, _buffer: &mut [u8]) -> Result<usize> {
        bail!("não consegui ler a memória do MTGA em {address:#x}")
    }
}

pub fn le_i32(bytes: &[u8], offset: usize) -> Option<i32> {
    Some(i32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

pub fn le_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn little_endian_helpers_respect_bounds() {
        let bytes = [0x01, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF];
        assert_eq!(le_u32(&bytes, 0), Some(1));
        assert_eq!(le_i32(&bytes, 4), Some(-1));
        assert_eq!(le_u32(&bytes, 5), None);
    }
}
