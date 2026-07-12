package com.navicat.patcher;

import javax.crypto.Cipher;
import java.math.BigInteger;
import java.security.*;
import java.security.interfaces.RSAPrivateKey;
import java.security.interfaces.RSAPublicKey;
import java.security.spec.X509EncodedKeySpec;
import java.util.Arrays;
import java.util.Base64;

/**
 * RSA 加密工具：密钥生成、公钥加密解密、私钥加密（签名）。
 * 私钥加密使用手动 PKCS#1 v1.5 BlockType=1 填充 + BigInteger.modPow。
 */
public class RSAHelper {

    private final KeyPair keyPair;

    private RSAHelper(KeyPair keyPair) {
        this.keyPair = keyPair;
    }

    /**
     * 生成 2048 位 RSA 密钥对
     */
    public static RSAHelper generate() throws NoSuchAlgorithmException {
        KeyPairGenerator kpg = KeyPairGenerator.getInstance("RSA");
        kpg.initialize(2048, new SecureRandom());
        return new RSAHelper(kpg.generateKeyPair());
    }

    /**
     * 获取公钥的 Base64 编码（不含 PEM 头尾），用于嵌入 .pkey 节
     */
    public String getPublicKeyBase64() {
        byte[] der = keyPair.getPublic().getEncoded(); // X.509 SubjectPublicKeyInfo
        return Base64.getEncoder().encodeToString(der);
    }

    /**
     * 获取公钥的 PEM 格式（含头尾），用于显示
     */
    public String getPublicKeyPEM() {
        return formatPEM("PUBLIC KEY", keyPair.getPublic().getEncoded());
    }

    /**
     * 获取私钥的 PEM 格式（PKCS#8），用于显示
     */
    public String getPrivateKeyPEM() {
        return formatPEM("PRIVATE KEY", keyPair.getPrivate().getEncoded());
    }

    /**
     * 使用私钥解密请求码（标准 RSA 解密：公钥加密 → 私钥解密）
     */
    public String decryptRequest(String requestCodeB64) throws Exception {
        byte[] encrypted = Base64.getDecoder().decode(requestCodeB64);
        Cipher cipher = Cipher.getInstance("RSA/ECB/PKCS1Padding");
        cipher.init(Cipher.DECRYPT_MODE, keyPair.getPrivate());
        byte[] plain = cipher.doFinal(encrypted);
        return new String(plain);
    }

    /**
     * 使用私钥加密消息（RSA 私钥运算，用于生成激活码）。
     * 手动实现 PKCS#1 v1.5 BlockType=1 填充 + m^d mod n。
     */
    public String privateEncrypt(byte[] message) {
        RSAPrivateKey privKey = (RSAPrivateKey) keyPair.getPrivate();
        BigInteger n = privKey.getModulus();
        BigInteger d = privKey.getPrivateExponent();
        int keySize = n.bitLength() / 8; // 2048/8 = 256

        // 1. PKCS#1 v1.5 填充 (BlockType=1): 0x00 0x01 [0xFF...] 0x00 [message]
        byte[] em = pkcs1V15PrivatePad(message, keySize);

        // 2. RSA 运算: c = m^d mod n
        BigInteger m = new BigInteger(1, em);
        BigInteger c = m.modPow(d, n);

        // 3. 转换为 keySize 字节数组
        byte[] result = c.toByteArray();
        if (result.length == keySize) {
            // 完美匹配
        } else if (result.length == keySize + 1 && result[0] == 0) {
            // 前导零字节，截掉
            result = Arrays.copyOfRange(result, 1, result.length);
        } else if (result.length < keySize) {
            // 左侧补零
            byte[] padded = new byte[keySize];
            System.arraycopy(result, 0, padded, keySize - result.length, result.length);
            result = padded;
        }

        return Base64.getEncoder().encodeToString(result);
    }

    /**
     * PKCS#1 v1.5 私钥加密填充 (BlockType=1)
     * 格式: 0x00 | 0x01 | 0xFF * ps_len | 0x00 | message
     */
    private static byte[] pkcs1V15PrivatePad(byte[] message, int keySize) {
        if (message.length > keySize - 11) {
            throw new IllegalArgumentException("消息太长，超过 RSA 模数限制");
        }
        int psLen = keySize - message.length - 3;
        byte[] em = new byte[keySize];
        em[0] = 0x00;
        em[1] = 0x01;
        Arrays.fill(em, 2, 2 + psLen, (byte) 0xFF);
        // em[2 + psLen] = 0x00 (already zero)
        System.arraycopy(message, 0, em, 3 + psLen, message.length);
        return em;
    }

    /**
     * 格式化为 PEM (Base64 每 64 字符换行)
     */
    private static String formatPEM(String type, byte[] derBytes) {
        String base64 = Base64.getMimeEncoder(64, "\n".getBytes())
                .encodeToString(derBytes);
        return "-----BEGIN " + type + "-----\n" + base64 + "\n-----END " + type + "-----";
    }

    /**
     * 从 PEM 格式公钥恢复 RSAPublicKey（用于测试或验证）
     */
    public static RSAPublicKey parsePublicKey(String pem) throws Exception {
        String base64 = pem
                .replace("-----BEGIN PUBLIC KEY-----", "")
                .replace("-----END PUBLIC KEY-----", "")
                .replaceAll("\\s", "");
        byte[] der = Base64.getDecoder().decode(base64);
        X509EncodedKeySpec spec = new X509EncodedKeySpec(der);
        return (RSAPublicKey) KeyFactory.getInstance("RSA").generatePublic(spec);
    }
}
