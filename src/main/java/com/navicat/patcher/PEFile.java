package com.navicat.patcher;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * PE 文件解析与修改：支持解析 PE 头、查找节、添加新节、写入文件。
 * 纯 Java 实现，不依赖第三方 PE 库。
 */
public class PEFile {

    // ── PE 结构常量 ──
    private static final int DOS_E_LFANEW_OFFSET = 0x3C;
    private static final int COFF_HEADER_SIZE = 20;
    private static final int SECTION_HEADER_SIZE = 40;

    // Optional Header 字段偏移 (PE32 和 PE32+ 相同位置)
    private static final int OH_SIZEOF_INITIALIZED_DATA = 8;
    private static final int OH_SECTION_ALIGNMENT = 32;
    private static final int OH_FILE_ALIGNMENT = 36;
    private static final int OH_SIZEOF_IMAGE = 56;
    private static final int OH_SIZEOF_HEADERS = 60;
    private static final int OH_CHECKSUM = 64;

    // ── 解析后的数据 ──
    private byte[] data;
    private int peOffset;          // PE 签名位置 (e_lfanew)
    private int coffOffset;        // COFF 头位置
    private int optionalOffset;    // Optional Header 位置
    private int sectionTableOffset;// 节表位置
    private short numberOfSections;
    private short sizeOfOptionalHeader;
    private int sectionAlignment;
    private int fileAlignment;
    private boolean isPE32Plus;

    // ── 节信息 ──
    private final List<PESection> sections = new ArrayList<>();

    /**
     * 表示 PE 文件中的一个节
     */
    public static class PESection {
        public String name;
        public int virtualSize;
        public int virtualAddress;
        public int sizeOfRawData;
        public int pointerToRawData;
        public int characteristics;
        public int headerOffset; // 在文件中节头的偏移
    }

    private PEFile() {}

    /**
     * 从文件路径解析 PE 文件
     */
    public static PEFile parse(String filePath) throws IOException {
        PEFile pe = new PEFile();
        pe.data = Files.readAllBytes(Path.of(filePath));
        pe.parseHeaders();
        return pe;
    }

    private void parseHeaders() {
        // 1. DOS Header → e_lfanew
        peOffset = ByteUtils.readIntLE(data, DOS_E_LFANEW_OFFSET);

        // 2. 验证 PE 签名 "PE\0\0"
        if (data[peOffset] != 'P' || data[peOffset + 1] != 'E'
                || data[peOffset + 2] != 0 || data[peOffset + 3] != 0) {
            throw new RuntimeException("无效的 PE 签名");
        }

        // 3. COFF Header
        coffOffset = peOffset + 4;
        numberOfSections = ByteUtils.readShortLE(data, coffOffset + 2);
        sizeOfOptionalHeader = ByteUtils.readShortLE(data, coffOffset + 16);

        // 4. Optional Header
        optionalOffset = peOffset + 4 + COFF_HEADER_SIZE;
        short magic = ByteUtils.readShortLE(data, optionalOffset);
        isPE32Plus = (magic == 0x20B); // PE32+ (64-bit)

        sectionAlignment = ByteUtils.readIntLE(data, optionalOffset + OH_SECTION_ALIGNMENT);
        fileAlignment = ByteUtils.readIntLE(data, optionalOffset + OH_FILE_ALIGNMENT);

        // 5. Section Table
        sectionTableOffset = optionalOffset + sizeOfOptionalHeader;
        for (int i = 0; i < numberOfSections; i++) {
            int off = sectionTableOffset + i * SECTION_HEADER_SIZE;
            PESection sec = new PESection();
            sec.headerOffset = off;

            // 节名: 8字节，以 null 结尾
            int nameEnd = off;
            while (nameEnd < off + 8 && data[nameEnd] != 0) nameEnd++;
            sec.name = new String(data, off, nameEnd - off);

            sec.virtualSize = ByteUtils.readIntLE(data, off + 8);
            sec.virtualAddress = ByteUtils.readIntLE(data, off + 12);
            sec.sizeOfRawData = ByteUtils.readIntLE(data, off + 16);
            sec.pointerToRawData = ByteUtils.readIntLE(data, off + 20);
            sec.characteristics = ByteUtils.readIntLE(data, off + 36);

            sections.add(sec);
        }
    }

    /**
     * 按名称查找节
     */
    public PESection getSection(String name) {
        for (PESection sec : sections) {
            if (sec.name.equals(name)) return sec;
        }
        return null;
    }

