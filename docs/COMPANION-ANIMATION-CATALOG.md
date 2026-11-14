# 桌宠与角色动画清单

2026-11-12，从本机实际1.12.1 Data/patch.MPQ重新读取 AnimationData.dbc 与 Character/Troll/Female/TrollFemale.m2。未修改角色、Agent或桌宠程序。

全局动画表208条，包含人物、生物和物件；Ada的巨魔女性原始模型AnimationLookup中有124个有效条目。其它种族/性别数量、动作造型可不同；全局动画名、原始模型可索引与桌宠事件映射是三件事。缺失条目还可能经PlayableAnimationLookup/AnimationData回退；不是208个动作都属于Ada，也不代表每个ID都是独特玩法技能。

普通坐下：96坐下→97坐姿→98起立。吃喝：97坐姿作为下半身底层，上半身叠加61 EmoteEat，另由SpellVisualKit挂杯子或食物。没有独立命名为Drink的全局动画。资源中的122 EatingLoop不在本次巨魔女性原始lookup内，不能据名字就代替61。

本机实测资源映射：

| 物品 | 法术ID | Visual/Kit | 道具 |
|---|---:|---|---|
| 烤鹌鹑8952、魔法甜面包8076 | 1131 | visual51，STATE409 | Item_Bread |
| 烤鱿鱼13928 | 18230 | visual51，STATE409 | Item_Bread |
| 迪尔格的超美味奇美拉肉片21023 | 25660 | visual51，STATE409 | Item_Bread |
| 牵牛花露8766 | 1137 | visual101，CAST438，anim61 | TankardA_SpellObject |
| 魔法晶水8079 | 22734 | visual101，CAST438，anim61 | TankardA_SpellObject |

因此烹饪食品与普通食品可有不同恢复/增益，却共用同一进食视觉；烤鱿鱼/肉片拿通用面包不是必然映射错误。是否换道具取决于具体物品使用的法术/visual kit，而不是“烹饪”分类。这里的做饭/制造工作动画与坐下吃成品不同。特殊食物的附加外观效果属于另一条效果链，不能等同新增进食身体动画。

0.4.0新增协议1.1动作桥：移动/游泳/潜行/拾取/椅子/跪姿、原生表情、攻击/受击/防御、真实跳跃opcode事件和坐骑组合。动作由真实持续状态或短事件触发；任意资源动画ID可经通用animation事件消费，但不等于Agent会自动产生每种动作。飞行状态仍预留，变形模型/完整CharProc视觉未完成。原生自发跳跃当前无任务行为，不能仅因支持jump事件就声称角色会主动跳。吃喝仍保留既有坐姿和杯子/食物，武器收起策略尚不完整。

以下是Ada原始模型的完整124条可索引列表。中文是便于理解的说明，原名/ID来自资源；表情、攻击名不代表可以获得对应职业技能。

