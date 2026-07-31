# 生成后自检清单 (Checklist)

---

## 步骤 1：提取参数自检

- [ ] className、productLine、targetModule、targetPackage、businessLogic 是否全部提取成功
- [ ] 参数缺失时是否直接终止并提示缺失字段
- [ ] 是否按表格格式输出了参数

## 步骤 2：计算 classType 自检

- [ ] classType 是否正确拆分（取驼峰类名最后一个单词）
- [ ] 拆分是否遵循标准驼峰规则（无连续大写）

## 步骤 3：创建 Java 类自检

- [ ] 是否已读取 `oc-spec-content.md` 规范
- [ ] 是否已读取 `oc-spec-class-{classType}.md` 规范
- [ ] 类文件已存在时是否跳过
- [ ] 类代码是否完整实现了 businessLogic 描述的业务功能

## 步骤 4：代码规范检查自检

- [ ] 类名符合大驼峰命名，且无连续大写字母
- [ ] 包路径与 targetPackage 一致，无拼写错误
- [ ] 类文件位于指定的 targetModule 模块内
- [ ] 类注解（如 `@RestController`、`@Service` 等）符合该 classType 规范
- [ ] 所有必要的 import 声明完整且无多余导入
- [ ] 类代码完整实现了 businessLogic 描述的业务功能
- [ ] 代码符合项目编码规范：无注释、无多余空行、符合 oc-spec-content.md 中的通用约定
- [ ] 检查失败时是否输出失败项及修复建议（最多 3 次）

## 步骤 5：输出结果自检

- [ ] 是否按固定模板输出（类创建结果表格）
- [ ] 表格是否包含所有字段（className、classType、targetModule、targetPackage、文件路径、状态、businessLogic）
