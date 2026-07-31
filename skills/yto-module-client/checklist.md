# 生成后自检清单 (Checklist)

---

## 模块命名与目录

- [ ] 模块命名是否符合 `{clientProductLine}-{functionModule}-{businessName}-client` 格式
- [ ] 目录结构是否正确（client/{clientProductLine}-{functionModule}-{businessName}-client/src/main/java/.../client/）

## 类与配置

- [ ] Feign 接口是否添加了 `@FeignClient` 注解
- [ ] Service 实现类是否注入了 Feign 接口并封装调用逻辑
- [ ] AutoConfiguration 类是否使用 `@PropertySource` 加载资源文件
- [ ] 资源文件是否包含 uat 和 prod 两个环境配置

## POM 注册

- [ ] 根 pom.xml 是否注册了 Client 模块（modules + dependencyManagement）
- [ ] 若 appName 不为空，主应用 pom.xml 是否引入了 Client 模块依赖
- [ ] Client 模块 pom.xml 的 parent 是否指向根 pom
- [ ] Client 模块 pom.xml 是否包含必要依赖（yto-framework-core、yto-framework-model、openfeign）
- [ ] 若 appName 不为空，主应用 pom.xml 引入依赖是否未指定 version

## 通用自检

- [ ] 每个步骤是否作为独立待办任务（未合并）
- [ ] 已存在的模块/配置是否已跳过
- [ ] 全程中文输出
