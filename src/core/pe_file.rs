//! PE 文件解析与修改：支持解析 PE 头、查找节、添加新节、写入文件。
//! 纯 Rust 实现，不依赖第三方 PE 库。
//!
//! 移植自 Electron 版 pe-file.ts（Java 版 PEFile.java），
//! 步骤编号与注释保持一致。

use std::fs;
use std::path::Path;

use super::byte_utils::{
    align_up, find_bytes, read_i32_le, read_u16_le, read_u32_le, write_u16_le, write_u32_le,
};
use super::Result;

// ── PE 结构常量 ──
const DOS_E_LFANEW_OFFSET: usize = 0x3c;
const COFF_HEADER_SIZE: usize = 20;
const SECTION_HEADER_SIZE: usize = 40;

// Optional Header 字段偏移 (PE32 和 PE32+ 相同位置)
const OH_SIZEOF_INITIALIZED_DATA: usize = 8;
const OH_SECTION_ALIGNMENT: usize = 32;
const OH_FILE_ALIGNMENT: usize = 36;
const OH_SIZEOF_IMAGE: usize = 56;
const OH_CHECKSUM: usize = 64;

/// 表示 PE 文件中的一个节（对应 TS 版 PESection 接口）
/// 字段与 Java/TS 版逐一对齐，部分字段仅在写入 PE 头时使用
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PeSection {
    pub name: String,
    pub virtual_size: u32,
    pub virtual_address: u32,
    pub size_of_raw_data: u32,
    pub pointer_to_raw_data: u32,
    pub characteristics: u32,
    /// 在文件中节头的偏移
    pub header_offset: usize,
}

/// PE 文件：解析后的字节数据 + 头部元信息 + 节表
pub struct PeFile {
    data: Vec<u8>,
    pe_offset: usize,       // PE 签名位置 (e_lfanew)
    coff_offset: usize,     // COFF 头位置
    optional_offset: usize, // Optional Header 位置
    section_table_offset: usize,
    number_of_sections: u16,
    size_of_optional_header: u16,
    section_alignment: u32,
    file_alignment: u32,
    /// 是否为 PE32+（64 位）
    pub is_pe32_plus: bool,
    /// 节信息
    pub sections: Vec<PeSection>,
}

impl PeFile {
    /// 从文件路径解析 PE 文件
    pub fn parse(file_path: &Path) -> Result<PeFile> {
        let data = fs::read(file_path)?;
        let mut pe = PeFile {
            data,
            pe_offset: 0,
            coff_offset: 0,
            optional_offset: 0,
            section_table_offset: 0,
            number_of_sections: 0,
            size_of_optional_header: 0,
            section_alignment: 0,
            file_alignment: 0,
            is_pe32_plus: false,
            sections: Vec::new(),
        };
        pe.parse_headers()?;
        Ok(pe)
    }

    fn parse_headers(&mut self) -> Result<()> {
        // 1. DOS Header → e_lfanew
        self.pe_offset = read_i32_le(&self.data, DOS_E_LFANEW_OFFSET) as usize;

        // 2. 验证 PE 签名 "PE\0\0"
        if self.pe_offset + 4 > self.data.len()
            || self.data[self.pe_offset] != 0x50 // 'P'
            || self.data[self.pe_offset + 1] != 0x45 // 'E'
            || self.data[self.pe_offset + 2] != 0
            || self.data[self.pe_offset + 3] != 0
        {
            return Err("无效的 PE 签名".into());
        }

        // 3. COFF Header
        self.coff_offset = self.pe_offset + 4;
        self.number_of_sections = read_u16_le(&self.data, self.coff_offset + 2);
        self.size_of_optional_header = read_u16_le(&self.data, self.coff_offset + 16);

        // 4. Optional Header
        self.optional_offset = self.pe_offset + 4 + COFF_HEADER_SIZE;
        let magic = read_u16_le(&self.data, self.optional_offset);
        self.is_pe32_plus = magic == 0x20b; // PE32+ (64-bit)

        self.section_alignment = read_u32_le(&self.data, self.optional_offset + OH_SECTION_ALIGNMENT);
        self.file_alignment = read_u32_le(&self.data, self.optional_offset + OH_FILE_ALIGNMENT);

        // 5. Section Table
        self.section_table_offset = self.optional_offset + self.size_of_optional_header as usize;
        for i in 0..self.number_of_sections as usize {
            let off = self.section_table_offset + i * SECTION_HEADER_SIZE;
            if off + SECTION_HEADER_SIZE > self.data.len() {
                return Err("节表越界，PE 文件已损坏".into());
            }

            // 节名: 8 字节，以 null 结尾（节名均为 ASCII，latin1 == utf8）
            let name_bytes = &self.data[off..off + 8];
            let name_end = name_bytes.iter().position(|&b| b == 0).unwrap_or(8);
            let name = String::from_utf8_lossy(&name_bytes[..name_end]).into_owned();

            self.sections.push(PeSection {
                name,
                virtual_size: read_u32_le(&self.data, off + 8),
                virtual_address: read_u32_le(&self.data, off + 12),
                size_of_raw_data: read_u32_le(&self.data, off + 16),
                pointer_to_raw_data: read_u32_le(&self.data, off + 20),
                characteristics: read_u32_le(&self.data, off + 36),
                header_offset: off, // 在文件中节头的偏移
            });
        }
        Ok(())
    }

    /// 按名称查找节
    pub fn get_section(&self, name: &str) -> Option<&PeSection> {
        self.sections.iter().find(|sec| sec.name == name)
    }

