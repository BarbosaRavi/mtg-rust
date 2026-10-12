use crate::memory::{Mono, OBJECT_HEADER, Process, le_i32, le_u32};
use anyhow::{Context, Result, bail};

const GAME_PROCESS: &str = "MTGA.exe";
const GAME_ASSEMBLY: &str = "Core";
const ROOT_CLASS: &str = "WrapperController";
const ROOT_FIELD: &str = "<Instance>k__BackingField";
const PATH_TO_CARDS: [&str; 3] = [
    "<InventoryManager>k__BackingField",
    "InventoryServiceWrapper",
    "<Cards>k__BackingField",
];
const NOT_LOADED: &str = "o MTGA ainda não carregou a coleção (faça login e espere a tela inicial)";
const MAX_ENTRIES_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnedCard {
    pub grp_id: u32,
    pub owned: u32,
}

pub fn read_owned_cards() -> Result<Vec<OwnedCard>> {
    let process = Process::find(GAME_PROCESS)?.context("o MTGA não está aberto")?;
    let mono = Mono::attach(&process)?;
    let image = mono.image(GAME_ASSEMBLY)?;
    let root = mono.class(image, "", ROOT_CLASS)?;

    let mut object = mono.static_object(root, ROOT_FIELD)?;
    for field in PATH_TO_CARDS {
        object = mono.object_field(loaded(object)?, field)?;
    }
    read_card_dictionary(&mono, loaded(object)?)
}

fn read_card_dictionary(mono: &Mono, dictionary: u64) -> Result<Vec<OwnedCard>> {
    let entries = mono.array(loaded(mono.object_field(dictionary, "_entries")?)?)?;
    let count = usize::try_from(mono.i32_field(dictionary, "_count")?)?;
    let layout = EntryLayout::read(mono, entries.element_class, entries.element_size)?;

    let size = count * entries.element_size;
    if count > entries.length || size > MAX_ENTRIES_BYTES {
        bail!("o dicionário de cartas do MTGA tem um formato inesperado");
    }
    let bytes = mono.process().read_vec(entries.data, size)?;

    Ok(bytes
        .chunks_exact(entries.element_size)
        .filter_map(|entry| layout.parse(entry))
        .collect())
}

fn loaded(object: u64) -> Result<u64> {
    if object == 0 {
        bail!(NOT_LOADED);
    }
    Ok(object)
}

struct EntryLayout {
    hash_code: usize,
    key: usize,
    value: usize,
}

impl EntryLayout {
    fn read(mono: &Mono, class: u64, size: usize) -> Result<Self> {
        let offset = |name: &str| -> Result<usize> {
            let field = mono.field(class, name)?;
            let inside_struct = field
                .offset
                .checked_sub(OBJECT_HEADER)
                .context("campo do dicionário fora do lugar")?;
            Ok(usize::try_from(inside_struct)?)
        };

        let layout = Self {
            hash_code: offset("hashCode")?,
            key: offset("key")?,
            value: offset("value")?,
        };
        if [layout.hash_code, layout.key, layout.value]
            .iter()
            .any(|&offset| offset + 4 > size)
        {
            bail!("o dicionário de cartas do MTGA tem um formato inesperado");
        }
        Ok(layout)
    }

    fn parse(&self, entry: &[u8]) -> Option<OwnedCard> {
        let removed = le_i32(entry, self.hash_code)? < 0;
        let owned = u32::try_from(le_i32(entry, self.value)?).ok()?;
        if removed || owned == 0 {
            return None;
        }
        Some(OwnedCard {
            grp_id: le_u32(entry, self.key)?,
            owned,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(hash_code: i32, key: u32, value: i32) -> Vec<u8> {
        [
            hash_code.to_le_bytes(),
            (-1i32).to_le_bytes(),
            key.to_le_bytes(),
            value.to_le_bytes(),
        ]
        .concat()
    }

    #[test]
    fn parse_skips_removed_and_unowned_entries() {
        let layout = EntryLayout {
            hash_code: 0,
            key: 8,
            value: 12,
        };

        assert_eq!(
            layout.parse(&entry(75452, 75452, 2)),
            Some(OwnedCard {
                grp_id: 75452,
                owned: 2
            })
        );
        assert_eq!(layout.parse(&entry(-1, 75452, 2)), None);
        assert_eq!(layout.parse(&entry(75452, 75452, 0)), None);
    }
}
