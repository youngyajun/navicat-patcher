//! 核心补丁服务：备份文件、生成密钥、应用补丁、离线激活。
//!
//! 移植自 Electron 版 patcher-service.ts（Java 版 PatcherService.java），
//! 对应 Python 脚本的完整流程。
//!
//! 日志通过注入的回调（Arc<dyn Fn(&str)>）推送到任意前端——
//! Electron 版是 webContents.send，Tauri 版是全局事件，egui 版是 mpsc 通道。

use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use super::byte_utils::{find_bytes, write_i32_le};
use super::pe_file::PeFile;
use super::rsa::RsaKeyPair;
use super::Result;

// ── 字节码常量（与 Java 版 / Python 脚本 / Node 版完全一致）──

/// 原始字节码
pub const ORIGINAL_BYTECODE: &[u8] = &[
    // 48 8b d0 48 8b cf ff d3 48 89 46 20 48 8b 55 10
    0x48, 0x8b, 0xd0, 0x48, 0x8b, 0xcf, 0xff, 0xd3,
    0x48, 0x89, 0x46, 0x20, 0x48, 0x8b, 0x55, 0x10,
    // 48 83 fa 0f 76 34 48 ff c2 48 8b 4d f8 48 8b c1
    0x48, 0x83, 0xfa, 0x0f, 0x76, 0x34, 0x48, 0xff, 0xc2,
    0x48, 0x8b, 0x4d, 0xf8, 0x48, 0x8b, 0xc1,
    // 48 81 fa 00 10 00 00 72 1c 48 83 c2 27 48 8b 49
    0x48, 0x81, 0xfa, 0x00, 0x10, 0x00, 0x00, 0x72, 0x1c,
    0x48, 0x83, 0xc2, 0x27, 0x48, 0x8b, 0x49,
    // f8 48 2b c1 48 83 c0 f8 48 83
    0xf8, 0x48, 0x2b, 0xc1, 0x48, 0x83, 0xc0, 0xf8, 0x48, 0x83,
];

/// 补丁字节码（74 字节，中间 4 字节为 RIP 位移占位符）
pub const PATCH_BYTECODE: &[u8] = &[
    // 48 8d 0d 00 00 00 00  → lea rcx, [rip + disp32]
    0x48, 0x8d, 0x0d, 0x00, 0x00, 0x00, 0x00,
    // 48 89 08             → mov [rax], rcx
    0x48, 0x89, 0x08,
    // 48 89 c2             → mov rdx, rax
    0x48, 0x89, 0xc2,
    // 48 89 f9             → mov rcx, rdi
    0x48, 0x89, 0xf9,
    // ff d3                → call rbx
    0xff, 0xd3,
    // 48 89 46 20          → mov [rsi+20h], rax
    0x48, 0x89, 0x46, 0x20,
    // 48 8b 55 10          → mov rdx, [rbp+10h]
    0x48, 0x8b, 0x55, 0x10,
    // 90 * 48 (NOP 填充，与 Python 脚本一致: 74 字节总计)
    0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90,
    0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90,
    0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90,
    0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90,
    0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90,
    0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90,
];

/// 节属性: MEM_READ | INITIALIZED_DATA
const MEM_READ: u32 = 0x4000_0000;
const INITIALIZED_DATA: u32 = 0x0000_0040;

/// Step 2 结果：密钥对 PEM（仅用于界面展示）
pub struct KeyPairInfo {
    pub public_key_pem: String,
    pub private_key_pem: String,
}

/// Step 3 结果：补丁偏移信息
pub struct PatchResultInfo {
    pub patch_file_offset: i64,
    pub offset_from_rip: i64,
}

/// 核心补丁服务（Arc 共享，跨线程调用；耗时操作放后台线程不阻塞 UI）
pub struct PatcherService {
    /// 日志回调（对应 Java Consumer<String> logger /
    /// Electron webContents.send / Tauri app.emit）
    logger: Arc<dyn Fn(&str) + Send + Sync>,
    /// 当前密钥对（未生成时为 None），对应 TS 版的 this.rsa
    keys: Mutex<Option<RsaKeyPair>>,
}