    /// 添加新节到 PE 文件。
    ///
    /// * `name`            - 节名 (最长 8 字节)
    /// * `content`         - 节数据
    /// * `characteristics` - 节属性
    ///
    /// 返回新添加的节信息（VirtualAddress 等字段）。
    pub fn add_section(
        &mut self,
        name: &str,
        content: &[u8],
        characteristics: u32,
    ) -> Result<PeSection> {
        // 1. 检查节名长度（8 字节缓冲，超长报错）
        let name_b = name.as_bytes();
        if name_b.len() > 8 {
            return Err("节名超过 8 字节限制".into());
        }
        let mut name_bytes = [0u8; 8];
        name_bytes[..name_b.len()].copy_from_slice(name_b);

        // 2. 检查是否有足够的头部空间
        let header_end =
            self.section_table_offset + self.number_of_sections as usize * SECTION_HEADER_SIZE;
        let mut first_raw_data = u32::MAX;
        for sec in &self.sections {
            if sec.pointer_to_raw_data > 0 && sec.pointer_to_raw_data < first_raw_data {
                first_raw_data = sec.pointer_to_raw_data;
            }
        }
        if header_end + SECTION_HEADER_SIZE > first_raw_data as usize {
            return Err("PE 头部空间不足，无法添加新节".into());
        }

        // 3. 计算新节的 VirtualAddress (在最后一个节之后，对齐到 SectionAlignment)
        let mut max_va_end: u32 = 0;
        for sec in &self.sections {
            let va_end = sec.virtual_address.wrapping_add(sec.virtual_size);
            if va_end > max_va_end {
                max_va_end = va_end;
            }
        }
        let new_virtual_address =
            align_up(max_va_end as u64, self.section_alignment as u64) as u32;

        // 4. 计算新节的 PointerToRawData (在文件末尾，对齐到 FileAlignment)
        let new_pointer_to_raw_data = align_up(self.data.len() as u64, self.file_alignment as u64) as u32;

        // 5. 计算 SizeOfRawData (对齐到 FileAlignment)
        let new_size_of_raw_data = align_up(content.len() as u64, self.file_alignment as u64) as u32;

        // 6. 创建新的缓冲区 (原数据 + 填充 + 节数据，其余自动填充 0)
        let new_size = new_pointer_to_raw_data as usize + new_size_of_raw_data as usize;
        let mut new_data = vec![0u8; new_size];
        new_data[..self.data.len()].copy_from_slice(&self.data);
        // 复制节数据到末尾
        let raw_start = new_pointer_to_raw_data as usize;
        new_data[raw_start..raw_start + content.len()].copy_from_slice(content);

        self.data = new_data;

        // 7. 写入新节头
        let h = header_end;
        // Name (8 bytes)
        self.data[h..h + 8].copy_from_slice(&name_bytes);
        // VirtualSize
        write_u32_le(&mut self.data, h + 8, content.len() as u32);
        // VirtualAddress
        write_u32_le(&mut self.data, h + 12, new_virtual_address);
        // SizeOfRawData
        write_u32_le(&mut self.data, h + 16, new_size_of_raw_data);
        // PointerToRawData
        write_u32_le(&mut self.data, h + 20, new_pointer_to_raw_data);
        // PointerToRelocations = 0 (vec 已置零)
        // PointerToLinenumbers = 0
        // NumberOfRelocations = 0
        // NumberOfLinenumbers = 0
        // Characteristics
        write_u32_le(&mut self.data, h + 36, characteristics);

        // 8. 更新 NumberOfSections
        self.number_of_sections += 1;
        write_u16_le(&mut self.data, self.coff_offset + 2, self.number_of_sections);

        // 9. 更新 SizeOfImage
        let new_size_of_image = align_up(
            new_virtual_address as u64 + content.len() as u64,
            self.section_alignment as u64,
        ) as u32;
        write_u32_le(&mut self.data, self.optional_offset + OH_SIZEOF_IMAGE, new_size_of_image);

        // 10. 更新 SizeOfInitializedData
        let size_of_init_data =
            read_u32_le(&self.data, self.optional_offset + OH_SIZEOF_INITIALIZED_DATA);
        write_u32_le(
            &mut self.data,
            self.optional_offset + OH_SIZEOF_INITIALIZED_DATA,
            size_of_init_data.wrapping_add(new_size_of_raw_data),
        );

        // 11. 清除 CheckSum (设为 0 表示不校验)
        write_u32_le(&mut self.data, self.optional_offset + OH_CHECKSUM, 0);

        // 12. 创建并注册节对象
        let new_sec = PeSection {
            name: name.to_string(),
            virtual_size: content.len() as u32,
            virtual_address: new_virtual_address,
            size_of_raw_data: new_size_of_raw_data,
            pointer_to_raw_data: new_pointer_to_raw_data,
            characteristics,
            header_offset: h,
        };
        self.sections.push(new_sec.clone());

        Ok(new_sec)
    }

    /// 将修改后的 PE 数据写入文件（先内存 patch 再整体写文件，与 Java/TS 版策略一致）
    pub fn write(&self, file_path: &Path) -> Result<()> {
        fs::write(file_path, &self.data)?;
        Ok(())
    }

    /// 在指定文件偏移处写入补丁字节码
    pub fn patch_at(&mut self, file_offset: usize, patch_bytes: &[u8]) {
        self.data[file_offset..file_offset + patch_bytes.len()].copy_from_slice(patch_bytes);
    }

    /// 在文件数据中搜索字节模式，返回文件偏移（未找到 -1）
    pub fn find_bytes(&self, pattern: &[u8]) -> i64 {
        find_bytes(&self.data, pattern)
    }
}
