//! RSA 加密工具：密钥生成、公钥加密解密、私钥加密（签名）。
//!
//! 移植自 Electron 版 rsa.ts（Java 版 RSAHelper.java）。
//!
//! ★★★ 移植重点：私钥加密（private_encrypt）★★★
//!
//! Java 版的手动实现是：
//!     1. 手动构造 PKCS#1 v1.5 BlockType=1 填充：00 01 [FF × ps] 00 ‖ message
//!     2. BigInteger.modPow(m, d, n)          ← 纯数学私钥运算
//!     3. 结果补齐/截断到模长 256 字节后 Base64
//!
//! Node 版用 privateEncrypt(RSA_NO_PADDING) 完成 m^d mod n；
//! Rust 的 rsa crate 没有暴露 "raw RSA" API（sign 系列强制先哈希 + DigestInfo），
//! 但可以直接访问私有指数 d 与模数 n，用 BigUint::modpow 完成同一运算：
//!     c = m.modpow(d, n)
//! 这与 Java BigInteger.modPow 语义完全一致，字节级等价。
//!
//! ⚠️ 常见误区（与 Node 版注释对应）：
//!   1. rsa crate 的 SigningKey 会先对数据做哈希再包 DigestInfo，输出不等价；
//!   2. 任何标准签名 API 同样强制先哈希——这就是该逻辑必须手写 modpow 的原因。
//!
//! 正确性验证方法：用公钥 (e, n) 对输出做 c^e mod n，
//! 结果应为 00 01 FF…FF 00 ‖ message，与 Java/Node/Python 版完全一致。

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use rsa::pkcs1v15::DecryptingKey;
use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};
use rsa::traits::{Decryptor, PrivateKeyParts, PublicKeyParts};
use rsa::{BigUint, RsaPrivateKey, RsaPublicKey};

use super::Result;

/// RSA 密钥对（对应 TS 版 RSAHelper）
pub struct RsaKeyPair {
    private_key: RsaPrivateKey,
    public_key: RsaPublicKey,
}

impl RsaKeyPair {
    /// 生成 2048 位 RSA 密钥对。
    ///
    /// 对应 Node 的异步 generateKeyPair（libuv 线程池）/
    /// Java 版后台 Task 线程中的 KeyPairGenerator。
    pub fn generate() -> Result<RsaKeyPair> {
        let mut rng = rand::thread_rng();
        let private_key = RsaPrivateKey::new(&mut rng, 2048)?;
        let public_key = RsaPublicKey::from(&private_key);
        Ok(RsaKeyPair { private_key, public_key })
    }

    /// 获取公钥的 Base64 编码（不含 PEM 头尾），用于嵌入 .pkey 节。
    ///
    /// 对应 Java keyPair.getPublic().getEncoded() 与 Node 的
    /// export({ type: 'spki', format: 'der' })：
    /// X.509 SubjectPublicKeyInfo 的 DER 编码 → Base64，字节级一致，
    /// 保证补丁后的 libcc.dll 中公钥格式与 Java/Node/Python 版完全相同。
    pub fn public_key_base64(&self) -> Result<String> {
        let der = self.public_key.to_public_key_der()?;
        Ok(BASE64.encode(der.as_bytes()))
    }

    /// 获取公钥的 PEM 格式（含头尾），用于显示
    pub fn public_key_pem(&self) -> Result<String> {
        let pem = self.public_key.to_public_key_pem(LineEnding::LF)?;
        Ok(pem.trim().to_string())
    }

    /// 获取私钥的 PEM 格式（PKCS#8），用于显示
    pub fn private_key_pem(&self) -> Result<String> {
        let pem = self.private_key.to_pkcs8_pem(LineEnding::LF)?;
        Ok(pem.to_string().trim().to_string())
    }

