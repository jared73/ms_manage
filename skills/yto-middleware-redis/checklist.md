# 生成后自检清单 (Checklist)

---

## POM 依赖

- [ ] 主应用 pom.xml 是否引入了 Redis 相关依赖（yto-framework-redis）

## 配置文件

- [ ] Redis 配置文件命名是否符合 `bootstrap-redis-uat.yml` / `bootstrap-redis-prod.yml` 格式
- [ ] Redis 配置文件是否位于主应用 resources 目录下
- [ ] uat 和 prod 环境配置是否分别包含正确的 host、port、password
- [ ] Redis 配置是否使用 spring.redis 前缀格式
- [ ] password 是否使用 ENC 加密格式

## 通用自检

- [ ] 每个步骤是否作为独立待办任务（未合并）
- [ ] 已存在的配置是否已跳过
- [ ] 全程中文输出
