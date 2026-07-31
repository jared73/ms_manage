# 生成后自检清单 (Checklist)

---

## 步骤 1-2：参数与触发模式自检

- [ ] productLine、sourceClassName、targetModule、targetPackage 是否全部提取成功
- [ ] 参数缺失时是否直接终止并提示缺失字段
- [ ] 工作流触发时是否跳过交互
- [ ] 测试类名称是否固定为 {sourceClassName}Test

## 步骤 3-4：规范与框架自检

- [ ] 是否已读取 `.spec/oc-spec-test.md`
- [ ] 是否已校验 JUnit 版本（不可不校验就默认 JUnit 5）
- [ ] JUnit 版本确认结果是否已输出

## 步骤 5-6：代码读取自检

- [ ] 被测试业务类源代码是否已成功读取
- [ ] 业务类依赖的 DTO/Entity/Enum 等是否已读取（仅允许范围内）
- [ ] 是否未读取其他业务应用的测试代码

## 步骤 7：类类型判断自检

- [ ] 类类型是否正确判定（Controller/Consumer/ServiceImpl/Service）
- [ ] 判定规则是否正确应用（后缀匹配优先级）

## 步骤 8-9：测试类生成与写入自检

- [ ] Controller 类型是否使用 MockMvc 接口测试
- [ ] Consumer 类型是否使用 MQ 消费者测试
- [ ] Service/ServiceImpl 类型是否使用 Mockito 单元测试
- [ ] 测试类是否写入 `{targetModule}/src/test/java/{targetPackage转路径}/` 目录

## 步骤 10：代码规范检查自检

- [ ] 测试类名称是否符合规范：{源类名}Test
- [ ] 测试类是否放在 test/java 目录下
- [ ] 测试类包路径是否与业务类完全一致
- [ ] 是否使用标准注解：@RunWith / @Test / @InjectMocks / @Mock / @Autowired（JUnit 4）
- [ ] 是否包含正常场景、异常场景用例
- [ ] 是否遵循 .spec/oc-spec-test.md 规范
- [ ] 测试方法名是否符合命名规范
- [ ] 未使用 JUnit 5 语法（@org.junit.jupiter.api.Test、org.junit.jupiter.api.Assertions）
- [ ] 检查失败时是否输出失败项及修复建议（最多 3 次）

## 步骤 11：输出结果自检

- [ ] 是否按固定模板输出（测试类信息收集结果 + 创建完成表格）
- [ ] 表格是否包含所有字段
