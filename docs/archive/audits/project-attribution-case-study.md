# project-alpha 项目 Token 归因专项审计

> 历史审计证据，不是当前规范。当前规则见
> [统计契约](../../contracts/accounting.md)。

审计时间：2026-09-01（Asia/Shanghai）

## 结论

当前 GUI 显示的 project-alpha 约 45.3 亿不是项目真实累计量，而只是本应用
`post-sampling` 账本已经捕获的部分。对仍存在的 609 个同仓库线程逐文件剥离
subagent 继承历史并按累计计数正增量重建后，project-alpha 的 replay-safe 本机下限
约为 190.0 亿；相对当前账本新增约 144.7 亿。

这个结果仍是本机证据下限，不是官方计费账单，也不包含已经删除且没有保留
rollout 的线程。

## 数据源和范围

- Codex `state_5.sqlite`：线程身份、父子边、cwd、rollout 路径、Git origin。
- Codex rollout JSONL：逐条 `token_count.total_token_usage` 与组成字段。
- Usage Ledger：当前 confirmed `post-sampling` 事件和日汇总。
- Codex 官方账号日桶：仅用于账号范围校验，不用于猜测项目归属。

project-alpha 当前项目目录中有 608 个目录节点和 21 个根会话。另发现 1 个
`/Users/example/.codex/worktrees/.../project-alpha` 会话拥有相同 Git origin，却被
现有解析器归为无项目。609 个 rollout 共约 16.79 GiB。

## 不能使用的算法

`threads.tokens_used` 不能跨线程求和。609 个线程的该字段合计
1,342,462,043,956 tokens（约 1.342 万亿），而重建下限只有约 190 亿，朴素
求和膨胀约 70.7 倍。

原因是 subagent rollout 在创建时复制祖先的完整 `token_count` 历史。专项
扫描读到 3,579,650 条 token 事件，其中 3,494,279 条（97.62%）属于创建时
的密集继承前缀；另有 3,113 条累计值未变化的重复重发。

## replay-safe 重建规则

1. 根会话从累计零点开始，只接受 `total_token_usage` 的正增量。
2. 子代理从 canonical `session_meta` 后进入“继承前缀待判定”状态。
3. 创建后连续密集出现的 token 序列视为候选继承前缀；以相邻 token 事件
   首次超过 2 秒的间隙作为真实调用边界。project-alpha 中继承前缀持续时间中位数
   0.103 秒、P90 0.682 秒、最大 2.822 秒。
4. 前缀末端累计值成为 child baseline，前缀本身不产生用量。
5. baseline 之后仅累计 `current_total - previous_total` 的正增量；累计值未变
   的重发为零；明确回退才开始新 counter epoch。
6. 输入、缓存读取、缓存写入、输出分别按同一累计差分计算，并要求
   `input + output = total`、缓存读取不超过输入。
7. 与 post-sampling 重叠时绝不相加；按 `thread × local day` 选择证据更完整
   的整行来源。项目、模型和组成都必须来自同一个被选来源。

## 审计结果

| 指标 | Token |
|---|---:|
| 当前 Usage Ledger confirmed（含修正前错归 worktree） | 4,533,350,092 |
| rollout replay-safe 重建 | 18,954,414,962 |
| `thread × day` 来源选择后的统一下限 | 19,000,170,830 |
| 当前账本之外可补充 | 14,466,820,738 |

补充部分的组成守恒：

| 组成 | Token |
|---|---:|
| Input | 14,438,218,970 |
| Cached input | 14,151,845,668 |
| Output | 28,601,768 |
| Reasoning（Output 子集） | 11,133,716 |
| Total | 14,466,820,738 |

`Input + Output - Total = 0`。

## 独立交叉验证

- 307 个同时存在 rollout 重建和 ledger 事件的线程中，重建/ledger 比值中位数
  为 1.0000；P10 为 0.9521，P90 为 1.4433。
- 2026-09-01 的完整实时区间：ledger 1,658,967,924，rollout 重建
  1,680,260,660，相差约 1.3%。按线程日选择后为 1,702,686,922。
- 旧日期的差异显著更大，且集中在 post-sampling 账本开始采集前或日志轮转后，
  符合“历史源缺失”而不是“项目突然不活跃”的模式。

## 根因

### 1. 正常 daemon 没有扫描历史 rollout

当前 daemon 的循环只调用 `ingest_post_sampling` 和额度 tail；通用的
`ingest_all`/`ingest_paths` 只存在于库和测试路径，没有进入正常 daemon 的历史
采集主流程。因此应用启动前的 rollout 即使仍在磁盘，也不会进入项目账本。

### 2. post-sampling 只能证明仍保留在 logs_2.sqlite 的请求

当前主 logs 数据从 8 月下旬开始，旧分片仅覆盖 6 月少量日期。project-alpha 从 7 月
开始使用，302 个目录节点完全没有 confirmed sampling；其余节点也存在部分日期
缺口。logs 数据轮转后，严格白名单算法会把仍可恢复的 rollout 用量视为不存在。

### 3. Codex worktree 项目解析存在自举死循环

项目配置只有 `/Users/example/project-alpha` 根路径。worktree cwd 不在该前缀下，必须
依赖 Git identity；但现有代码只从已经带 native `project_id` 的线程学习 Git
identity，而大量线程的 native `project_id` 本来就是空值。结果是同仓库 worktree
无法为项目提供 Git identity，也无法再通过 Git identity 归入项目。

### 4. “无项目证据”混合了不可恢复和可恢复两类缺口

账号 Total 与本机项目账本的差额中既包含其他设备、未捕获账号和已删除详情，
也包含 rollout 仍在但应用没有解析的可恢复历史。把它们统一显示为“无项目证据”
会掩盖采集算法缺陷。

## 对总缺口的影响

审计快照的账号下限约 951.8 亿，本机 confirmed 约 124.7 亿，差额约 827.2 亿。
只补入 project-alpha 的 144.7 亿后，本机可归因下限升至约 269.3 亿，覆盖率从约
13.1% 升至 28.3%，剩余差额降至约 682.5 亿。其他仍保留 rollout 的项目需要用
同一算法重建；已删除的 project-epsilon 子代理不能用累计字段替代。

## 产品级修复要求

1. 新增一次性、可续跑的 rollout reconstruction 游标；启动后分批回填，后续只读
   追加字节，不在每次启动重扫全部文件。
2. 历史重建按线程、日期、账号、模型和 Token 组成保存紧凑汇总，不保存复制前缀。
3. post-sampling 与 rollout reconstruction 按 `thread × day` 做来源选择，绝不求和。
4. UI 分别显示“post-sampling confirmed”“rollout reconstructed”“两源交叉验证”
   和“真正无可用源”；不再把可恢复 rollout 混进无项目差额。
5. 项目解析从配置根目录内已知线程学习 Git origin，使同仓库 Codex worktree 自动
   归入原项目；重投影只能改变项目维度，Token 总量必须完全不变。
6. 任何项目日用量若超过同账号官方日桶，必须先检查账号覆盖是否完整；官方只捕获
   2/4 账号时不能把已捕获日桶当作全设备硬上限。

## 验收恒等式

- 同一来源：`input + output = total`。
- 缓存读取和缓存写入均为 input 子集，不重复加到 total。
- `effective(thread, day) = choose(sampling, reconstruction)`，不是两者之和。
- 项目合计 + 独立对话 + 真正未归属 = 本机有效归因总量。
- 本机有效归因仍不得冒充完整官方账号 Total。
