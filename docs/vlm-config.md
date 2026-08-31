# 视觉模型配置(vlm_config.json)

丛雨「看着你玩游戏」的视觉分析(MediaProjection 截屏 → 云端 VLM → 语音/气泡念出)。

## 配置字段(OpenAI 兼容端点)

```json
{
  "base_url": "https://integrate.api.nvidia.com/v1",
  "api_key": "YOUR_KEY",
  "model": "meta/llama-3.2-11b-vision-instruct"
}
```

- `base_url` / `api_key` / `model` — 任意 OpenAI 兼容视觉端点
- 备选: 智谱 GLM-4V-Flash 免费(`https://open.bigmodel.cn/api/paas/v4`,
  `glm-4v-flash`), 通义 Qwen-VL, 自建 vLLM 等

## 加载优先级(3 级回退)

1. `{filesDir}/vlm_config.json` — 运行时注入(调试: `adb run-as rust.cute_pet`)
2. `pet/assets/vlm_config.json` — **构建期内嵌**(发布 APK 开箱即用)
3. 环境变量 `PET_VLM_BASE_URL` / `PET_VLM_API_KEY` / `PET_VLM_MODEL`(桌面)

> ⚠️ `pet/assets/vlm_config.json` 已被 .gitignore 忽略(防 key 入库)。
> 发布含 key 的 APK 前请确认可接受(APK 内 key 可被提取); 生产环境建议
> 走服务端代理而非内嵌个人 key。

## 触发方式

- **快捷按钮**「🔍 看看我在干嘛」— 手动截屏分析
- **自动**: 配置好 key 后启动即自动申请一次授权, 之后每 ~90s 分析一帧
  (Android 16 锁屏会自动停止录屏, 需重新授权)