| ID | 原名 | 中文说明 | 当前桌宠 |
|---:|---|---|---|
| 0 | Stand | 待机 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 1 | Death | 死亡过程 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 3 | Stop | 停止 | 通用animation可播放，暂无专门自动触发映射 |
| 4 | Walk | 步行 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 5 | Run | 奔跑 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 8 | StandWound | 站立受伤 | 通用animation可播放，暂无专门自动触发映射 |
| 9 | CombatWound | 战斗受伤 | 通用animation可播放，暂无专门自动触发映射 |
| 10 | CombatCritical | 暴击受击反应 | 通用animation可播放，暂无专门自动触发映射 |
| 11 | ShuffleLeft | 向左挪步 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 12 | ShuffleRight | 向右挪步 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 13 | Walkbackwards | 倒退行走 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 14 | Stun | 昏迷姿态 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 15 | HandsClosed | 握拳 | 通用animation可播放，暂无专门自动触发映射 |
| 16 | AttackUnarmed | 徒手攻击 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 17 | Attack1H | 单手武器攻击 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 18 | Attack2H | 双手武器攻击 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 19 | Attack2HL | 双手长柄攻击 | 通用animation可播放，暂无专门自动触发映射 |
| 20 | ParryUnarmed | 徒手招架 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 21 | Parry1H | 单手武器招架 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 22 | Parry2H | 双手武器招架 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 23 | Parry2HL | 长柄武器招架 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 24 | ShieldBlock | 盾牌格挡 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 25 | ReadyUnarmed | 徒手战斗准备 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 26 | Ready1H | 单手武器战斗准备 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 27 | Ready2H | 双手武器战斗准备 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 28 | Ready2HL | 长柄武器战斗准备 | 通用animation可播放，暂无专门自动触发映射 |
| 29 | ReadyBow | 弓箭准备 | 通用animation可播放，暂无专门自动触发映射 |
| 30 | Dodge | 躲闪 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 37 | JumpStart | 起跳 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 38 | Jump | 空中跳跃 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 39 | JumpEnd | 落地 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 40 | Fall | 下落 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 41 | SwimIdle | 水中待机 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 42 | Swim | 游泳 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 43 | SwimLeft | 向左游 | 通用animation可播放，暂无专门自动触发映射 |
| 44 | SwimRight | 向右游 | 通用animation可播放，暂无专门自动触发映射 |
| 45 | SwimBackwards | 倒退游泳 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 46 | AttackBow | 弓攻击 | 通用animation可播放，暂无专门自动触发映射 |
| 48 | ReadyRifle | 火枪准备 | 通用animation可播放，暂无专门自动触发映射 |
| 49 | AttackRifle | 火枪攻击 | 通用animation可播放，暂无专门自动触发映射 |
| 50 | Loot | 拾取 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 51 | ReadySpellDirected | 定向法术准备 | 部分法术kit/施法状态及通用animation |
| 52 | ReadySpellOmni | 非定向法术准备 | 部分法术kit/施法状态及通用animation |
| 53 | SpellCastDirected | 定向施法 | 部分法术kit/施法状态及通用animation |
| 54 | SpellCastOmni | 非定向施法 | 部分法术kit/施法状态及通用animation |
| 55 | BattleRoar | 战斗咆哮 | 通用animation可播放，暂无专门自动触发映射 |
| 56 | ReadyAbility | 技能准备 | 通用animation可播放，暂无专门自动触发映射 |
| 57 | Special1H | 单手特殊攻击 | 通用animation可播放，暂无专门自动触发映射 |
| 58 | Special2H | 双手特殊攻击 | 通用animation可播放，暂无专门自动触发映射 |
| 59 | ShieldBash | 盾击 | 通用animation可播放，暂无专门自动触发映射 |
| 60 | EmoteTalk | 说话 | 通用Emotes/animation可播放，触发依原生数据 |
| 61 | EmoteEat | 吃喝手部动作 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 62 | EmoteWork | 工作 | 通用Emotes/animation可播放，触发依原生数据 |
| 63 | EmoteUseStanding | 站立使用物品 | 通用Emotes/animation可播放，触发依原生数据 |
| 64 | EmoteTalkExclamation | 感叹式说话 | 通用Emotes/animation可播放，触发依原生数据 |
| 65 | EmoteTalkQuestion | 疑问式说话 | 通用Emotes/animation可播放，触发依原生数据 |
| 66 | EmoteBow | 鞠躬 | 通用Emotes/animation可播放，触发依原生数据 |
| 67 | EmoteWave | 挥手 | 通用Emotes/animation可播放，触发依原生数据 |
| 68 | EmoteCheer | 欢呼 | 通用Emotes/animation可播放，触发依原生数据 |
| 69 | EmoteDance | 跳舞 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 70 | EmoteLaugh | 大笑 | 通用Emotes/animation可播放，触发依原生数据 |
| 73 | EmoteRude | 粗鲁动作 | 通用Emotes/animation可播放，触发依原生数据 |
| 74 | EmoteRoar | 吼叫 | 通用Emotes/animation可播放，触发依原生数据 |
| 75 | EmoteKneel | 下跪表情 | 通用Emotes/animation可播放，触发依原生数据 |
| 76 | EmoteKiss | 飞吻 | 通用Emotes/animation可播放，触发依原生数据 |
| 77 | EmoteCry | 哭泣 | 通用Emotes/animation可播放，触发依原生数据 |
| 78 | EmoteChicken | 小鸡动作 | 通用Emotes/animation可播放，触发依原生数据 |
| 79 | EmoteBeg | 乞求 | 通用Emotes/animation可播放，触发依原生数据 |
| 80 | EmoteApplaud | 鼓掌 | 通用Emotes/animation可播放，触发依原生数据 |
| 81 | EmoteShout | 呼喊 | 通用Emotes/animation可播放，触发依原生数据 |
| 82 | EmoteFlex | 展示肌肉 | 通用Emotes/animation可播放，触发依原生数据 |
| 83 | EmoteShy | 害羞 | 通用Emotes/animation可播放，触发依原生数据 |
| 84 | EmotePoint | 指向 | 通用Emotes/animation可播放，触发依原生数据 |
| 85 | Attack1HPierce | 单手刺击 | 通用animation可播放，暂无专门自动触发映射 |
| 87 | AttackOff | 副手攻击 | 通用animation可播放，暂无专门自动触发映射 |
| 88 | AttackOffPierce | 副手刺击 | 通用animation可播放，暂无专门自动触发映射 |
| 89 | Sheath | 收起武器 | 通用animation可播放，暂无专门自动触发映射 |
| 90 | HipSheath | 腰部收起武器 | 通用animation可播放，暂无专门自动触发映射 |
| 91 | Mount | 骑乘姿态 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 95 | Kick | 踢击 | 通用animation可播放，暂无专门自动触发映射 |
| 96 | SitGroundDown | 坐下过渡 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 97 | SitGround | 坐姿待机 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 98 | SitGroundUp | 起立过渡 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 99 | SleepDown | 躺下过渡 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 100 | Sleep | 睡眠循环 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 101 | SleepUp | 醒来过渡 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 102 | SitChairLow | 坐低椅子 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 103 | SitChairMed | 坐中等椅子 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 104 | SitChairHigh | 坐高椅子 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 105 | LoadBow | 弓装填 | 通用animation可播放，暂无专门自动触发映射 |
| 106 | LoadRifle | 火枪装填 | 通用animation可播放，暂无专门自动触发映射 |
| 107 | AttackThrown | 投掷攻击 | 通用animation可播放，暂无专门自动触发映射 |
| 108 | ReadyThrown | 投掷准备 | 通用animation可播放，暂无专门自动触发映射 |
| 109 | HoldBow | 持弓 | 通用animation可播放，暂无专门自动触发映射 |
| 110 | HoldRifle | 持火枪 | 通用animation可播放，暂无专门自动触发映射 |
| 111 | HoldThrown | 持投掷武器 | 通用animation可播放，暂无专门自动触发映射 |
| 112 | LoadThrown | 投掷装填 | 通用animation可播放，暂无专门自动触发映射 |
| 113 | EmoteSalute | 敬礼 | 通用Emotes/animation可播放，触发依原生数据 |
| 114 | KneelStart | 跪下过渡 | 通用animation可播放，暂无专门自动触发映射 |
| 115 | KneelLoop | 跪姿循环 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 116 | KneelEnd | 跪姿起身 | 通用animation可播放，暂无专门自动触发映射 |
| 118 | SpecialUnarmed | 徒手特殊攻击 | 通用animation可播放，暂无专门自动触发映射 |
| 119 | StealthWalk | 潜行行走 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 120 | StealthStand | 潜行待机 | 1.1持续状态或原生事件映射；具体场景依观测能力 |
| 121 | Knockdown | 击倒 | 通用animation可播放，暂无专门自动触发映射 |
| 123 | UseStandingLoop | 站立使用物品循环 | 通用animation可播放，暂无专门自动触发映射 |
| 124 | ChannelCastDirected | 定向引导施法 | 部分法术kit/施法状态及通用animation |
| 125 | ChannelCastOmni | 非定向引导施法 | 部分法术kit/施法状态及通用animation |
| 126 | Whirlwind | 旋风动作 | 通用animation可播放，暂无专门自动触发映射 |
| 128 | UseStandingStart | 站立使用物品起始 | 通用animation可播放，暂无专门自动触发映射 |
| 129 | UseStandingEnd | 站立使用物品结束 | 通用animation可播放，暂无专门自动触发映射 |
| 131 | Drown | 溺水过程 | 通用animation可播放，暂无专门自动触发映射 |
| 132 | Drowned | 溺亡 | 通用animation可播放，暂无专门自动触发映射 |
| 133 | FishingCast | 抛鱼竿 | 通用animation可播放，暂无专门自动触发映射 |
| 134 | FishingLoop | 钓鱼等待 | 通用animation可播放，暂无专门自动触发映射 |
| 136 | EmoteWorkNoSheathe | 不收武器工作 | 通用animation可播放，暂无专门自动触发映射 |
| 138 | EmoteUseStandingNoSheathe | 不收武器站立使用物品 | 通用animation可播放，暂无专门自动触发映射 |
| 143 | Sprint | 冲刺 | 通用animation可播放，暂无专门自动触发映射 |
| 181 | DragonStomp | 特殊踏地（全局名DragonStomp，不代表角色拥有龙技能） | 通用animation可播放，暂无专门自动触发映射 |
| 185 | EmoteYes | 点头 | 通用Emotes/animation可播放，触发依原生数据 |
| 186 | EmoteNo | 摇头 | 通用Emotes/animation可播放，触发依原生数据 |
| 187 | JumpLandRun | 落地接奔跑 | 通用animation可播放，暂无专门自动触发映射 |
| 195 | EmoteTrain | 火车表情 | 通用Emotes/animation可播放，触发依原生数据 |
| 199 | EmoteEatNoSheathe | 不收武器吃喝 | 通用Emotes/animation可播放，触发依原生数据 |