    /// 使用私钥解密请求码（标准 RSA 解密：公钥加密 → 私钥解密）。
    /// 对应 Java Cipher.getInstance("RSA/ECB/PKCS1Padding") + DECRYPT_MODE。
    pub fn decrypt_request(&self, request_code_b64: &str) -> Result<String> {
        let encrypted = BASE64.decode(request_code_b64.trim())?;
        // DecryptingKey 内置 PKCS#1 v1.5 解填充（clone 仅复制密钥组件，开销可忽略）
        let key = DecryptingKey::new(self.private_key.clone());
        let plain = key.decrypt(&encrypted)?;
        Ok(String::from_utf8(plain)?)
    }

    /// 使用私钥加密消息（RSA 私钥运算，用于生成激活码）。
    /// 对应 Java privateEncrypt：手动 PKCS#1 v1.5 BlockType=1 填充 + m^d mod n。
    /// 实现说明见文件头部 ★★★ 注释。
    pub fn private_encrypt(&self, message: &[u8]) -> Result<String> {
        let key_size: usize = 256; // 2048 / 8

        // 1. PKCS#1 v1.5 填充 (BlockType=1): 0x00 0x01 [0xFF...] 0x00 [message]
        let em = pkcs1_v15_private_pad(message, key_size)?;

        // 2. RSA 运算: c = m^d mod n
        //    BigUint::modpow 即纯私钥运算，等价 Java BigInteger.modPow
        //    与 Node 的 privateEncrypt(RSA_NO_PADDING)。
        let m = BigUint::from_bytes_be(&em);
        let c = m.modpow(self.private_key.d(), self.private_key.n());

        // 3. 转换为 keySize 字节数组后 Base64
        //    BigUint 丢弃前导零，需左补零到模长（对应 Java 版的补零逻辑；
        //    Node 的 NO_PADDING 输出恒为模长，天然不需要）
        let bytes = c.to_bytes_be();
        let mut padded = vec![0u8; key_size - bytes.len()];
        padded.extend_from_slice(&bytes);

        Ok(BASE64.encode(padded))
    }
}

/// PKCS#1 v1.5 私钥加密填充 (BlockType=1)
/// 格式: 0x00 | 0x01 | 0xFF * ps_len | 0x00 | message
fn pkcs1_v15_private_pad(message: &[u8], key_size: usize) -> Result<Vec<u8>> {
    if message.len() > key_size - 11 {
        return Err("消息太长，超过 RSA 模数限制".into());
    }
    let ps_len = key_size - message.len() - 3;
    let mut em = vec![0u8; key_size];
    em[0] = 0x00;
    em[1] = 0x01;
    em[2..2 + ps_len].fill(0xFF);
    // em[2 + ps_len] = 0x00 (vec 已置零)
    em[3 + ps_len..].copy_from_slice(message);
    Ok(em)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 私钥加密 → 公钥运算还原，验证 PKCS#1 v1.5 BlockType=1 填充结构
    #[test]
    fn test_private_encrypt_structure() {
        let pair = RsaKeyPair::generate().unwrap();
        let message = b"{\"K\":\"NAVMIKCHCWNIHS3Q\",\"N\":\"test\",\"O\":\"org\",\"T\":1700000000}";

        let reg_code = pair.private_encrypt(message).unwrap();
        let c_bytes = BASE64.decode(reg_code).unwrap();
        assert_eq!(c_bytes.len(), 256);

        // 公钥运算 c^e mod n 还原明文整数
        let c = BigUint::from_bytes_be(&c_bytes);
        let m = c.modpow(pair.public_key.e(), pair.public_key.n());
        let em = m.to_bytes_be();
        // 补齐到 256
        let mut full = vec![0u8; 256 - em.len()];
        full.extend_from_slice(&em);

        // 验证填充结构: 00 01 FF...FF 00 || message
        assert_eq!(full[0], 0x00);
        assert_eq!(full[1], 0x01);
        let ps_end = 256 - message.len() - 1;
        assert!(full[2..ps_end].iter().all(|&b| b == 0xFF));
        assert_eq!(full[ps_end], 0x00);
        assert_eq!(&full[ps_end + 1..], message);
    }
}
