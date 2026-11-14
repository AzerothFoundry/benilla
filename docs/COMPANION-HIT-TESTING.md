# 桌宠轮廓点击穿透

实现为 macOS 主线程全局指针观测与人物几何 GPU 轮廓结合，设置窗口和角色选择页不穿透。其它平台保持原交互，尚未验证同等原生穿透。

离屏相机以最多30Hz、256像素宽、单个在途异步readback更新alpha轮廓，复用当前皮肤骨骼姿态、可见装备与坐骑实体。它排除法术／装备光效祖先及标记的billboard，不把大型光圈当人物。alpha阈值48/255，边缘允许2个mask像素容差；超过0.5秒未完成更新或模型未准备好时不截获背景点击。角色动作与GPU读回存在有限延迟，不声称操作系统逐像素零延迟命中。

全局指针用NSEvent读取，经NSWindow屏幕坐标转换及contentView翻转映射到逻辑内容坐标，不依赖穿透期间不会更新的CursorMoved。只有人物轮廓上的新按下才捕获；捕获保持到全局松开，因拖动离开身体不会中途穿透。已在背景按下的拖动经过人物时不抢鼠标。菜单入口始终可用。

## 原生验收（不得只检查flags）

准备独立临时设置的最新集成QA程序；父任务负责停止待更新的正式桌宠，避免无法区分实例。所有点击仅针对自建背景与本次QA窗口，禁止点击其它应用。不得自动弹权限或改TCC。

辅助程序：

- `/tmp/companion-clickthrough-background`：自建背景窗口，点击计数写入传入JSON路径，600秒自动退出。
- `/tmp/companion-clickthrough-background --permissions`：只读检测post-event及screen-capture权限。
- `/tmp/companion-clickthrough-inject QA_PID BACKGROUND_PID x y click`：CG全局坐标点击；仅当此处窗口栈到测试背景之间没有第三方应用时注入。
- 同一injector末尾可用`drag end_x end_y`，整个起终点均需测试窗口栈覆盖。
- QA进程设置`REALM_COMPANION_HIT_MASK_DUMP=/tmp/unique.pgm`可导出首个非空GPU mask；可转PNG与真实OS画面对照。此文件不是截图每帧读取，只有显式环境变量才写一次。

至少验证：透明点点击使背景计数+1，人物点不增加；人物点起拖实际改变QA窗口位置；背景起拖穿过人物不改变QA位置；放大缩小/坐下/挥手/装备隐藏后轮廓变化；大法术效果外缘仍可穿透；设置和选择页可点击；坐骑实体可命中但其粒子不能扩张命中区。配合CGWindow bounds和背景计数保存实际结果，不能用NSApp.sendEvent伪称已走OS命中路由。

当前实现检查与PointerGate两项回归已通过；最终集成原生验收结果由发布任务补充，不以本文声明代替实测。

## 2026-11-12 原生交互验收

用户在本次独立QA实例上明确反馈透明区域能点击到后方窗口、角色拖动正常，作为人工原生验证通过。PointerGate两项与mask判定一项回归、编译检查通过。

受控自动注入未声称全通过：用户移动窗口后测试背景未覆盖目标点，guarded injector拒绝注入；没有点击第三方应用。背景起拖跨入人物及大型法术外缘未单独取得自动OS证据，保留自动化边界。已清理自己的QA和测试背景进程，fixture交还坐骑QA。详细私有结果 `/tmp/companion-hit-os/final-report.json`。