impl PatcherService {
    pub fn new(logger: Arc<dyn Fn(&str) + Send + Sync>) -> PatcherService {
        PatcherService { logger, keys: Mutex::new(None) }
    }

    fn log(&self, msg: &str) {
        (self.logger)(msg);
    }

    /// 在持有密钥对的前提下执行闭包（等价 TS 版的 this.rsa 判空）
    fn with_keys<R>(&self, f: impl FnOnce(&RsaKeyPair) -> Result<R>) -> Result<R> {
        let guard = self.keys.lock().unwrap();
        let keys = guard
            .as_ref()
            .ok_or_else(|| "尚未生成密钥对，请先执行 Step 2".to_string())?;
        f(keys)
    }

    // ── Step 1: 备份 ──

    /// 备份 PE 文件，返回备份文件路径
    pub fn backup_file(&self, pe_file_path: &str) -> Result<String> {
        let path = Path::new(pe_file_path);
        if !path.exists() {
            return Err(format!("PE 文件不存在: {pe_file_path}").into());
        }
        let backup = format!("{pe_file_path}.bak");
        if Path::new(&backup).exists() {
            self.log(&format!("备份文件已存在，跳过备份: {backup}"));
        } else {
            fs::copy(path, &backup)?;
            self.log(&format!("已备份: {pe_file_path} → {backup}"));
        }
        Ok(backup)
    }

    // ── Step 2: 生成密钥对 ──

    pub fn generate_key_pair(&self) -> Result<KeyPairInfo> {
        self.log("正在生成 2048 位 RSA 密钥对...");
        let pair = RsaKeyPair::generate()?;
        let info = KeyPairInfo {
            public_key_pem: pair.public_key_pem()?,
            private_key_pem: pair.private_key_pem()?,
        };
        self.log("密钥对生成完成。");
        self.log(&format!("公钥:\n{}", info.public_key_pem));
        self.log(&format!("私钥:\n{}", info.private_key_pem));
        *self.keys.lock().unwrap() = Some(pair);
        Ok(info)
    }

    // ── Step 3: 应用补丁 ──

