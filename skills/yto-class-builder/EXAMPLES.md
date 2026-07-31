# Few-Shot 优质输入输出示例

---

## 示例一：创建 Controller 类

### 输入

| 字段 | 值 |
|------|-----|
| className | ZhuanZhuanTerminalOptController |
| productLine | oc |
| targetModule | oc-opt-terminal-ms |
| targetPackage | cn.com.yto56.oc.opt.terminal.controller |
| businessLogic | 接收转转终端操作请求，调用 zhuanZhuanTerminalOptService 处理核销/改约/取消操作 |

### 执行过程

```
✅ 步骤1完成，参数提取成功
| className | productLine | targetModule | targetPackage |
|------|------|------|------|
| ZhuanZhuanTerminalOptController | oc | oc-opt-terminal-ms | cn.com.yto56.oc.opt.terminal.controller |

✅ 步骤2完成，已计算出 classType 为 Controller

✅ 步骤3完成，ZhuanZhuanTerminalOptController.java 已创建

步骤4：检查清单结果
- [x] 类名符合大驼峰命名，且无连续大写字母
- [x] 包路径与 targetPackage 一致
- [x] 类文件位于 oc-opt-terminal-ms 模块内
- [x] 类注解 @RestController 符合 Controller 规范
- [x] 所有 import 声明完整且无多余导入
- [x] 类代码完整实现了 businessLogic
- [x] 代码符合项目编码规范
```

### 输出

| className | classType | targetModule | targetPackage | 文件路径 | 状态 | businessLogic |
|------|------|------|------|------|------|------|
| ZhuanZhuanTerminalOptController | Controller | oc-opt-terminal-ms | cn.com.yto56.oc.opt.terminal.controller | oc-opt-terminal-ms/src/main/java/.../controller/ZhuanZhuanTerminalOptController.java | 已创建 | 接收转转终端操作请求... |

---

## 示例二：创建 Plugin 类

### 输入

| 字段 | 值 |
|------|-----|
| className | ZhuanZhuanCreateConvertDataPlugin |
| productLine | lms |
| targetModule | lms-access-default-plugin |
| targetPackage | cn.com.yto56.lms.plugin.plugins |
| businessLogic | 1. 从 context 获取 ZzesSecondhandCreateOrderReq 请求对象<br>2. 将请求字段转换为平台标准字段映射<br>3. 调用 context.put 放入转换后的数据对象 |

### 执行过程

```
✅ 步骤1完成，参数提取成功

✅ 步骤2完成，已计算出 classType 为 Plugin

✅ 步骤3完成，ZhuanZhuanCreateConvertDataPlugin.java 已创建

步骤4：检查清单结果
- [x] 类名符合大驼峰命名，且无连续大写字母
- [x] 包路径与 targetPackage 一致
- [x] 类文件位于 lms-access-default-plugin 模块内
- [x] 类注解符合 Plugin 规范
- [x] 所有 import 声明完整且无多余导入
- [x] 类代码完整实现了 businessLogic
- [x] 代码符合项目编码规范
```

### 输出

| className | classType | targetModule | targetPackage | 文件路径 | 状态 | businessLogic |
|------|------|------|------|------|------|------|
| ZhuanZhuanCreateConvertDataPlugin | Plugin | lms-access-default-plugin | cn.com.yto56.lms.plugin.plugins | lms-access-default-plugin/src/main/java/.../plugins/ZhuanZhuanCreateConvertDataPlugin.java | 已创建 | 从 context 获取... |

---

## 示例三：创建 ServiceImpl 类

### 输入

| 字段 | 值 |
|------|-----|
| className | ZhuanZhuanAccessServiceImpl |
| productLine | lms |
| targetModule | lms-access-default-ms |
| targetPackage | cn.com.yto56.lms.service.impl |
| businessLogic | 1. 注入 DispatcherManager<br>2. 调用 dispatcherManager.execute(engineName, key, context) 调度 Plugin 链路<br>3. 从 context 获取执行结果封装为 Resp 返回 |

### 执行过程

```
✅ 步骤1完成，参数提取成功

✅ 步骤2完成，已计算出 classType 为 ServiceImpl

✅ 步骤3完成，ZhuanZhuanAccessServiceImpl.java 已创建

步骤4：检查清单结果
- [x] 类名符合大驼峰命名，且无连续大写字母
- [x] 包路径与 targetPackage 一致
- [x] 类文件位于 lms-access-default-ms 模块内
- [x] 类注解 @Service 符合 ServiceImpl 规范
- [x] 所有 import 声明完整且无多余导入
- [x] 类代码完整实现了 businessLogic（含 DispatcherManager 调度）
- [x] 代码符合项目编码规范
```

### 输出

| className | classType | targetModule | targetPackage | 文件路径 | 状态 | businessLogic |
|------|------|------|------|------|------|------|
| ZhuanZhuanAccessServiceImpl | ServiceImpl | lms-access-default-ms | cn.com.yto56.lms.service.impl | lms-access-default-ms/src/main/java/.../impl/ZhuanZhuanAccessServiceImpl.java | 已创建 | 注入 DispatcherManager... |
