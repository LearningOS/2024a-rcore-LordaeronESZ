# 编程作业

目标：

1. 实现系统调用 `sys_spawn`，根据程序路径创建一个新进程。
2. 实现带优先级的 stride 调度算法，并添加 `sys_set_priority` 系统调用，用以设置进程优先级。
3. 从本章开始，内核必须向前兼容。



方案：

1. 将 `sys_fork` 和 `sys_exec` 的实现结合起来，并去掉冗余的操作（例如 `sys_fork` 拷贝父进程的地址空间）。
2. 在 PCB 中添加 prio 和 stride，并修改 TaskManager 的 add 和 fetch 逻辑，实现优先权调度。
3. 使用 `git cherry-pick` 系列命令，将前一章分支 commit 到本章分支。（因为涉及从任务到进程的变更，修改内容过多，最后还是手动移植了。。。）

# 问答作业

TODO