    /// 应用补丁到 PE 文件
    ///
    /// * `backup_path` - 备份文件路径
    /// * `output_path` - 输出文件路径 (补丁后的 libcc.dll)
    pub fn apply_patch(&self, backup_path: &str, output_path: &str) -> Result<PatchResultInfo> {
        self.with_keys(|keys| {
            self.log(&format!("正在解析 PE 文件: {backup_path}"));
            let mut pe = PeFile::parse(Path::new(backup_path))?;

            // 1. 查找原始字节码
            let patch_file_offset = pe.find_bytes(ORIGINAL_BYTECODE);
            if patch_file_offset == -1 {
                return Err("在二进制文件中未找到原始字节码，可能版本不匹配。".into());
            }
            self.log(&format!("找到待修补位置: 0x{:x}", patch_file_offset));

            // 2. 添加 .pkey 节
            //    +1 为 null 终止符（公钥 Base64 为纯 ASCII，直接转字节）
            let public_key_b64 = keys.public_key_base64()?;
            let mut payload = public_key_b64.into_bytes();
            payload.push(0); // null 终止符

            self.log(&format!("添加 .pkey 节 (公钥数据 {} 字节)...", payload.len()));
            let pkey_section = pe.add_section(".pkey", &payload, MEM_READ | INITIALIZED_DATA)?;

            // 3. 计算 RIP 相对偏移
            let text_section = pe
                .get_section(".text")
                .ok_or::<super::Error>("未找到 .text 节".into())?;

            // 先拷出所需字段，结束不可变借用（后面 patch_at 需要 &mut）
            let text_file_offset = text_section.pointer_to_raw_data;
            let text_va = text_section.virtual_address;
            let pkey_va = pkey_section.virtual_address;

            // offset_from_rip = pkey_VA - (text_VA + patch_file_offset - text_file_offset + 7)
            // +7 = lea rcx, [rip+disp32] 指令长度 (3 字节 opcode + 4 字节 displacement)
            let offset_from_rip =
                pkey_va as i64 - (text_va as i64 + patch_file_offset - text_file_offset as i64 + 7);

            self.log(&format!(".text FOA: 0x{:x}", text_file_offset));
            self.log(&format!(".text VA:  0x{:x}", text_va));
            self.log(&format!(".pkey VA:  0x{:x}", pkey_va));
            self.log(&format!("Patch 指令: lea rcx, [rip + 0x{:x}]", offset_from_rip));

            // 4. 填充补丁字节码中的位移
            let mut patch_bytes = PATCH_BYTECODE.to_vec();
            // 找到 00 00 00 00 占位符的位置并替换
            let placeholder_idx = find_bytes(&patch_bytes, &[0, 0, 0, 0]);
            if placeholder_idx == -1 {
                return Err("补丁字节码中未找到位移占位符".into());
            }
            write_i32_le(&mut patch_bytes, placeholder_idx as usize, offset_from_rip as i32); // 支持负位移

            // 5. 在内存中应用补丁（先打补丁，再写文件）
            pe.patch_at(patch_file_offset as usize, &patch_bytes);
            self.log(&format!("补丁已应用，写入位置: 0x{:x}", patch_file_offset));

            // 6. 写入 PE 文件 (含新节 + 补丁)
            pe.write(Path::new(output_path))?;
            self.log(&format!("PE 文件已写入: {output_path}"));

            Ok(PatchResultInfo {
                patch_file_offset,
                offset_from_rip,
            })
        })
    }

    // ── Step 4: 离线激活 ──

    /// 解密请求码
    pub fn decrypt_request(&self, request_code: &str) -> Result<String> {
        self.log("正在解密请求码...");
        let plain = self.with_keys(|keys| keys.decrypt_request(request_code))?;
        self.log(&format!("解密结果: {plain}"));
        Ok(plain)
    }

    /// 生成激活码
    ///
    /// * `request_code` - 请求码 (Base64)
    /// * `username`     - 用户名
    /// * `organization` - 组织名
    ///
    /// 返回激活码 (Base64)
    pub fn generate_activation_code(
        &self,
        request_code: &str,
        username: &str,
        organization: &str,
    ) -> Result<String> {
        // 1. 解密请求码
        let plain = self.decrypt_request(request_code)?;

        // 2. 解析 JSON 并添加字段
        //    serde_json 的 preserve_order feature 使 Map 保持插入顺序，
        //    等价 Jackson 的 LinkedHashMap / JS 的 JSON.parse（字符串键保持插入顺序）。
        //    注意：若请求码 JSON 中存在超过 2^53 的整数，
        //    serde_json 用 i64/u64 保存（比 JS double 更精确），
        //    实际请求码数据不会触发差异。
        let mut data: serde_json::Map<String, Value> = serde_json::from_str(&plain)?;
        data.insert("N".into(), Value::String(username.to_string()));
        data.insert("O".into(), Value::String(organization.to_string()));
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        data.insert("T".into(), Value::Number(now.into()));

        // 3. 序列化 JSON（紧凑、不转义非 ASCII，等价 Jackson 默认行为 / JSON.stringify）
        let msg = serde_json::to_string(&data)?;
        self.log(&format!("激活消息: {msg}"));

        // 4. 私钥加密（★ 见 rsa.rs 文件头部的移植说明）
        let reg_code = self.with_keys(|keys| keys.private_encrypt(msg.as_bytes()))?;
        self.log("激活码已生成。");

        Ok(reg_code)
    }
}
