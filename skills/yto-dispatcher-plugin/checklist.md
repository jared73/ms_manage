# 生成后自检清单 (Checklist)

---

## 配置文件

- [ ] 配置文件名是否符合 `bootstrap-plugin-{功能}.yml` 格式
- [ ] engineName 是否符合 `{业务}-{功能}` 格式
- [ ] 配置文件是否位于正确的 resources 目录下

## 调度策略

- [ ] 每个 dispatcher 是否遵循四阶段结构（参数初始化 → 参数校验 → 业务处理 → 生成返回值）
- [ ] 参数初始化阶段是否包含 `BaseInitParamPlugin`
- [ ] 条件分支节点是否设置了 `selfPredicate: true`
- [ ] 条件分支的 beanName 首个是否为 FilterPlugin
- [ ] 已有策略是否未被覆盖（仅追加）

## 通用自检

- [ ] 每个步骤是否作为独立待办任务（未合并）
- [ ] 全程中文输出
