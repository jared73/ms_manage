# 生成后自检清单 (Checklist)

---

## 步骤 1-4：信息收集自检

- [ ] spec.md 是否已成功读取
- [ ] 原始需求文档是否已读取（若提供）
- [ ] 业务规范 `.spec/oc-business-*.md` 是否已匹配并读取
- [ ] 产品线地图 `.spec/oc-project-business-map.md` 是否已读取
- [ ] 项目地图 `.doc/{产品线}-business-map.md` 是否已加载

## 步骤 5：模块决策自检

- [ ] 是否已遵守 `.spec/oc-spec-application-responsibility.md`
- [ ] 每个模块决策（新建/复用/不需要）是否给出理由
- [ ] 标注「复用」的模块是否真实存在
- [ ] 标注「新建」的模块是否确实不存在
- [ ] 决策范围是否覆盖 ms、plugin、client、service、producer、common、dto

## 步骤 6：类清单自检

- [ ] 是否已遵守 `.spec/oc-spec-name.md` 命名规范
- [ ] 是否已遵守 `.spec/oc-spec-class-*.md` 编写规范
- [ ] 每个类是否包含所有必填字段（className、productLine、targetModule、targetPackage、classType、apiEndpoint、businessLogic）
- [ ] 每个类的 businessLogic 是否包含完整执行步骤（禁止简略描述）
- [ ] Controller 类是否包含 @PostMapping 路径、Req 对象、Service 方法、Resp 返回
- [ ] Plugin 类是否包含 context 获取、Feign/Client 调用、异常处理、结果放入 context
- [ ] Feign 类是否包含目标 URL、路径、入参、出参

## 步骤 7：Plugin 调度策略自检

- [ ] 是否已遵守 `.spec/oc-spec-plugin-dispatcher.md`
- [ ] 每个 dispatcher 是否遵循四阶段结构
- [ ] 调度配置是否包含所有必填字段（engineName、configFileName、dispatchers）

## 步骤 8：依赖与中间件自检

- [ ] dependency_mapping 是否完整
- [ ] redis_list 是否包含所有必填字段
- [ ] consumer_list 是否包含所有必填字段

## 步骤 9：产品线拆分自检

- [ ] 是否正确识别涉及的产品线
- [ ] 每个产品线是否生成独立的 plan-{productLine}.md
- [ ] 每个文件是否仅包含该产品线的内容
- [ ] 应用交互 mermaid 图是否仅包含该产品线的模块
- [ ] 模块清单是否仅包含该产品线的模块
- [ ] 业务类清单是否仅包含该产品线的类

## 步骤 10-11：索引与确认自检

- [ ] 多产品线时是否生成了 plan-index.md
- [ ] plan-index.md 是否正确链接各产品线文件
- [ ] 是否已暂停等待用户确认
- [ ] 禁止在未获得确认前执行后续步骤

## 通用自检

- [ ] 所有清单是否使用表格格式
- [ ] 所有文件是否可以被下一个 AI 直接使用，无需额外推断
- [ ] businessLogic 禁止使用简略描述
- [ ] 全程中文输出
