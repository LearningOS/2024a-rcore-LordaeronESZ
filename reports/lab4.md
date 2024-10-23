# 编程作业

目标：实现 `sys_linkat, sys_unlinkat, sys_fstat` 系统调用，用以支持硬链接机制。



方案：

1. `sys_linkat`：往当前目录（根目录）的数据块中添加一个新的目录项，该目录项的 name 为指定的 new_name，inode_number 为被链接文件的 inode 号，再将 DiskInode 的引用计数加一。
2. `sys_unlinkat`：将当前目录（根目录）的数据块中的目录项移除，将 DiskInode 的引用计数减一，如果正好减为零，则将 DiskInode 及其对应的数据块内存空间回收。
3. `sys_fstat`：根据文件描述符获取到对应的 DiskInode 数据结构，将相关信息填入 Stat 结构体即可，注意用户虚拟内存和内核虚拟内存之间的转换关系。

# 问答作业

TODO