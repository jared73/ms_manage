# 生成后自检清单 (Checklist)

---

## 依赖注入

- [ ] 每个微服务的 pom.xml 是否引入了所有指定的依赖模块
- [ ] 新增依赖是否未指定 version（由根 pom dependencyManagement 管理）
- [ ] 新增依赖的 groupId 是否正确（com.yto.hermes）
- [ ] 新增依赖的 artifactId 是否与模块名一致
- [ ] 是否未引入重复依赖
- [ ] 是否未覆盖已有依赖

## 通用自检

- [ ] 每个步骤是否作为独立待办任务（未合并）
- [ ] 全程中文输出
