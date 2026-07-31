# 生成后自检清单 (Checklist)

---

## 阶段 1：准备自检

- [ ] plan.md 文件是否已成功读取
- [ ] productLine 是否正确提取
- [ ] 新建模块清单是否完整解析
- [ ] 复用模块清单是否完整解析
- [ ] 业务类清单是否完整解析（每个类含 businessLogic）
- [ ] 中间件配置（redis_list、consumer_list）是否完整解析
- [ ] 依赖关系（dependency_mapping）是否完整解析
- [ ] Plugin 调度策略是否完整解析
- [ ] businessLogic 是否无缺失（缺失则提示用户补充）

## 阶段 2：模块创建自检

- [ ] 新建模块清单中每个模块是否都调用了 yto-module-{type}
- [ ] 传入参数（type、moduleDir、productLine、functionModule、businessName、appType、appName）是否完整
- [ ] 每个模块是否创建成功或已存在跳过

## 阶段 3：配置 Redis 自检

- [ ] redis_list 中每个配置项是否都调用了 yto-middleware-redis
- [ ] 传入参数是否完整（appName、redisName、环境 appId 等）

## 阶段 4：配置 Consumer 自检

- [ ] consumer_list 中每个配置项是否都调用了 yto-middleware-consumer
- [ ] 传入参数是否完整（appName、consumerName、topic 等）

## 阶段 5：添加依赖自检

- [ ] dependency_mapping 中每个微服务是否都调用了 yto-pom-dependency
- [ ] 传入参数（belongApp、dependencies）是否完整

## 阶段 6：插件 Plugin 自检

- [ ] Plugin 调度策略中每个配置项是否都调用了 yto-dispatcher-plugin
- [ ] 传入参数（pluginModule、engineName、configFileName、dispatchers）是否完整

## 阶段 7：Java 类自检

- [ ] 业务类清单中每个类是否都调用了 yto-class-builder
- [ ] 传入参数（className、productLine、targetModule、targetPackage、businessLogic）是否完整
- [ ] businessLogic 是否包含完整执行步骤

## 阶段 8：检查代码自检

- [ ] 每个类的代码是否符合 `.spec/oc-spec-class-*.md` 规范
- [ ] 每个类的 import 是否正确（无缺失、无重复）
- [ ] 发现的错误是否已全部自动修正

## 阶段 9：测试自检

- [ ] 是否仅为新类生成了测试
- [ ] yto-test-builder 是否成功调用
- [ ] 测试代码是否基于 `.spec/oc-spec-test-*.md` 规范

## 阶段 10：验证自检

- [ ] yto-test-runner 是否成功调用
- [ ] 测试失败时是否调用了 yto-code-fixer 修复（最多 3 次）
- [ ] 最终测试结果是否为 PASS

## 通用自检

- [ ] 每个阶段是否作为独立待办任务（未合并）
- [ ] plan.md 中所有清单是否都使用表格格式
- [ ] 所有产物路径是否已记录
- [ ] 全程中文输出
