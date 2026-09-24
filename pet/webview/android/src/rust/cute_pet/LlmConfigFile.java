package rust.cute_pet;

/**
 * llm_config.json 的解析(纯 Java, 无 Android 依赖 —— 同 TtsSidFile 的抽法)。
 *
 * 壳内 AI 对话的配置文件, 放在素材目录 assets/pet/llm_config.json:
 *   {"base_url":"https://api.deepseek.com","api_key":"sk-...","model":"deepseek-chat"}
 * OverlayService 启动时读取并经 evalJs 注入 window.cutePetLLM(见 pet_bridge.js),
 * 之后 wasm 聊天面板的输入会经 LLM 轮询桥(chat::llm_bridge)打到该端点。
 *
 * 为什么手写解析: 壳零依赖, 而 org.json 在 android.jar 里是 `Stub!`,
 * JVM 上跑不了 ⇒ 纯 Java 单测通道(见 tools/LlmConfigFileTest)必须无 Android 依赖。
 * 解析宽容: UTF-8 BOM、键名/值两侧空白、标准转义(\" \\ \/ \b \f \n \r \t 与 4 位十六进制 Unicode)。
 */
final class LlmConfigFile {

    private LlmConfigFile() {
    }

    /** 解析结果; 字段缺失为 null(调用方按缺省处理)。 */
    static final class Config {
        final String baseUrl;
        final String apiKey;
        final String model;

        Config(String baseUrl, String apiKey, String model) {
            this.baseUrl = baseUrl;
            this.apiKey = apiKey;
            this.model = model;
        }
    }

    /**
     * @return 解析出的配置; JSON 整体非法(null/非对象开头/引号未闭合)返回 null。
     *         base_url 或 api_key 缺失/为空 ⇒ 返回 null(视为未配置, 别注入半套配置)。
     *         model 可缺(调用方回退默认模型)。
     */
    static Config parse(String raw) {
        if (raw == null) {
            return null;
        }
        String s = raw.replace("\uFEFF", "").trim();
        if (!s.startsWith("{")) {
            return null;
        }
        String baseUrl = stringValue(s, "base_url");
        String apiKey = stringValue(s, "api_key");
        String model = stringValue(s, "model");
        if (baseUrl == null || baseUrl.isEmpty() || apiKey == null || apiKey.isEmpty()) {
            return null;
        }
        return new Config(baseUrl, apiKey, model);
    }

    /** 在 JSON 文本里找 "key" : "value" 的 value(支持标准转义)。找不到/未闭合返回 null。 */
    static String stringValue(String json, String key) {
        String needle = "\"" + key + "\"";
        int i = json.indexOf(needle);
        if (i < 0) {
            return null;
        }
        i += needle.length();
        while (i < json.length() && Character.isWhitespace(json.charAt(i))) {
            i++;
        }
        if (i >= json.length() || json.charAt(i) != ':') {
            return null;
        }
        i++;
        while (i < json.length() && Character.isWhitespace(json.charAt(i))) {
            i++;
        }
        if (i >= json.length() || json.charAt(i) != '"') {
            return null;
        }
        i++;
        StringBuilder out = new StringBuilder();
        while (i < json.length()) {
            char c = json.charAt(i);
            if (c == '"') {
                return out.toString();
            }
            if (c == '\\' && i + 1 < json.length()) {
                char n = json.charAt(++i);
                switch (n) {
                    case '"': out.append('"'); break;
                    case '\\': out.append('\\'); break;
                    case '/': out.append('/'); break;
                    case 'n': out.append('\n'); break;
                    case 't': out.append('\t'); break;
                    case 'r': out.append('\r'); break;
                    case 'b': out.append('\b'); break;
                    case 'f': out.append('\f'); break;
                    case 'u':
                        if (i + 4 < json.length()) {
                            try {
                                out.append((char) Integer.parseInt(json.substring(i + 1, i + 5), 16));
                                i += 4;
                            } catch (NumberFormatException e) {
                                out.append('u');
                            }
                        } else {
                            out.append('u');
                        }
                        break;
                    default: out.append(n);
                }
            } else {
                out.append(c);
            }
            i++;
        }
        return null; // 引号未闭合
    }
}
