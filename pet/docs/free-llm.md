# 桌宠 AI 对话：免费模型渠道

桌宠内置的「语料兜底」是离线台词检索（答非所问时可能不理想）。想要**真正自由的对话**，配置一个 LLM API Key 即可（OpenAI 兼容格式，1 分钟搞定）。

## 配置方法（三选一）

桌宠读取以下环境变量（`PET_LLM_API_KEY` + `PET_LLM_BASE_URL` + `PET_LLM_MODEL`）：

```bash
# 以 NVIDIA NIM 为例（其他渠道同理替换 URL/Key）
PET_LLM_BASE_URL=https://integrate.api.nvidia.com/v1
PET_LLM_API_KEY=nvapi-xxxx
PET_LLM_MODEL=meta/llama-3.1-8b-instruct
```

Android 打包时在 `cargo quad-apk` 的 `--env` 参数传入；WASM 版暂不支持网络 LLM（走语料兜底）。

## 免费渠道（2026-08 实测有效，均有免费额度）

| 渠道 | 免费额度 | 入口 | 备注 |
|---|---|---|---|
| **NVIDIA NIM** | 免费 API Key（`nvapi-` 开头），多种开源模型（Llama/Qwen 等） | https://build.nvidia.com | 注册即送额度，OpenAI 兼容 `/v1` 端点 |
| **OpenRouter** | 每天免费请求 + 多个免费模型（`:free` 后缀） | https://openrouter.ai | 聚合所有大厂模型，一个 Key 全通 |
| **商汤日日新（SenseChat）** | 新用户免费 tokens | https://platform.sensenova.cn | 国内直连快，OpenAI 兼容 |
| **DeepSeek**（备选，非免费但有低价） | 充值制，极便宜 | https://platform.deepseek.com | 本项目默认演示端点 |

## 提示

- 免费模型的「免费」指注册赠送额度，用完需充值或换号，但日常聊天足够用很久。
- 配置后重启桌宠，聊天走 LLM（会带上最近 10 轮上下文，前言搭后语）；未配置则自动回退离线语料。
- 密钥只存在本地环境变量/打包配置里，不上传。
