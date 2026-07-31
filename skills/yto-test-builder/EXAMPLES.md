# Few-Shot 优质输入输出示例

---

## 示例一：为 Controller 类生成测试

### 输入

| 字段 | 值 |
|------|-----|
| productLine | oc |
| sourceClassName | ZhuanZhuanTerminalOptController |
| targetModule | oc-opt-terminal-ms |
| targetPackage | cn.com.yto56.oc.opt.terminal.controller |

### 执行过程

```
✅ 步骤1完成，productLine=oc, sourceClassName=ZhuanZhuanTerminalOptController, targetModule=oc-opt-terminal-ms, targetPackage=cn.com.yto56.oc.opt.terminal.controller

✅ 步骤2完成，工作流触发模式

✅ 步骤3完成，已读取 .spec/oc-spec-test.md

✅ 步骤4完成，JUnit 版本确认为 JUnit4

✅ 步骤5完成，已读取 ZhuanZhuanTerminalOptController.java

✅ 步骤6完成，已读取 3 个依赖类

被测试类类型判定为 Controller

✅ 步骤8完成，ZhuanZhuanTerminalOptControllerTest.java 已生成

✅ 步骤9完成，测试类已写入
```

### 输出

| 字段 | 值 |
|------|-----|
| 被测试类名 | ZhuanZhuanTerminalOptController |
| 测试类名 | ZhuanZhuanTerminalOptControllerTest |
| 目标模块 | oc-opt-terminal-ms |
| 测试包路径 | cn.com.yto56.oc.opt.terminal.controller |

| 变更位置 | 变更内容 |
|---------|---------|
| 测试类 | 创建 `oc-opt-terminal-ms/src/test/java/.../controller/ZhuanZhuanTerminalOptControllerTest.java` |

---

## 示例二：为 ServiceImpl 类生成测试

### 输入

| 字段 | 值 |
|------|-----|
| productLine | lms |
| sourceClassName | ZhuanZhuanAccessServiceImpl |
| targetModule | lms-access-default-ms |
| targetPackage | cn.com.yto56.lms.service.impl |

### 执行过程

```
✅ 步骤1-6完成

被测试类类型判定为 ServiceImpl

✅ 步骤8完成，ZhuanZhuanAccessServiceImplTest.java 已生成（Mockito 单元测试）

✅ 步骤9完成，测试类已写入
```

### 输出

| 字段 | 值 |
|------|-----|
| 被测试类名 | ZhuanZhuanAccessServiceImpl |
| 测试类名 | ZhuanZhuanAccessServiceImplTest |
| 目标模块 | lms-access-default-ms |
| 测试包路径 | cn.com.yto56.lms.service.impl |

| 变更位置 | 变更内容 |
|---------|---------|
| 测试类 | 创建 `lms-access-default-ms/src/test/java/.../impl/ZhuanZhuanAccessServiceImplTest.java` |
