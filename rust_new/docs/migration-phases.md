# 迁移阶段说明

## Phase 0

- 固化旧版行为样本。
- 为配置、Preset、路由与 SSE 建立 fixtures。

## Phase 1（当前）

- 建立 Rust workspace。
- 实现 `ccr-shared`、`ccr-config`、`ccr-protocol` 的首版。
- 为后续 crate 建立可编译骨架。

## Phase 2（当前已落地基础版）

- 已实现 Transformer 注册与执行管线。
- 已实现 Tokenizer 服务与基础 fallback。
- 已补充内置 transformer 与动态 tokenizer 创建能力。

## Phase 3（当前已落地基础版）

- 实现 Core 路由决策。
- 接入 Provider 注册、请求转换与 token 统计。

## Phase 4（当前已落地基础版）

- 接入 HTTP 服务。
- 已支持基础 `/api/*` 与 `/v1/messages`（非流式+SSE 流式）、`/v1/messages/count_tokens`。
- 后续补齐流式能力与更多管理接口。

## Phase 5

- 接入 CLI 生命周期。
- 替换现有 Node CLI 入口。
