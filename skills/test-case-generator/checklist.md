# 生成后自检清单 (Checklist)

---

## API 测试自检

- [ ] **AC 全覆盖**：每个 AC-ID 至少有 1 条正向测试？
- [ ] **EC 全覆盖**：每个 EC-ID 至少有 1 条异常测试？
- [ ] **安全测试**：至少包含 401（未授权）和 403（权限不足）测试？
- [ ] **参数校验**：每个 @NotBlank/@NotNull/@Size 字段有对应的测试？
- [ ] **Given 数据具体**：前置条件包含具体的字段值，不写"数据存在"？
- [ ] **Then 断言可验证**：包含具体的 code、message、data 字段值？
- [ ] **覆盖率矩阵**：全部 AC 和 EC 在矩阵中有对应测试用例 ID？

## E2E 测试自检

- [ ] **核心流程覆盖**：每个 US-ID 至少有 1 条 E2E 场景？
- [ ] **UI 状态流转**：Loading→Success、Loading→Empty、Loading→Error 都有测试？
- [ ] **重试机制**：Error→Loading→Success（点击重试后恢复）有测试？
- [ ] **操作步骤具体**：每步操作有明确的动作和预期结果？

## Mock 数据自检

- [ ] **与 Given 一致**：fixtures 中的数据值与 AC 中 Given 的值对应？
- [ ] **响应结构完整**：Mock 数据包含 code、message、data 三个字段？
- [ ] **错误响应覆盖**：包含 400/401/403/404/409 等错误 Mock？

## 通用自检

- [ ] **术语一致**：命名与 glossary.context.md 一致？
- [ ] **测试 ID 编号规范**：API 测试 TC-API-0xx、E2E 测试 TC-E2E-0xx？
- [ ] **关联 Spec 正确**：测试文档标注了关联的 spec 文件路径？
- [ ] **日期使用当前日期**：文档中的日期使用当前系统日期？