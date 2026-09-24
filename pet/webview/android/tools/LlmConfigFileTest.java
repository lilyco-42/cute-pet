package rust.cute_pet;

/**
 * LlmConfigFile 的 JVM 断言(不依赖 JUnit / Android / Gradle)。
 *
 * 配置文件是**人手写的 JSON**, 各种脏输入都可能出现, 而解析一失败就是
 * "AI 对话悄悄没生效"这种静默失效 —— 正是纯 Java 单测该守住的地方。
 * 由 `tools/run_pure_java_tests.sh` 用 javac + java 跑(= CI 里的「纯 Java 单测」步骤)。
 * 新增纯逻辑类时的套路: 保证它不 import android.*, 然后照这里加断言文件, 脚本会自动收集。
 */
public final class LlmConfigFileTest {

    private static int failed = 0;

    private static void check(String label, boolean ok) {
        if (!ok) {
            failed++;
        }
        System.out.println((ok ? "  ✅ " : "  ❌ ") + label);
    }

    public static void main(String[] args) {
        // 正常写法(三个字段全齐)
        LlmConfigFile.Config c = LlmConfigFile.parse(
                "{\"base_url\":\"https://api.deepseek.com\",\"api_key\":\"sk-abc\",\"model\":\"deepseek-chat\"}");
        check("正常解析", c != null
                && "https://api.deepseek.com".equals(c.baseUrl)
                && "sk-abc".equals(c.apiKey)
                && "deepseek-chat".equals(c.model));

        // 脏格式(记事本/文件管理器常见)
        c = LlmConfigFile.parse("\uFEFF { \"base_url\" : \"https://x.example\" , \"api_key\" : \"k\" } ");
        check("BOM 与键值两侧空白", c != null
                && "https://x.example".equals(c.baseUrl) && "k".equals(c.apiKey) && c.model == null);
        check("model 缺失可容忍(base_url+key 齐)", c != null && c.model == null);

        // 转义
        c = LlmConfigFile.parse("{\"base_url\":\"https://a\\/b\",\"api_key\":\"k\\\"x\"}");
        check("转义 \\/ 与 \\\"", c != null && "https://a/b".equals(c.baseUrl) && "k\"x".equals(c.apiKey));
        c = LlmConfigFile.parse("{\"base_url\":\"https://x\",\"api_key\":\"k\",\"model\":\"\\u4e39\\u96e8\"}");
        check("\\uXXXX 转义", c != null && "丹雨".equals(c.model));

        // 未配置判定: 缺端点或缺密钥都必须整份拒绝(不注入半套配置)
        check("缺 api_key 视为未配置", LlmConfigFile.parse("{\"base_url\":\"https://x\"}") == null);
        check("缺 base_url 视为未配置", LlmConfigFile.parse("{\"api_key\":\"k\"}") == null);
        check("空 api_key 视为未配置", LlmConfigFile.parse("{\"base_url\":\"https://x\",\"api_key\":\"\"}") == null);

        // 非法输入必须整体回退 null, 而不是抛异常把启动搞崩
        check("非 JSON 文本", LlmConfigFile.parse("随便写的") == null);
        check("null", LlmConfigFile.parse(null) == null);
        check("数组开头", LlmConfigFile.parse("[]") == null);
        check("引号未闭合", LlmConfigFile.parse("{\"base_url\":\"https://x") == null);
        check("键后无冒号", LlmConfigFile.parse("{\"base_url\" https://x}") == null);

        System.out.println(failed == 0 ? "\n纯 Java 单测全部通过" : "\n失败 " + failed + " 项");
        System.exit(failed == 0 ? 0 : 1);
    }
}
