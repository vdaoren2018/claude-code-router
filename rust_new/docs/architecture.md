# Rust 重构架构说明

## 目标

在 `rust_new/` 中建立独立 Rust workspace，以一比一兼容现有 Claude Code Router 的 CLI、配置、HTTP 协议、Preset 机制与流式处理行为。

## crate 分层

- `ccr-shared`：共享常量、Preset 类型系统、Schema 计算、导入导出与市场信息。
- `ccr-config`：JSON5 配置加载、环境变量插值、兼容字段归一化。
- `ccr-protocol`：统一消息结构、Transformer 类型、Tokenizer 协议。
- `ccr-transform`：Transformer 注册表与执行管线骨架。
- `ccr-tokenizer`：Tokenizer trait 与服务骨架。
- `ccr-core`：Provider 注册、路由决策与后续请求分发。
- `ccr-server`：HTTP 服务与流式编排骨架。
- `ccr-cli`：命令行与进程生命周期骨架。
- `ccr-plugin-api`：插件协议预留层。

## 当前落地范围

首批已实现的内容集中在：

- Preset 文件读取、拆分、模板替换、映射应用。
- 敏感字段脱敏与回填。
- 配置文件读取、初始配置合并、环境变量插值。
- LLM/Transformer/Tokenizer 的统一协议结构。
- Transformer 注册中心、use 链解析与执行管线。
- Tokenizer 服务、fallback、配置驱动实例化。
- Core 路由决策（longContext/webSearch/think/background/default）与请求预处理。
- Server HTTP 基础路由（`/api/health`、`/api/config`、`/api/providers`、`/api/route/preview`）。
- `/v1/messages` 非流式与 SSE 流式转发（含响应重写链）以及 `/v1/messages/count_tokens` 统计。

## 下一步

1. 继续补齐 `packages/shared/src/preset/*` 细节差异（尤其安装与导出边界行为）。
2. 在 `ccr-server` 中补齐 SSE 流式编排与错误回退策略。
3. 扩展 `/api/*` 管理面（provider CRUD、插件能力等）。
4. 最终接管全部 CLI 与 Node 运行时入口。
