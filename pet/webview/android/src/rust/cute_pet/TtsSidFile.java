package rust.cute_pet;

/**
 * 音色 sid 文件的解析(纯 Java, 无 Android 依赖)。
 *
 * 抽成独立单元的理由: 输入是**人手写或文件管理器写出的文本文件**, 各种脏格式都可能出现,
 * 而它一失败就是"音色没变"这种不报错的静默失效。无 Android 依赖 ⇒ 可以直接在 JVM 上
 * `javac` 编译并跑断言验证(见 tools/TtsSidFileTest 说明), 不必等真机。
 */
final class TtsSidFile {

    private TtsSidFile() {
    }

    /**
     * 宽容解析: 允许 UTF-8 BOM、前后空白、空行、`#` 整行或行尾注释、CRLF。
     *
     * @return 解析出的 sid; 内容非法/为空/为负则返回 -1(调用方回退到默认值)
     */
    static int parse(String raw) {
        if (raw == null) {
            return -1;
        }
        for (String line : raw.replace("\uFEFF", "").split("\\R")) {
            String s = line.trim();
            if (s.isEmpty() || s.startsWith("#")) {
                continue;
            }
            int hash = s.indexOf('#');
            if (hash >= 0) {
                s = s.substring(0, hash).trim();
            }
            if (s.isEmpty()) {
                continue;
            }
            try {
                int v = Integer.parseInt(s);
                return v >= 0 ? v : -1;
            } catch (NumberFormatException e) {
                return -1;
            }
        }
        return -1;
    }
}
