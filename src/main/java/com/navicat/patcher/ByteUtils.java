package com.navicat.patcher;

/**
 * 字节操作工具类：模式搜索、小端读写、对齐计算
 */
public final class ByteUtils {

    private ByteUtils() {}

    /**
     * 在 data 中搜索 pattern 的第一个匹配位置
     */
    public static int indexOf(byte[] data, byte[] pattern) {
        if (pattern.length == 0) return 0;
        if (pattern.length > data.length) return -1;
        outer:
        for (int i = 0; i <= data.length - pattern.length; i++) {
            for (int j = 0; j < pattern.length; j++) {
                if (data[i + j] != pattern[j]) continue outer;
            }
            return i;
        }
        return -1;
    }

    public static int readIntLE(byte[] data, int offset) {
        return (data[offset] & 0xFF)
                | ((data[offset + 1] & 0xFF) << 8)
                | ((data[offset + 2] & 0xFF) << 16)
                | ((data[offset + 3] & 0xFF) << 24);
    }

    public static long readLongLE(byte[] data, int offset) {
        return (data[offset] & 0xFFL)
                | ((data[offset + 1] & 0xFFL) << 8)
                | ((data[offset + 2] & 0xFFL) << 16)
                | ((data[offset + 3] & 0xFFL) << 24)
                | ((data[offset + 4] & 0xFFL) << 32)
                | ((data[offset + 5] & 0xFFL) << 40)
                | ((data[offset + 6] & 0xFFL) << 48)
                | ((data[offset + 7] & 0xFFL) << 56);
    }

    public static short readShortLE(byte[] data, int offset) {
        return (short) ((data[offset] & 0xFF) | ((data[offset + 1] & 0xFF) << 8));
    }

    public static void writeIntLE(byte[] data, int offset, int value) {
        data[offset] = (byte) (value & 0xFF);
        data[offset + 1] = (byte) ((value >> 8) & 0xFF);
        data[offset + 2] = (byte) ((value >> 16) & 0xFF);
        data[offset + 3] = (byte) ((value >> 24) & 0xFF);
    }

    public static void writeShortLE(byte[] data, int offset, short value) {
        data[offset] = (byte) (value & 0xFF);
        data[offset + 1] = (byte) ((value >> 8) & 0xFF);
    }

    public static int alignUp(int value, int alignment) {
        return (value + alignment - 1) & ~(alignment - 1);
    }

    /**
     * 将 byte[] 格式化为十六进制字符串，用于日志输出
     */
    public static String toHex(byte[] data) {
        StringBuilder sb = new StringBuilder();
        for (byte b : data) {
            sb.append(String.format("%02x", b & 0xFF));
        }
        return sb.toString();
    }
}
