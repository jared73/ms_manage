# 生成后自检清单 (Checklist)

---

## 模块命名与目录

- [ ] 模块命名是否符合 `{productLine}-{functionModule}-{businessName}-plugin` 格式
- [ ] 目录结构是否正确（plugin/{productLine}-{functionModule}-{businessName}-plugin/src/main/java/.../plugin/）

## 类与配置

- [ ] AutoConfiguration 类是否使用 `@PropertySource` 加载资源文件
- [ ] pom.xml 是否包含 dispatcher 和 chain 依赖

## POM 注册

- [ ] 根 pom.xml 是否注册了 Plugin 模块（modules + dependencyManagement）
- [ ] 若 appName 不为空，主应用 pom.xml 是否引入了 Plugin 模块依赖
- [ ] Plugin 模块 pom.xml 的 parent 是否指向根 pom（{productLine}-core）
- [ ] 主应用 pom.xml 引入依赖是否未指定 version（由根 pom dependencyManagement 管理）

## 通用自检

- [ ] 每个步骤是否作为独立待办任务（未合并）
- [ ] 已存在的模块/配置是否已跳过
- [ ] 全程中文输出
