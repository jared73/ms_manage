# 生成后自检清单 (Checklist)

---

## 模块命名与目录

- [ ] 模块命名是否符合 `{productLine}-{businessName}-{dbType}-service` 格式
- [ ] 目录结构是否正确（service/{productLine}-{businessName}-{dbType}-service/src/main/java/.../service/）

## 类与配置

- [ ] AutoConfiguration 类是否正确配置
- [ ] pom.xml 的 parent 是否指向根 pom
- [ ] pom.xml 是否包含必要依赖（yto-framework-datasource、yto-framework-core、security-cipher-sdk）
- [ ] 数据库配置文件是否包含 uat 和 prod 两个环境
- [ ] 数据库配置文件命名是否符合 `bootstrap-db-{businessName}-{dbType}-uat.yml` / `bootstrap-db-{businessName}-{dbType}-prod.yml` 格式
- [ ] 数据库配置是否使用 dynamic-datasource 格式

## POM 注册

- [ ] 根 pom.xml 是否注册了 Service 模块（modules + dependencyManagement）
- [ ] 若 appName 不为空，主应用 pom.xml 是否引入了 Service 模块依赖
- [ ] 若 appName 不为空，主应用 pom.xml 引入依赖是否未指定 version

## 通用自检

- [ ] 每个步骤是否作为独立待办任务（未合并）
- [ ] 已存在的模块/配置是否已跳过
- [ ] 全程中文输出
