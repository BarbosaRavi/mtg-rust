use super::pe;
use super::process::Process;
use anyhow::{Context, Result, bail};

const MONO_MODULE: &str = "mono-2.0-bdwgc.dll";
const ROOT_DOMAIN_EXPORT: &str = "mono_get_root_domain";
const MOV_RAX_RIP: [u8; 3] = [0x48, 0x8B, 0x05];
const MOV_RAX_RIP_LEN: u64 = 7;

// Layout of Unity's 64-bit mono-2.0-bdwgc.dll, checked on Unity 6000.3 (MTGA, 2026-10).
const DOMAIN_ASSEMBLIES: u64 = 0xA0;
const LIST_NEXT: u64 = 0x08;
const ASSEMBLY_NAME: u64 = 0x10;
const ASSEMBLY_IMAGE: u64 = 0x60;
const IMAGE_CLASS_CACHE_SIZE: u64 = 0x4D0 + 0x18;
const IMAGE_CLASS_CACHE_TABLE: u64 = 0x4D0 + 0x20;
const CLASS_ELEMENT_CLASS: u64 = 0x00;
const CLASS_KIND: u64 = 0x1B;
const CLASS_PARENT: u64 = 0x30;
const CLASS_NAME: u64 = 0x48;
const CLASS_NAMESPACE: u64 = 0x50;
const CLASS_VTABLE_SIZE: u64 = 0x5C;
const CLASS_ELEMENT_SIZE: u64 = 0x90;
const CLASS_FIELDS: u64 = 0x98;
const CLASS_RUNTIME_INFO: u64 = 0xD0;
const CLASS_GENERIC_CLASS: u64 = 0xF0;
const CLASS_FIELD_COUNT: u64 = 0x100;
const CLASS_NEXT_IN_CACHE: u64 = 0x108;
const RUNTIME_INFO_VTABLE: u64 = 0x08;
const VTABLE_SLOTS: u64 = 0x48;
const FIELD_SIZE: u64 = 0x20;
const FIELD_NAME: u64 = 0x08;
const FIELD_OFFSET: u64 = 0x18;
const TYPE_ATTRS: u64 = 0x08;
const ARRAY_LENGTH: u64 = 0x18;
const ARRAY_DATA: u64 = 0x20;
pub const OBJECT_HEADER: u64 = 0x10;

const KIND_MASK: u8 = 0x07;
const KIND_GENERIC_INSTANCE: u8 = 3;
const ATTR_STATIC: u32 = 0x10;
const POINTER_SIZE: u64 = 8;

const MAX_LINKS: usize = 200_000;
const MAX_CACHE_BUCKETS: usize = 1_000_000;
const MAX_FIELDS: u64 = 10_000;
const MAX_PARENTS: usize = 64;