    /**
     * 添加新节到 PE 文件。
     * @param name           节名 (最长 8 字节)
     * @param content        节数据
     * @param characteristics 节属性
     * @return 新添加的节信息
     */
    public PESection addSection(String name, byte[] content, int characteristics) {
        // 1. 检查节名长度
        byte[] nameBytes = new byte[8];
        System.arraycopy(name.getBytes(), 0, nameBytes, 0,
                Math.min(name.length(), 8));

        // 2. 检查是否有足够的头部空间
        int headerEnd = sectionTableOffset + numberOfSections * SECTION_HEADER_SIZE;
        int firstRawData = Integer.MAX_VALUE;
        for (PESection sec : sections) {
            if (sec.pointerToRawData > 0 && sec.pointerToRawData < firstRawData) {
                firstRawData = sec.pointerToRawData;
            }
        }
        if (headerEnd + SECTION_HEADER_SIZE > firstRawData) {
            throw new RuntimeException("PE 头部空间不足，无法添加新节");
        }

        // 3. 计算新节的 VirtualAddress (在最后一个节之后，对齐到 SectionAlignment)
        int maxVAEnd = 0;
        for (PESection sec : sections) {
            int vaEnd = sec.virtualAddress + sec.virtualSize;
            if (vaEnd > maxVAEnd) maxVAEnd = vaEnd;
        }
        int newVirtualAddress = ByteUtils.alignUp(maxVAEnd, sectionAlignment);

        // 4. 计算新节的 PointerToRawData (在文件末尾，对齐到 FileAlignment)
        int newPointerToRawData = ByteUtils.alignUp(data.length, fileAlignment);

        // 5. 计算 SizeOfRawData (对齐到 FileAlignment)
        int newSizeOfRawData = ByteUtils.alignUp(content.length, fileAlignment);

        // 6. 创建新的字节数组 (原数据 + 填充 + 节数据)
        int newSize = newPointerToRawData + newSizeOfRawData;
        byte[] newData = new byte[newSize];
        System.arraycopy(data, 0, newData, 0, data.length);
        // 复制节数据到末尾
        System.arraycopy(content, 0, newData, newPointerToRawData, content.length);
        // 其余部分自动填充 0

        this.data = newData;

        // 7. 写入新节头
        int newHeaderOffset = headerEnd;
        // Name (8 bytes)
        System.arraycopy(nameBytes, 0, data, newHeaderOffset, 8);
        // VirtualSize
        ByteUtils.writeIntLE(data, newHeaderOffset + 8, content.length);
        // VirtualAddress
        ByteUtils.writeIntLE(data, newHeaderOffset + 12, newVirtualAddress);
        // SizeOfRawData
        ByteUtils.writeIntLE(data, newHeaderOffset + 16, newSizeOfRawData);
        // PointerToRawData
        ByteUtils.writeIntLE(data, newHeaderOffset + 20, newPointerToRawData);
        // PointerToRelocations = 0 (already zeroed)
        // PointerToLinenumbers = 0
        // NumberOfRelocations = 0
        // NumberOfLinenumbers = 0
        // Characteristics
        ByteUtils.writeIntLE(data, newHeaderOffset + 36, characteristics);

        // 8. 更新 NumberOfSections
        numberOfSections++;
        ByteUtils.writeShortLE(data, coffOffset + 2, numberOfSections);

        // 9. 更新 SizeOfImage
        int newSizeOfImage = ByteUtils.alignUp(newVirtualAddress + content.length, sectionAlignment);
        ByteUtils.writeIntLE(data, optionalOffset + OH_SIZEOF_IMAGE, newSizeOfImage);

        // 10. 更新 SizeOfInitializedData
        int sizeOfInitData = ByteUtils.readIntLE(data, optionalOffset + OH_SIZEOF_INITIALIZED_DATA);
        ByteUtils.writeIntLE(data, optionalOffset + OH_SIZEOF_INITIALIZED_DATA,
                sizeOfInitData + newSizeOfRawData);

        // 11. 清除 CheckSum (设为 0 表示不校验)
        ByteUtils.writeIntLE(data, optionalOffset + OH_CHECKSUM, 0);

        // 12. 创建并注册节对象
        PESection newSec = new PESection();
        newSec.name = name;
        newSec.virtualSize = content.length;
        newSec.virtualAddress = newVirtualAddress;
        newSec.sizeOfRawData = newSizeOfRawData;
        newSec.pointerToRawData = newPointerToRawData;
        newSec.characteristics = characteristics;
        newSec.headerOffset = newHeaderOffset;
        sections.add(newSec);

        return newSec;
    }

    /**
     * 将修改后的 PE 数据写入文件
     */
    public void write(String filePath) throws IOException {
        Files.write(Path.of(filePath), data);
    }

    /**
     * 在指定文件偏移处写入补丁字节码
     */
    public void patchAt(int fileOffset, byte[] patchBytes) {
        System.arraycopy(patchBytes, 0, data, fileOffset, patchBytes.length);
    }

    /**
     * 在文件数据中搜索字节模式，返回文件偏移
     */
    public int findBytes(byte[] pattern) {
        return ByteUtils.indexOf(data, pattern);
    }

    // ── Getter ──
    public byte[] getData() { return data; }
    public int getSectionAlignment() { return sectionAlignment; }
    public int getFileAlignment() { return fileAlignment; }
    public boolean isPE32Plus() { return isPE32Plus; }
    public List<PESection> getSections() { return sections; }
}
