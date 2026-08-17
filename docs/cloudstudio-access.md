# CloudStudio 工作空间访问指南(供 AI Agent 复用)

> 目标: 让任意 AI Agent 无需浏览器、纯 API 拿到 CloudStudio 云 GPU 工作空间的访问权,
> 并通过 Jupyter 执行 Python(检查环境 / 跑训练 / 启动服务 / 传文件)。
> 本文来自实战(2026-08-14), 完整记录了认证原理与可复现步骤。

---

## 0. 你需要什么

1. **用户的 CloudStudio 会话 cookie**(浏览器 devtools → Application → Cookies → cloudstudio.net):
   - `cloudstudio-session`(必填, 形如 uuid.uuid.uuid)
   - `cloudstudio-session-team`(团队, 如 `gh` = GitHub 团队)
2. **工作空间的 spaceKey**: 一个 32 位 hex 字符串, 可从工作空间预览域名提取
   (如 `https://04e7e16c9cac40dda427befd85ead378--7860.ap-shanghai2.cloudstudio.club` 里的 `04e7e16c...`)。

> ⚠️ 数字 ID(控制台 URL 里的 `cloudstudio.net/a/37112682784653312`)**不是** spaceKey,
> workspace API 用的是预览域名里的那个 hex hash。

---

## 1. 认证原理(两个关键机制)

### 1.1 CSRF token = cookie 的 djb2 哈希

CloudStudio 控制台 API 要求请求头 `X-XSRF-TOKEN`, 其值不是随机数,
而是从会话 cookie 算出的哈希(JS 源码反推):

```js
// 原版 (cloudstudio 前端 bundle)
function Vq() {
  const e = document.cookie.match(/(^|[^;]+)\s*\bskey\b\s*=\s*([^;]+)/)?.[2]
          || document.cookie.match(/(^|[^;]+)\s*\bcloudstudio-session\b\s*=\s*([^;]+)/)?.[2];
  if (!e) return '';
  let t = 5381;
  for (const ch of e) t += (t << 5) + ch.charCodeAt(0);
  return t & 2147483647;
}
```

Python 等价实现(注意 JS 位运算的 32 位有符号截断):

```python
def to_int32(x):
    x = int(x) & 0xFFFFFFFF
    return x - 0x100000000 if x >= 0x80000000 else x

def csrf_token(session_cookie_value: str) -> int:
    t = 5381
    for ch in session_cookie_value:
        t32 = to_int32(t)
        t = t + to_int32(t32 << 5) + ord(ch)
    return t & 0x7FFFFFFF
```

### 1.2 workspace 访问 JWT(5 分钟有效)

控制台 API 认证通过后, 再向 `GET /api/workspace/{spaceKey}/sessions` 换取**工作空间级 JWT**,
用于访问 jps / pty / api / agent 等云端服务(cookie 在这些子域无效, 必须用 JWT Bearer)。

---

## 2. 完整流程(纯 API, 无浏览器)

### 2.1 带 cookie 调控制台 API(验证 + 拿数据)

所有控制台请求带:

```http
Cookie: cloudstudio-session=<值>; cloudstudio-session-team=gh
X-XSRF-TOKEN: <csrf_token(session值)>
User-Agent: Mozilla/5.0
```

### 2.2 查工作空间详情(拿连接地址)

```http
GET https://cloudstudio.net/api/workspace/v2/{spaceKey}
```

返回 `data.connections`:

```json
{
  "spaceKey": "04e7e16c...",
  "connections": {
    "webIDE":       "https://{spaceKey}.ap-shanghai2.cloudstudio.club",
    "pty":          "https://{spaceKey}--pty.ap-shanghai2.cloudstudio.club",
    "preview":      "https://{spaceKey}--{port}.ap-shanghai2.cloudstudio.club",
    "api":          "https://{spaceKey}--api.ap-shanghai2.cloudstudio.club",
    "jupyterServer": "https://{spaceKey}--jps.ap-shanghai2.cloudstudio.club",
    "agent":        "https://{spaceKey}--agent.ap-shanghai2.cloudstudio.club"
  },
  "status": { "status": "Running", "expireAt": 1786742317 }
}
```

