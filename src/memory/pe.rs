use super::process::Process;
use anyhow::{Result, bail};

const PE_HEADER_POINTER: u64 = 0x3C;
const PE_SIGNATURE: u32 = 0x4550;
const OPTIONAL_HEADER_MAGIC: u64 = 0x18;
const PE32_PLUS: u16 = 0x20B;
const EXPORT_DIRECTORY: u64 = 0x88;
const EXPORT_NAME_COUNT: u64 = 0x18;
const EXPORT_FUNCTIONS: u64 = 0x1C;
const EXPORT_NAMES: u64 = 0x20;
const EXPORT_ORDINALS: u64 = 0x24;

pub fn export_address(process: &Process, base: u64, name: &str) -> Result<Option<u64>> {
    let header = base + u64::from(process.read_u32(base + PE_HEADER_POINTER)?);
    if process.read_u32(header)? != PE_SIGNATURE
        || process.read_u16(header + OPTIONAL_HEADER_MAGIC)? != PE32_PLUS
    {
        bail!("o módulo em {base:#x} não é uma DLL de 64 bits");
    }

    let rva = |address: u64| -> Result<u64> { Ok(base + u64::from(process.read_u32(address)?)) };
    let directory = rva(header + EXPORT_DIRECTORY)?;
    let name_count = process.read_u32(directory + EXPORT_NAME_COUNT)?;
    let functions = rva(directory + EXPORT_FUNCTIONS)?;
    let names = rva(directory + EXPORT_NAMES)?;
    let ordinals = rva(directory + EXPORT_ORDINALS)?;

    for index in 0..u64::from(name_count) {
        if process.read_c_string(rva(names + 4 * index)?)? == name {
            let ordinal = u64::from(process.read_u16(ordinals + 2 * index)?);
            return rva(functions + 4 * ordinal).map(Some);
        }
    }
    Ok(None)
}
