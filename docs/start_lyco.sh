#!/bin/bash
# 云端启动 lyco_chat serve (脱离会话, 常驻)
cd /workspace/lyco_chat
# 若模型已存在则跳过训练; 否则 serve 会先训练
if [ ! -f model/base_gpu.json ]; then
  mkdir -p model
fi
# 用 setsid 完全脱离进程组, 输出到 serve.log
setsid nohup ./target/release/demo_chat --config server.toml > serve.log 2>&1 < /dev/null &
echo "LAUNCHED PID $!"
exit 0