# rust_new

`rust_new/` 是 Claude Code Router 的 Rust 重构工作区。

## 当前状态

- 已创建独立 Rust workspace。
- 已落地第一阶段核心 crate：`ccr-shared`、`ccr-config`、`ccr-protocol`。
- 已创建后续 crate 骨架：`ccr-transform`、`ccr-tokenizer`、`ccr-core`、`ccr-server`、`ccr-cli`、`ccr-plugin-api`。
- 已补充兼容矩阵、架构说明、迁移阶段文档与基础 fixtures。

## 设计原则

1. 行为兼容优先于语法翻译。
2. 与现有 TypeScript 工程并存，不直接替换旧实现。
3. 先落地共享逻辑、配置层与协议层，再向执行层推进。
4. 所有可观察行为以现有 Node 版本为兼容基准。
