package com.navicat.patcher;

import com.fasterxml.jackson.databind.ObjectMapper;

import java.io.FileNotFoundException;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.time.Instant;
import java.util.Map;
import java.util.function.Consumer;

/**
 * 核心补丁服务：备份文件、生成密钥、应用补丁、离线激活。
 * 对应 Python 脚本的完整流程。
 */
public class PatcherService {

    // ── 字节码常量（与 Python 脚本完全一致）──

    public static final byte[] ORIGINAL_BYTECODE = new byte[]{
            // 48 8b d0 48 8b cf ff d3 48 89 46 20 48 8b 55 10
            (byte) 0x48, (byte) 0x8b, (byte) 0xd0, (byte) 0x48, (byte) 0x8b, (byte) 0xcf, (byte) 0xff, (byte) 0xd3,
            (byte) 0x48, (byte) 0x89, (byte) 0x46, (byte) 0x20, (byte) 0x48, (byte) 0x8b, (byte) 0x55, (byte) 0x10,
            // 48 83 fa 0f 76 34 48 ff c2 48 8b 4d f8 48 8b c1
            (byte) 0x48, (byte) 0x83, (byte) 0xfa, (byte) 0x0f, (byte) 0x76, (byte) 0x34, (byte) 0x48, (byte) 0xff, (byte) 0xc2,
            (byte) 0x48, (byte) 0x8b, (byte) 0x4d, (byte) 0xf8, (byte) 0x48, (byte) 0x8b, (byte) 0xc1,
            // 48 81 fa 00 10 00 00 72 1c 48 83 c2 27 48 8b 49
            (byte) 0x48, (byte) 0x81, (byte) 0xfa, (byte) 0x00, (byte) 0x10, (byte) 0x00, (byte) 0x00, (byte) 0x72, (byte) 0x1c,
            (byte) 0x48, (byte) 0x83, (byte) 0xc2, (byte) 0x27, (byte) 0x48, (byte) 0x8b, (byte) 0x49,
            // f8 48 2b c1 48 83 c0 f8 48 83
            (byte) 0xf8, (byte) 0x48, (byte) 0x2b, (byte) 0xc1, (byte) 0x48, (byte) 0x83, (byte) 0xc0, (byte) 0xf8, (byte) 0x48, (byte) 0x83,
    };

    public static final byte[] PATCH_BYTECODE = new byte[]{
            // 48 8d 0d 00 00 00 00  → lea rcx, [rip + disp32]
            (byte) 0x48, (byte) 0x8d, (byte) 0x0d, (byte) 0x00, (byte) 0x00, (byte) 0x00, (byte) 0x00,
            // 48 89 08             → mov [rax], rcx
            (byte) 0x48, (byte) 0x89, (byte) 0x08,
            // 48 89 c2             → mov rdx, rax
            (byte) 0x48, (byte) 0x89, (byte) 0xc2,
            // 48 89 f9             → mov rcx, rdi
            (byte) 0x48, (byte) 0x89, (byte) 0xf9,
            // ff d3                → call rbx
            (byte) 0xff, (byte) 0xd3,
            // 48 89 46 20          → mov [rsi+20h], rax
            (byte) 0x48, (byte) 0x89, (byte) 0x46, (byte) 0x20,
            // 48 8b 55 10          → mov rdx, [rbp+10h]
            (byte) 0x48, (byte) 0x8b, (byte) 0x55, (byte) 0x10,
            // 90 * 48 (NOP 填充，与 Python 脚本一致: 74字节总计)
            (byte) 0x90, (byte) 0x90, (byte) 0x90, (byte) 0x90, (byte) 0x90, (byte) 0x90,
            (byte) 0x90, (byte) 0x90, (byte) 0x90, (byte) 0x90, (byte) 0x90, (byte) 0x90,
            (byte) 0x90, (byte) 0x90, (byte) 0x90, (byte) 0x90, (byte) 0x90, (byte) 0x90,
            (byte) 0x90, (byte) 0x90, (byte) 0x90, (byte) 0x90, (byte) 0x90, (byte) 0x90,
            (byte) 0x90, (byte) 0x90, (byte) 0x90, (byte) 0x90, (byte) 0x90, (byte) 0x90,
            (byte) 0x90, (byte) 0x90, (byte) 0x90, (byte) 0x90, (byte) 0x90, (byte) 0x90,
            (byte) 0x90, (byte) 0x90, (byte) 0x90, (byte) 0x90, (byte) 0x90, (byte) 0x90,
            (byte) 0x90, (byte) 0x90, (byte) 0x90, (byte) 0x90, (byte) 0x90, (byte) 0x90,
    };

    // 节属性: MEM_READ | INITIALIZED_DATA
    private static final int MEM_READ = 0x40000000;
    private static final int INITIALIZED_DATA = 0x00000040;

    // 产品密钥
    public static final String PRODUCT_KEY = "NAVMIKCHCWNIHS3Q";
    // 产品密钥（显示格式，每4位用 - 连接）
    public static final String PRODUCT_KEY_FORMATTED = "NAVM-IKCH-CWNI-HS3Q";

    private final Consumer<String> logger;
    private RSAHelper rsaHelper;
    private final ObjectMapper objectMapper = new ObjectMapper();

    public PatcherService(Consumer<String> logger) {
        this.logger = logger;
    }

    public PatcherService() {
        this(System.out::println);
    }

    private void log(String msg) {
        if (logger != null) logger.accept(msg);
    }

    // ── Step 1: 备份 ──

