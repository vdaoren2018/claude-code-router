# Rust 开发规则（rust_new）

## 1. 基本原则

1. 优先保证与现有 TypeScript 行为兼容。
2. 保持实现简单直观，避免过度工程化。
3. 新增逻辑必须有对应测试。

## 2. 代码风格

1. 统一使用 `cargo fmt --all`。
2. 命名遵循 Rust 习惯：
   - 类型：`PascalCase`
   - 函数/变量：`snake_case`
   - 常量：`SCREAMING_SNAKE_CASE`
3. 文件按模块拆分，单文件超过 300~800 行时必须拆分。

## 3. 注释规范

1. 所有注释使用中文。
2. 公共类型、函数、模块必须写文档注释（`///` 或 `//!`）。
3. 注释密度目标不低于 30%，重点说明：
   - 为什么这样设计
   - 关键边界条件
   - 与 TS 对齐点

## 4. 错误处理

1. 禁止 `unwrap()` / `expect()` 出现在生产路径（测试代码可使用）。
2. 统一使用 `Result<T, E>`，错误类型集中管理。
3. 发生报错先定位根因，再修复，禁止盲目重试。

## 5. 测试与质量门槛

每次提交前至少执行：

```bash
cargo fmt --all
cargo test --workspace
```

如涉及行为变化，需补充：
1. 单元测试（规则判定、边界场景）
2. 集成测试（模块串联链路）

## 6. 依赖与架构

1. 禁止重复造轮子，优先复用现有 crate。
2. 新依赖需说明用途，避免引入重型依赖。
3. 维持分层边界：
   - `ccr-protocol` 只放协议
   - `ccr-core` 负责路由与编排
   - `ccr-server` 负责 HTTP/SSE

## 7. 兼容性约束

1. 配置字段兼容 `Router/router`、`Providers/providers`。
2. 路由规则优先对齐 TS：
   - `web_search` 优先于 thinking
   - `longContextThreshold` 与 `longContext`
   - `<CCR-SUBAGENT-MODEL>` 模型覆盖
3. 流式与非流式行为保持可预测、可回归测试。