pub struct Mono<'a> {
    process: &'a Process,
    domain: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct Field {
    pub offset: u64,
    pub is_static: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Array {
    pub element_class: u64,
    pub element_size: usize,
    pub length: usize,
    pub data: u64,
}

impl<'a> Mono<'a> {
    pub fn attach(process: &'a Process) -> Result<Self> {
        let base = process
            .module_base(MONO_MODULE)?
            .context("o MTGA ainda não carregou o Mono")?;
        let function = pe::export_address(process, base, ROOT_DOMAIN_EXPORT)?
            .with_context(|| format!("{MONO_MODULE} não exporta {ROOT_DOMAIN_EXPORT}"))?;

        let mut code = [0; 3];
        process.read(function, &mut code)?;
        if code != MOV_RAX_RIP {
            bail!("{ROOT_DOMAIN_EXPORT} mudou de formato ({code:02x?})");
        }
        let displacement = i64::from(process.read_i32(function + 3)?);
        let domain =
            process.read_u64((function + MOV_RAX_RIP_LEN).wrapping_add_signed(displacement))?;
        if domain == 0 {
            bail!("o Mono do MTGA ainda está iniciando");
        }

        Ok(Self { process, domain })
    }

    pub fn process(&self) -> &Process {
        self.process
    }

    pub fn image(&self, assembly_name: &str) -> Result<u64> {
        let mut node = self.pointer(self.domain + DOMAIN_ASSEMBLIES)?;
        for _ in 0..MAX_LINKS {
            if node == 0 {
                break;
            }
            let assembly = self.pointer(node)?;
            let name = self.pointer(assembly + ASSEMBLY_NAME)?;
            if name != 0 && self.process.read_c_string(name)? == assembly_name {
                return self.pointer(assembly + ASSEMBLY_IMAGE);
            }
            node = self.pointer(node + LIST_NEXT)?;
        }
        bail!("o assembly {assembly_name} não está carregado no MTGA")
    }

    pub fn class(&self, image: u64, namespace: &str, name: &str) -> Result<u64> {
        let size = usize::try_from(self.process.read_u32(image + IMAGE_CLASS_CACHE_SIZE)?)?;
        if size > MAX_CACHE_BUCKETS {
            bail!("tabela de classes do Mono com tamanho estranho ({size})");
        }
        let table = self.pointer(image + IMAGE_CLASS_CACHE_TABLE)?;
        let buckets = self.process.read_vec(table, size * POINTER_SIZE as usize)?;

        let mut visited = 0;
        for bucket in buckets.as_chunks::<{ POINTER_SIZE as usize }>().0 {
            let mut class = u64::from_le_bytes(*bucket);
            while class != 0 && visited < MAX_LINKS {
                visited += 1;
                if self.class_name(class)? == name
                    && self.string_at(class + CLASS_NAMESPACE)? == namespace
                {
                    return Ok(class);
                }
                class = self.pointer(class + CLASS_NEXT_IN_CACHE)?;
            }
        }
        bail!("a classe {name} não existe no MTGA (o jogo mudou?)")
    }

    pub fn field(&self, class: u64, name: &str) -> Result<Field> {
        let mut current = class;
        for _ in 0..MAX_PARENTS {
            if current == 0 {
                break;
            }
            if let Some(field) = self.own_field(current, name)? {
                return Ok(field);
            }
            current = self.pointer(current + CLASS_PARENT)?;
        }
        bail!(
            "o campo {name} não existe em {} (o jogo mudou?)",
            self.class_name(class)?
        )
    }

    pub fn static_object(&self, class: u64, name: &str) -> Result<u64> {
        let field = self.field(class, name)?;
        if !field.is_static {
            bail!("o campo {name} não é estático");
        }

        let runtime_info = self.pointer(class + CLASS_RUNTIME_INFO)?;
        if runtime_info == 0 {
            bail!(
                "a classe {} ainda não foi inicializada",
                self.class_name(class)?
            );
        }
        let vtable = self.pointer(runtime_info + RUNTIME_INFO_VTABLE)?;
        let slots = u64::try_from(self.process.read_i32(class + CLASS_VTABLE_SIZE)?)?;
        let statics = self.pointer(vtable + VTABLE_SLOTS + slots * POINTER_SIZE)?;
        if statics == 0 {
            bail!(
                "os campos estáticos de {} ainda não existem",
                self.class_name(class)?
            );
        }
        self.pointer(statics + field.offset)
    }

    pub fn object_field(&self, object: u64, name: &str) -> Result<u64> {
        let field = self.instance_field(object, name)?;
        self.pointer(object + field.offset)
    }

    pub fn i32_field(&self, object: u64, name: &str) -> Result<i32> {
        let field = self.instance_field(object, name)?;
        self.process.read_i32(object + field.offset)
    }

    pub fn array(&self, array: u64) -> Result<Array> {
        let class = self.class_of(array)?;
        Ok(Array {
            element_class: self.pointer(class + CLASS_ELEMENT_CLASS)?,
            element_size: usize::try_from(self.process.read_i32(class + CLASS_ELEMENT_SIZE)?)?,
            length: usize::try_from(self.process.read_u32(array + ARRAY_LENGTH)?)?,
            data: array + ARRAY_DATA,
        })
    }

    fn instance_field(&self, object: u64, name: &str) -> Result<Field> {
        let field = self.field(self.class_of(object)?, name)?;
        if field.is_static {
            bail!("o campo {name} é estático");
        }
        Ok(field)
    }

    fn class_of(&self, object: u64) -> Result<u64> {
        if object == 0 {
            bail!("referência nula na memória do MTGA");
        }
        self.pointer(self.pointer(object)?)
    }

    fn own_field(&self, class: u64, name: &str) -> Result<Option<Field>> {
        let definition = self.definition(class)?;
        let count = u64::try_from(self.process.read_i32(definition + CLASS_FIELD_COUNT)?)
            .unwrap_or(0)
            .min(MAX_FIELDS);
        let fields = match self.pointer(class + CLASS_FIELDS)? {
            0 => self.pointer(definition + CLASS_FIELDS)?,
            fields => fields,
        };
        if fields == 0 {
            return Ok(None);
        }

        for index in 0..count {
            let field = fields + index * FIELD_SIZE;
            let field_type = self.pointer(field)?;
            if field_type == 0 {
                break;
            }
            if self.string_at(field + FIELD_NAME)? != name {
                continue;
            }
            let attrs = self.process.read_u32(field_type + TYPE_ATTRS)?;
            let offset = self.process.read_i32(field + FIELD_OFFSET)?;
            return Ok(Some(Field {
                offset: u64::try_from(offset).context("campo sem posição na memória")?,
                is_static: attrs & ATTR_STATIC != 0,
            }));
        }
        Ok(None)
    }

    fn definition(&self, class: u64) -> Result<u64> {
        let kind = self.process.read_u8(class + CLASS_KIND)? & KIND_MASK;
        if kind != KIND_GENERIC_INSTANCE {
            return Ok(class);
        }
        self.pointer(self.pointer(class + CLASS_GENERIC_CLASS)?)
    }

    fn class_name(&self, class: u64) -> Result<String> {
        self.string_at(class + CLASS_NAME)
    }

    fn string_at(&self, address: u64) -> Result<String> {
        match self.pointer(address)? {
            0 => Ok(String::new()),
            text => self.process.read_c_string(text),
        }
    }

    fn pointer(&self, address: u64) -> Result<u64> {
        self.process.read_u64(address)
    }
}
