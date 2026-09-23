package rust.cute_pet;

/**
 * TtsSidFile 的 JVM 断言(不依赖 JUnit / Android / Gradle)。
 *
 * 壳里绝大多数类是 Android 耦合的(android.jar 在 JVM 上全是 `Stub!`), 但**纯逻辑单元**
 * 可以抽出来直接在 JVM 上验: 本文件只依赖 src 里的 TtsSidFile.java, 由
 * `tools/run_pure_java_tests.sh` 用 javac + java 跑(= CI 里的「纯 Java 单测」步骤)。
 *
 * 新增纯逻辑类时的套路: 保证它不 import android.*, 然后照这里加断言文件, 脚本会自动收集。
 */
public final class TtsSidFileTest {

    private static int failed = 0;

    private static void eq(String label, int want, String input) {
        int got = TtsSidFile.parse(input);
        boolean ok = got == want;
        if (!ok) {
            failed++;
        }
        System.out.println((ok ? "  ✅ " : "  ❌ ") + label + ": want=" + want + " got=" + got);
    }

    public static void main(String[] args) {
        // 正常写法
        eq("纯数字", 27, "27");
        eq("带换行", 3, "3\n");
        eq("CRLF", 8, "8\r\n");
        eq("前后空格", 16, "  16  ");
        eq("边界 173", 173, "173");
        // 文件管理器 / 记事本常见的脏格式(这些正是最容易静默失效的输入)
        eq("UTF-8 BOM", 5, "\uFEFF5");
        eq("行尾注释", 42, "42  # 试听用");
        eq("整行注释后接值", 7, "# 音色\n7\n");
        eq("空行跳过", 9, "\n\n9");
        // 非法输入必须回退(-1), 而不是抛异常把说话整个搞崩
        eq("负数非法", -1, "-3");
        eq("非数字非法", -1, "abc");
        eq("混合非法", -1, "12abc");
        eq("空串非法", -1, "");
        eq("null 非法", -1, null);
        eq("只有注释", -1, "# nothing");

        System.out.println(failed == 0 ? "\n纯 Java 单测全部通过" : "\n失败 " + failed + " 项");
        System.exit(failed == 0 ? 0 : 1);
    }
}