    /**
     * 备份 PE 文件
     * @param peFilePath 原始文件路径 (libcc.dll)
     * @return 备份文件路径
     */
    public String backupFile(String peFilePath) throws IOException {
        Path src = Path.of(peFilePath);
        if (!Files.exists(src)) {
            throw new FileNotFoundException("PE 文件不存在: " + peFilePath);
        }
        Path backup = Path.of(peFilePath + ".bak");
        if (Files.exists(backup)) {
            log("备份文件已存在，跳过备份: " + backup);
        } else {
            Files.copy(src, backup, StandardCopyOption.COPY_ATTRIBUTES);
            log("已备份: " + peFilePath + " → " + backup);
        }
        return backup.toString();
    }

    // ── Step 2: 生成密钥对 ──

    public void generateKeyPair() throws Exception {
        log("正在生成 2048 位 RSA 密钥对...");
        rsaHelper = RSAHelper.generate();
        log("密钥对生成完成。");
        log("公钥:\n" + rsaHelper.getPublicKeyPEM());
        log("私钥:\n" + rsaHelper.getPrivateKeyPEM());
    }

    public RSAHelper getRsaHelper() {
        return rsaHelper;
    }

    // ── Step 3: 应用补丁 ──

    /**
     * 应用补丁到 PE 文件
     * @param backupPath  备份文件路径
     * @param outputPath  输出文件路径 (补丁后的 libcc.dll)
     * @return PatchResult 包含偏移信息
     */
    public PatchResult applyPatch(String backupPath, String outputPath) throws IOException {
        log("正在解析 PE 文件: " + backupPath);
        PEFile pe = PEFile.parse(backupPath);

        // 1. 查找原始字节码
        int patchFileOffset = pe.findBytes(ORIGINAL_BYTECODE);
        if (patchFileOffset == -1) {
            throw new RuntimeException("在二进制文件中未找到原始字节码，可能版本不匹配。");
        }
        log("找到待修补位置: 0x" + Integer.toHexString(patchFileOffset));

        // 2. 添加 .pkey 节
        String pubKeyBase64 = rsaHelper.getPublicKeyBase64();
        byte[] payload = new byte[pubKeyBase64.length() + 1]; // +1 for null terminator
        System.arraycopy(pubKeyBase64.getBytes(), 0, payload, 0, pubKeyBase64.length());
        // payload[payload.length - 1] = 0 (null terminator, already zero)

        log("添加 .pkey 节 (公钥数据 " + payload.length + " 字节)...");
        PEFile.PESection pkeySection = pe.addSection(".pkey", payload, MEM_READ | INITIALIZED_DATA);

        // 3. 计算 RIP 相对偏移
        PEFile.PESection textSection = pe.getSection(".text");
        if (textSection == null) {
            throw new RuntimeException("未找到 .text 节");
        }

        int textFileOffset = textSection.pointerToRawData;
        int textVA = textSection.virtualAddress;
        int pkeyVA = pkeySection.virtualAddress;

        // offset_from_rip = pkey_VA - (text_VA + patch_file_offset - text_file_offset + 7)
        // +7 = lea rcx, [rip+disp32] 指令长度 (3字节opcode + 4字节displacement)
        int offsetFromRip = pkeyVA - (textVA + patchFileOffset - textFileOffset + 7);

        log(".text FOA: 0x" + Integer.toHexString(textFileOffset));
        log(".text VA:  0x" + Integer.toHexString(textVA));
        log(".pkey VA:  0x" + Integer.toHexString(pkeyVA));
        log("Patch 指令: lea rcx, [rip + 0x" + Integer.toHexString(offsetFromRip) + "]");

        // 4. 填充补丁字节码中的位移
        byte[] patchBytes = PATCH_BYTECODE.clone();
        // 找到 00 00 00 00 占位符的位置并替换
        int placeholderIdx = ByteUtils.indexOf(patchBytes, new byte[]{0, 0, 0, 0});
        if (placeholderIdx == -1) {
            throw new RuntimeException("补丁字节码中未找到位移占位符");
        }
        ByteUtils.writeIntLE(patchBytes, placeholderIdx, offsetFromRip);

        // 5. 在内存中应用补丁（先打补丁，再写文件）
        pe.patchAt(patchFileOffset, patchBytes);
        log("补丁已应用，写入位置: 0x" + Integer.toHexString(patchFileOffset));

        // 6. 写入 PE 文件 (含新节 + 补丁)
        pe.write(outputPath);
        log("PE 文件已写入: " + outputPath);

        PatchResult result = new PatchResult();
        result.patchFileOffset = patchFileOffset;
        result.offsetFromRip = offsetFromRip;
        return result;
    }

    // ── Step 4: 离线激活 ──

    /**
     * 解密请求码
     */
    public String decryptRequest(String requestCode) throws Exception {
        log("正在解密请求码...");
        String plain = rsaHelper.decryptRequest(requestCode);
        log("解密结果: " + plain);
        return plain;
    }

    /**
     * 生成激活码
     * @param requestCode 请求码 (Base64)
     * @param username    用户名
     * @param organization 组织名
     * @return 激活码 (Base64)
     */
    @SuppressWarnings("unchecked")
    public String generateActivationCode(String requestCode, String username, String organization) throws Exception {
        // 1. 解密请求码
        String plain = decryptRequest(requestCode);

        // 2. 解析 JSON 并添加字段
        Map<String, Object> data = objectMapper.readValue(plain, Map.class);
        data.put("N", username);
        data.put("O", organization);
        data.put("T", Instant.now().getEpochSecond());

        // 3. 序列化 JSON (不转义非 ASCII 字符)
        String msg = objectMapper.writeValueAsString(data);
        log("激活消息: " + msg);

        // 4. 私钥加密
        String regCode = rsaHelper.privateEncrypt(msg.getBytes(StandardCharsets.UTF_8));
        log("激活码已生成。");

        return regCode;
    }

    /**
     * 补丁结果
     */
    public static class PatchResult {
        public int patchFileOffset;
        public int offsetFromRip;
    }
}
