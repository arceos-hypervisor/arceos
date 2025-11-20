#!/bin/bash

# 创建多分区磁盘镜像的脚本
# 用法: ./create_multi_partition_disk.sh [分区数量]

set -e

# 默认创建4个分区
PARTITION_COUNT=${1:-4}

echo "创建包含 $PARTITION_COUNT 个分区的磁盘镜像..."

# 清理旧文件
if [ -f "disk.img" ]; then
    echo "删除旧的 disk.img..."
    rm -f disk.img
fi

# 创建磁盘镜像 (200MB)
echo "创建 200MB 磁盘镜像..."
dd if=/dev/zero of=disk.img bs=1M count=200

# 创建GPT分区表
echo "创建 GPT 分区表..."
parted disk.img mklabel gpt

# 创建多个分区
PART_SIZE=40  # 每个分区40MB
PART_TYPES=("fat32" "ext4" "fat32" "ext4" "ntfs" "btrfs")
PART_NAMES=("fat32_1" "ext4_1" "fat32_2" "ext4_2" "ntfs_1" "btrfs_1")

for ((i=0; i<PARTITION_COUNT; i++)); do
    START=$((i * PART_SIZE + 1))
    END=$((START + PART_SIZE - 1))
    
    # 选择文件系统类型
    FS_TYPE=${PART_TYPES[$((i % ${#PART_TYPES[@]}))]}
    PART_NAME=${PART_NAMES[$((i % ${#PART_NAMES[@]}))]}
    
    echo "创建分区 $((i+1)): $FS_TYPE (名称: $PART_NAME)"
    
    # 创建分区
    parted disk.img mkpart primary $FS_TYPE ${START}MB ${END}MB
    
    # 设置分区名称
    parted disk.img name $((i+1)) $PART_NAME
done

# 显示分区信息
echo "分区信息:"
parted disk.img print

# 设置循环设备并格式化分区
echo "格式化分区..."
LOOP_DEV=$(losetup -f --show disk.img)

# 等待一下让系统识别分区表
sleep 1

# 刷新分区表
partprobe $LOOP_DEV

# 格式化每个分区
for ((i=0; i<PARTITION_COUNT; i++)); do
    PART_DEV="${LOOP_DEV}p$((i+1))"
    FS_TYPE=${PART_TYPES[$((i % ${#PART_TYPES[@]}))]}
    
    echo "格式化分区 $((i+1)) 为 $FS_TYPE..."
    
    case $FS_TYPE in
        "fat32")
            mkfs.vfat -F 32 $PART_DEV
            ;;
        "ext4")
            mkfs.ext4 -F $PART_DEV
            ;;
        "ntfs")
            mkfs.ntfs -f $PART_DEV
            ;;
        "btrfs")
            mkfs.btrfs -f $PART_DEV
            ;;
    esac
done

# 分离循环设备
losetup -d $LOOP_DEV

echo "多分区磁盘镜像创建完成!"
echo "分区数量: $PARTITION_COUNT"
echo "磁盘大小: 200MB"
echo "每个分区大小: ${PART_SIZE}MB"

# 显示分区表
echo ""
echo "最终分区表:"
fdisk -l disk.img

echo ""
echo "现在可以运行以下命令测试:"
echo "make A=examples/shell/ ARCH=aarch64 BLK=y features=fs LOG=info run"