- `status.status` 不是 Running 时, 需先请用户在工作空间控制台启动
- `jupyterServer` 就是 **JPS**(Jupyter Server 地址)

### 2.3 换 workspace JWT

```http
GET https://cloudstudio.net/api/workspace/{spaceKey}/sessions
```

返回 `data.token`(JWT, 约 5 分钟有效, 过期重新请求):

```json
{ "code": 0, "data": { "token": "eyJhbGciOiJIUzI1NiJ9..." } }
```

### 2.4 用 JWT 访问 Jupyter(执行代码)

```http
GET  https://{spaceKey}--jps.ap-shanghai2.cloudstudio.club/api
Authorization: Bearer <JWT>
```

通过 Jupyter 执行 Python: 创建 kernel → WebSocket channels → execute_request,
可直接复用本目录 `cs_exec.mjs`(`CS_TOKEN`=JWT, `CS_JPS`=jps 地址):

```bash
export CS_TOKEN=<JWT>
export CS_JPS=https://{spaceKey}--jps.ap-shanghai2.cloudstudio.club
node cs_exec.mjs "print('hello from cloud')"
node cs_exec.mjs --file 本地脚本.py     # 跑脚本(注意路径在本地, 内容会发到云端)
```

### 2.5 传文件(Jupyter REST)

```bash
# 上传: PUT  {JPS}/api/contents/<远端路径>  (base64, 见 cs_upload.mjs)
# 下载: GET  {JPS}/api/contents/<远端路径>?content=1  (见 cs_download.mjs)
node cs_upload.mjs  本地文件  /workspace/GPT_SoVITS/xxx.py
node cs_download.mjs /workspace/GPT_SoVITS/xxx.txt 本地文件
```

---

## 3. 一键脚本 cs_auth.mjs

把 2.1-2.3 封装成一条命令:

```bash
export CS_COOKIE='cloudstudio-session=<值>; cloudstudio-session-team=gh'
node cs_auth.mjs <spaceKey>          # 打印 JPS + JWT + 可复制的 export 命令

# 或自动取当前用户的第一个工作空间(需 status/list 有数据)
node cs_auth.mjs
```

输出示例:

```
JPS  https://04e7e16c...--jps.ap-shanghai2.cloudstudio.club
TOKEN eyJhbGciOiJIUzI1NiJ9...
---
export CS_JPS='https://04e7e16c...--jps.ap-shanghai2.cloudstudio.club'
export CS_TOKEN='eyJhbGciOiJIUzI1NiJ9...'
```

---

## 4. 踩坑清单

1. **spaceKey ≠ 数字 ID**: workspace v2 API 用 hex hash, 不是控制台 URL 的数字。
   数字 ID 查询返回 `space_not_found`。
2. **JWT 5 分钟过期**: 长任务前先刷新; kernel 一旦建立, WebSocket 连接不受 JWT 过期影响。
3. **CSRF 位运算陷阱**: Python 模拟 JS `t << 5` 必须做 32 位有符号截断, 否则 token 不对。
4. **子域认证**: `--jps` `--pty` `--api` `--agent` 一律用 `Authorization: Bearer <JWT>`,
   cookie 在这些子域无效(返回 401)。
5. **工作空间停止**: 预览 URL 返回 500/403; 需用户重启, 再重新走流程。
6. **API 路径前缀**: 控制台 API 实际挂在 `/api` 下: `/api/workspace/v2/{key}`, `/api/workspace/{key}/sessions`。
7. **团队 cookie**: `cloudstudio-session-team=gh` 表示 GitHub 团队, 缺失或错误会导致列表为空。

---

## 5. 安全提示

- cookie 与 JWT 都是敏感凭据: 仅在会话内使用, 不写入仓库/日志
- 给用户/其他 agent 时走私有通道, 用后即弃(cookie 可让用户重新登录使旧会话失效)
- 本文只描述获取自身工作空间访问权的方法, 请遵守 CloudStudio 服务条款
