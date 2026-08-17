# lyco_chat 云端部署指南（CloudStudio 算力）

> 把自研 `lyco_chat` 本地轻量 AI 部署到 CloudStudio 云 GPU（Tesla T4），
> 桌宠通过预览地址调用云端推理，本机不跑重负载。

## 依赖

- 认证凭据：`cloudstudio-session` + `cloudstudio-session-team` cookie（见 `cloudstudio-access.md`）
- spaceKey：`04e7e16c9cac40dda427befd85ead378`（可换成你的工作空间）

## 一键部署步骤

### 1. 认证拿 JPS + JWT

```bash
export CS_COOKIE='cloudstudio-session=<值>; cloudstudio-session-team=gh'
node cs_auth.mjs 04e7e16c9cac40dda427befd85ead378   # 打印 JPS + TOKEN
export CS_JPS='https://<spaceKey>--jps.ap-shanghai2.cloudstudio.club'
export CS_TOKEN='<JWT>'
```

### 2. 打包上传源码

```bash
tar -czf lyco_chat_src.tar.gz --exclude=target --exclude=model --exclude=data/washed --exclude=.git lyco_chat
node cs_upload.mjs lyco_chat_src.tar.gz lyco_chat_src.tar.gz   # 远程路径不带 /workspace 前缀
```

### 3. 云端解压 + 编译

```bash
node cs_exec.mjs "import subprocess,os; os.chdir('/workspace'); subprocess.run(['tar','xzf','lyco_chat_src.tar.gz'])"
node cs_exec.mjs "import subprocess; subprocess.run(['bash','-c','cd /workspace/lyco_chat && cargo build --release > build.log 2>&1'], timeout=600)"
# 轮询 build.log 直到 target/release/demo_chat 出现
```

### 4. 启动 serve（脱离会话常驻）

`start_lyco.sh`（本目录）：
```bash
#!/bin/bash
cd /workspace/lyco_chat
mkdir -p model
setsid nohup ./target/release/demo_chat --config server.toml > serve.log 2>&1 < /dev/null &
echo "LAUNCHED PID $!"
exit 0
```
```bash
node cs_upload.mjs start_lyco.sh start_lyco.sh
node cs_exec.mjs "import subprocess; subprocess.run(['bash','/workspace/start_lyco.sh'],timeout=25)"
```

> 首次启动会自动训练 15000 步（T4 约 10 分钟，`serve.log` 显示 Step 进度），
> 完成后 `colibri-style OpenAI server on http://127.0.0.1:8080`。

### 5. 访问预览地址

- AI 服务：`https://<spaceKey>--8080.ap-shanghai2.cloudstudio.club`
- 验证：`GET /v1/chat/completions`（POST `{"model":"lyco","messages":[...]}`）

### 6. 桌宠接入（本机）

```bash
PET_LLM_BASE_URL=https://<spaceKey>--8080.ap-shanghai2.cloudstudio.club \
PET_LLM_API_KEY=cloudstudio \
PET_LLM_MODEL=lyco \
cargo run --bin cute-pet
```

`pet/src/chat.rs` 的 `respond_llm` 走 `{BASE}/v1/chat/completions`，与 lyco_chat 完全兼容，零代码改动。

## 常见问题

- **后台进程被杀**：Jupyter kernel 关闭会清理子进程。必须用 `setsid nohup ... < /dev/null &` 完全脱离，且启动脚本要 `exit 0` 让 cs_exec 立即返回。
- **JWT 5 分钟过期**：长任务/轮询前先重新 `cs_auth.mjs` 刷新。
- **工作空间停止**：预览返回 500/403，需用户在控制台启动工作空间。
- **TTS（GPT-SoVITS）**：若 7860 预览 500，说明云端 TTS 服务未运行，需单独启动（见 `pet/docs/gpt-sovits.md`）。
