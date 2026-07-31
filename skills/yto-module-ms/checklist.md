# 生成后自检清单 (Checklist)

---

## 基础信息校验

- [ ] moduleDir 是否属于合法类型（client/ms/common/plugin/producer/service/plat）
- [ ] 微服务命名是否符合 `{productLine}-{functionModule}-{businessName}-{appType}` 格式
- [ ] appType 是否属于合法类型（api/ms/consumer/job）

## ms 模块

- [ ] 包名是否符合 `cn.com.yto56.{productLine}.{functionModule}.{businessName}` 格式
- [ ] 包路径结构是否按 appType 正确创建
- [ ] 启动类注解是否完整（`@SpringBootApplication`、`@EnableDiscoveryClient`、`@EnableFeignClients`、`@EnableConfigurationProperties`）
- [ ] 启动类 `scanBasePackages` 是否包含 `cn.com.yto56.{productLine}` 和 `cn.com.yto56.client`
- [ ] 启动类 `@EnableFeignClients` 的 `basePackages` 是否包含 `cn.com.yto56.client`
- [ ] 启动类是否使用 `YTOApplication.start()` 启动
- [ ] 资源文件是否完整（bootstrap.yml、bootstrap-uat.yml、bootstrap-prod.yml、bootstrap-common-uat.yml、bootstrap-common-prod.yml）
- [ ] bootstrap.yml 中 `spring.application.name` 是否与微服务名称一致
- [ ] bootstrap-uat.yml 中 `swagger.basePackage` 是否与包名一致
- [ ] 主应用 pom.xml 的 `parent` 是否指向根 pom.xml
- [ ] 主应用 pom.xml 是否包含框架依赖
- [ ] 主应用 pom.xml 是否包含公共模块依赖（{productLine}-common、{productLine}-dto）

## 根 POM 注册

- [ ] 根 pom.xml 的 `<modules>` 中是否按目录分组注册了新模块
- [ ] 根 pom.xml 的 `<dependencyManagement>` 中是否添加了模块版本管理

## 依赖分组顺序

- [ ] 主应用 pom.xml 依赖是否按分组顺序排列（框架依赖 → 公共模块 → Plugin → Producer → Client → 平台模块）

## 通用自检

- [ ] 每个步骤是否作为独立待办任务（未合并）
- [ ] 已存在的模块/配置是否已跳过
- [ ] 全程中文输出
