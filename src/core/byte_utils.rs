//! 字节操作工具：模式搜索、小端读写、对齐计算。
//!
//! 移植自 Electron 版 byte-utils.ts（Java 版 ByteUtils.java）。
//!
//! Node.js 的 Buffer 原生提供小端读写与模式搜索，Rust 标准库没有对应的
//! 切片级 API，因此这里给出手写实现：
//!   indexOf(data, pattern)   → find_bytes（首字节快速路径的原生搜索）
//!   readIntLE / readShortLE  → read_i32_le / read_u16_le（from_le_bytes）
//!   writeIntLE / writeShortLE→ write_i32_le / write_u16_le（to_le_bytes）
//!   alignUp(value, alignment)→ align_up（u64 运算避免 PE 字段接近 4GB 时溢出）

/// 在 `haystack` 中搜索 `needle` 的第一个匹配位置，未找到返回 -1。
///
/// 首字节快速路径：绝大多数窗口只需比较 1 字节即可跳过，
/// 对几十 MB 的 libcc.dll 搜索 71 字节模式，release 下毫秒级完成。
pub fn find_bytes(haystack: &[u8], needle: &[u8]) -> i64 {
    let n = needle.len();
    if n == 0 || haystack.len() < n {
        return -1;
    }
    let first = needle[0];
    let last_pos = haystack.len() - n;
    for i in 0..=last_pos {
        if unsafe { *haystack.get_unchecked(i) } == first
            && haystack[i..i + n] == *needle
        {
            return i as i64;
        }
    }
    -1
}

/// 将 value 向上对齐到 alignment（要求 alignment 为 2 的幂）。
///
/// 内部用 u64 运算：PE 的 VirtualAddress + VirtualSize 可能接近 u32 上限，
/// 与 JS（double，无 32 位回绕）行为保持一致。
pub fn align_up(value: u64, alignment: u64) -> u64 {
    (value + alignment - 1) & !(alignment - 1)
}

/// 读取无符号 16 位小端整数（对应 Node readUInt16LE）
pub fn read_u16_le(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

/// 读取无符号 32 位小端整数（对应 Node readUInt32LE）
pub fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

/// 读取有符号 32 位小端整数（对应 Node readInt32LE，用于 e_lfanew）
pub fn read_i32_le(data: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

/// 写入无符号 16 位小端整数（对应 Node writeUInt16LE）
pub fn write_u16_le(data: &mut [u8], offset: usize, value: u16) {
    data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

/// 写入无符号 32 位小端整数（对应 Node writeUInt32LE）
pub fn write_u32_le(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

/// 写入有符号 32 位小端整数，value 可为负数（如 RIP 相对位移）
pub fn write_i32_le(data: &mut [u8], offset: usize, value: i32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_bytes() {
        let data = [0x01, 0x02, 0x03, 0x04, 0x03, 0x04];
        assert_eq!(find_bytes(&data, &[0x03, 0x04]), 2);
        assert_eq!(find_bytes(&data, &[0x05]), -1);
        assert_eq!(find_bytes(&data, &[]), -1);
    }

    #[test]
    fn test_align_up() {
        assert_eq!(align_up(0, 0x1000), 0);
        assert_eq!(align_up(1, 0x1000), 0x1000);
        assert_eq!(align_up(0x1000, 0x1000), 0x1000);
        assert_eq!(align_up(0x1001, 0x1000), 0x2000);
    }

    #[test]
    fn test_le_roundtrip() {
        let mut buf = [0u8; 8];
        write_u32_le(&mut buf, 0, 0xDEADBEEF);
        assert_eq!(read_u32_le(&buf, 0), 0xDEADBEEF);
        write_i32_le(&mut buf, 4, -2);
        assert_eq!(read_i32_le(&buf, 4), -2);
        write_u16_le(&mut buf, 0, 0xBEEF);
        assert_eq!(read_u16_le(&buf, 0), 0xBEEF);
    }
}
