# 生成后自检清单 (Checklist)

> 用途：api-contract-generator 每次生成文档后，必须逐项自检

---

## API_SPEC.md 自检

- [ ] **接口路径遵循 RESTful**：使用 kebab-case，操作映射 GET/POST/PUT/DELETE 正确？
- [ ] **已有接口标注**：已在项目中实现的接口标注了"已实现 ✅"？
- [ ] **REQ-ID 关联**：每个接口关联了对应的 REQ-ID？
- [ ] **请求字段校验具体**：每个 DTO 字段标注了具体的 Bean Validation 注解和参数值？
- [ ] **错误码与 EC-ID 对应**：每个错误响应标注了关联的 EC-ID？
- [ ] **响应格式统一**：使用 ApiResponseReturn<T> 包装？
- [ ] **分页格式正确**：page 从 0 开始，响应为 Page<T> 格式？
- [ ] **SecretKey 脱敏**：列表接口中 secretKey 显示为 `******`，仅创建时返回完整值？
- [ ] **子资源路径正确**：如词条使用 `/api/lexicons/{lexiconId}/entries`？

## DB_SCHEMA.md 自检

- [ ] **已有表标注**：已存在的表标注了"已有 ✅"？
- [ ] **表名 snake_case 复数**：如 `applications`、`invocation_logs`？
- [ ] **字段名 snake_case**：如 `secret_key`、`invoke_type`？
- [ ] **外键命名正确**：如 `lexicon_id`、`app_id`？
- [ ] **时间字段命名**：`created_at`、`updated_at`？
- [ ] **状态字段类型**：Integer 编码（-1/0/1），不使用 PostgreSQL ENUM？
- [ ] **PostgreSQL 语法**：自增用 `BIGSERIAL`，JSON 用 `JSONB`？
- [ ] **索引命名规范**：`idx_{表}_{字段}` / `uk_{表}_{字段}`？
- [ ] **ER 图反映真实关系**：与架构文档中的关系一致？

## 通用自检

- [ ] **术语一致**：命名与 glossary.context.md 一致？（"词条"用 entry，"调用日志"用 invocation log）
- [ ] **关联 Spec 正确**：文档头部标注了关联的 spec 文件路径？
- [ ] **日期使用当前日期**：所有文件中的日期使用当前系统日期？
- [ ] **占位符已全部填充**：没有残留的 `{占位符}`？