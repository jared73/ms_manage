# Few-Shot 优质输入输出示例

---

## 示例一：单产品线 plan 输出（LMS）

### 输入

specFilePath: .requirements/转转二手接入-1.0.0/spec.md

### 输出：plan-lms.md（节选）

```markdown
# LMS 产品线技术方案

## 应用交互图

graph TB
    A[lms-access-default-ms] --> B[lms-access-default-plugin]
    B --> C[zz-es-client]
    B --> D[lms-common]

## 模块决策表

| 模块名 | type | 决策 | 理由 |
|--------|------|------|------|
| lms-access-default-plugin | plugin | 复用 | 已有 Plugin 模块 |
| lms-common | common | 复用 | 已有公共模块 |
| zz-es-client | client | 新建 | 需调用转转 ES 接口 |

## 新建模块清单

| type | moduleDir | productLine | functionModule | businessName | appType | appName |
|------|-----------|-------------|----------------|--------------|---------|---------|
| client | client | zz | es | secondhand | client | zz-es-secondhand-client |

## 业务类清单

| className | productLine | targetModule | targetPackage | classType | apiEndpoint | businessLogic |
|-----------|-------------|--------------|---------------|-----------|-------------|---------------|
| ZzesCreateConvertDataPlugin | lms | lms-access-default-plugin | cn.com.yto56.lms.plugin.plugins | Plugin | - | 1. 从 context 获取 ZzesSecondhandCreateOrderReq<br>2. 将请求字段转换为平台标准字段映射<br>3. 调用 context.put 放入转换后的数据对象 |
| ZzesCreateParamValidPlugin | lms | lms-access-default-plugin | cn.com.yto56.lms.plugin.plugins | Plugin | - | 1. 从 context 获取 ZzesSecondhandCreateOrderReq<br>2. 校验必填字段（订单号、商品ID等）<br>3. 校验不通过抛出 BusinessException |

## Plugin 调度策略

| engineName | configFileName | dispatchers 数量 |
|------------|----------------|-----------------|
| zzes-std-access | bootstrap-plugin-zzes-std.yml | 3 |

## 依赖关系

| belongApp | dependencies |
|-----------|-------------|
| lms-access-default-ms | lms-access-default-plugin, lms-common, zz-es-secondhand-client |
```

---

## 示例二：多产品线索引 plan-index.md

### 输出

```markdown
# 技术方案索引

## 需求概述
转转二手订单接入，涉及 LMS 平台接入和 OC 终端操作两个产品线。

## 产物清单

| 文件 | 产品线 | 内容说明 |
|------|--------|---------|
| [plan-lms.md](plan-lms.md) | lms | 平台接入：Plugin、调度策略、Client |
| [plan-oc.md](plan-oc.md) | oc | 终端操作：Controller、Service、Feign |

## 交互流程

graph TB
    A[oc-opt-terminal-ms] --> B[oc-opt-terminal-plugin]
    B --> C[zz-terminal-client]
    D[lms-access-default-ms] --> E[lms-access-default-plugin]
    E --> C
```
