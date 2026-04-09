# 兼容矩阵（初版）

## CLI

| 能力 | 旧版入口 | Rust 当前状态 |
| --- | --- | --- |
| 服务启动/停止 | `ccr start/stop/restart/status` | 已建骨架，未接入真实进程控制 |
| Claude Code 代理执行 | `ccr code` | 已建骨架，未接入 |
| 预设管理 | `ccr preset/install` | 共享层已落地部分逻辑 |
| UI 打开 | `ccr ui` | 已建骨架，未接入 |

## HTTP

| 接口 | 旧版行为 | Rust 当前状态 |
| --- | --- | --- |
| `POST /v1/messages` | 核心对话代理 | 已实现非流式+流式基础转发（Phase 4） |
| `POST /v1/messages/count_tokens` | 计算 token | 已实现（Phase 4） |
| `/api/config` | 配置读写 | 已实现基础读取（Phase 4） |
| `/api/presets` | 预设管理 | 共享层已具备基础能力 |
| `/api/providers` | Provider 管理 | 已实现只读列表（Phase 4） |

## 配置

| 能力 | 当前状态 |
| --- | --- |
| JSON5 解析 | 已实现 |
| `$VAR` / `${VAR}` 插值 | 已实现 |
| `Providers/providers` 兼容读取 | 已实现归一化 |
| `Plugins/plugins` 兼容读取 | 已实现归一化 |

## Preset

| 能力 | 当前状态 |
| --- | --- |
| Manifest 读写 | 已实现 |
| Template 变量替换 | 已实现 |
| ConfigMappings | 已实现 |
| 敏感字段脱敏/回填 | 已实现 |
| ZIP 解压安装 | 已实现基础版本 |
| Marketplace 拉取 | 已实现基础版本 |

## Transformer / Tokenizer

| 能力 | 当前状态 |
| --- | --- |
| Transformer 注册与 use 链解析 | 已实现（Phase 2） |
| Transformer pipeline 执行 | 已实现（Phase 2） |
| 内置 transformer（passthrough/maxtoken/sampling） | 已实现（Phase 2） |
| Tokenizer 服务与 fallback | 已实现（Phase 2） |
| 按配置动态创建 tokenizer | 已实现（Phase 2） |
| API tokenizer | 已实现（基础版） |

## 流式处理

| 能力 | 当前状态 |
| --- | --- |
| SSE 解析与重写 | 已实现基础版（Phase 4） |
| Tool Call 代理回注 | 未实现 |
| Agent 链路 | 未实现 |
