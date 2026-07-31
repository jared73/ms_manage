# 生成后自检清单 (Checklist)

---

## POM 依赖

- [ ] 主应用 pom.xml 是否引入了 Consumer 相关依赖（yto-framework-plugin-rocketmq）

## 配置文件

- [ ] Consumer 配置文件命名是否符合 `bootstrap-consumer-uat.yml` / `bootstrap-consumer-prod.yml` 格式
- [ ] Consumer 配置文件是否位于主应用 resources 目录下
- [ ] uat 和 prod 环境配置是否分别包含正确的 nameServer

## Consumer 类

- [ ] Consumer 类是否继承 `GeneralMqConsumer`
- [ ] Consumer 类是否添加了 `@Component` 和 `@ConsumeTopic` 注解
- [ ] Consumer 类是否实现了 `process` 方法
- [ ] Consumer 类的包路径是否正确

## 通用自检

- [ ] 每个步骤是否作为独立待办任务（未合并）
- [ ] 已存在的配置/类是否已跳过
- [ ] 全程中文输出
