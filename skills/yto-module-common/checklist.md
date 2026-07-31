# 生成后自检清单 (Checklist)

---

## 模块命名与目录

- [ ] common 模块命名是否符合 `{productLine}-common` 格式
- [ ] dto 模块命名是否符合 `{productLine}-dto` 格式
- [ ] common 模块目录结构是否正确（common/{productLine}-common/src/main/java/.../common/）
- [ ] dto 模块目录结构是否正确（common/{productLine}-dto/src/main/java/.../dto/）

## POM 注册

- [ ] 根 pom.xml 是否注册了 common 模块（modules + dependencyManagement）
- [ ] 根 pom.xml 是否注册了 dto 模块（modules + dependencyManagement）
- [ ] common 模块 pom.xml 的 parent 是否指向根 pom
- [ ] dto 模块 pom.xml 的 parent 是否指向根 pom
- [ ] dto 模块是否引入 common 模块依赖

## 通用自检

- [ ] 每个步骤是否作为独立待办任务（未合并）
- [ ] 已存在的模块/配置是否已跳过（不重复创建）
- [ ] 全程中文输出
